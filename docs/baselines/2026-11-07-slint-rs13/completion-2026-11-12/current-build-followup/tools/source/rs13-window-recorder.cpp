// Read-only, selected-window Graphics Capture. No activation or injected input.
// Frame SystemRelativeTime describes this capture surface, not photon scanout.
#define NOMINMAX
#include <windows.h>
#include <d3d11.h>
#include <dxgi.h>
#include <dwmapi.h>
#include <windows.graphics.capture.interop.h>
#include <windows.graphics.directx.direct3d11.interop.h>
#include <winrt/Windows.Foundation.h>
#include <winrt/Windows.Graphics.Capture.h>
#include <winrt/Windows.Graphics.DirectX.h>
#include <winrt/Windows.Graphics.DirectX.Direct3D11.h>
#include <wrl/client.h>
#include <algorithm>
#include <condition_variable>
#include <deque>
#include <exception>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <map>
#include <mutex>
#include <stdexcept>
#include <string>
#include <thread>
#include <vector>

using Microsoft::WRL::ComPtr;
using namespace winrt::Windows::Graphics::Capture;
using namespace winrt::Windows::Graphics::DirectX;
namespace fs = std::filesystem;

static long long qpc() { LARGE_INTEGER n{}; QueryPerformanceCounter(&n); return n.QuadPart; }
static long long frequency() { LARGE_INTEGER n{}; QueryPerformanceFrequency(&n); return n.QuadPart; }
static void check(HRESULT result) { winrt::check_hresult(result); }
static DWORD selectedPid = 0;
static HWND selectedWindow = nullptr;
static unsigned windowCount = 0;
static const wchar_t* selectedClass = nullptr;
static BOOL CALLBACK enumerate(HWND window, LPARAM) {
    DWORD pid = 0; GetWindowThreadProcessId(window, &pid);
    wchar_t name[128]{}; GetClassNameW(window, name, 128);
    if (pid == selectedPid && wcscmp(name, selectedClass) == 0) { selectedWindow = window; ++windowCount; }
    return TRUE;
}
static void bitmap(const fs::path& path, const std::vector<unsigned char>& pixels, int width, int height) {
    BITMAPFILEHEADER file{}; file.bfType = 0x4d42;
    file.bfOffBits = sizeof(file) + sizeof(BITMAPINFOHEADER);
    file.bfSize = file.bfOffBits + static_cast<DWORD>(pixels.size());
    BITMAPINFOHEADER info{}; info.biSize = sizeof(info); info.biWidth = width; info.biHeight = -height;
    info.biPlanes = 1; info.biBitCount = 32; info.biCompression = BI_RGB;
    std::ofstream output(path, std::ios::binary);
    output.write(reinterpret_cast<const char*>(&file), sizeof(file));
    output.write(reinterpret_cast<const char*>(&info), sizeof(info));
    output.write(reinterpret_cast<const char*>(pixels.data()), static_cast<std::streamsize>(pixels.size()));
    if (!output) throw std::runtime_error("write_window_frame");
}

// Disk writes must not stall the WGC loop during a 100-350ms transition. The
// queue owns at most 128MiB, including the image currently being written.
class FrameWriter {
    struct Frame { fs::path path; std::vector<unsigned char> pixels; int width, height; };
    std::mutex mutex;
    std::condition_variable wake;
    std::deque<Frame> queue;
    size_t bytes = 0;
    bool closed = false;
    std::exception_ptr failure;
    std::thread worker;
    void run() {
        try {
            for (;;) {
                Frame frame;
                {
                    std::unique_lock lock(mutex);
                    wake.wait(lock, [&] { return closed || !queue.empty(); });
                    if (queue.empty()) return;
                    frame = std::move(queue.front()); queue.pop_front();
                }
                bitmap(frame.path, frame.pixels, frame.width, frame.height);
                std::lock_guard lock(mutex); bytes -= frame.pixels.size();
            }
        } catch (...) { std::lock_guard lock(mutex); failure = std::current_exception(); }
    }
public:
    FrameWriter() : worker([this] { run(); }) {}
    bool submit(const fs::path& path, const std::vector<unsigned char>& pixels, int width, int height) {
        std::lock_guard lock(mutex);
        if (failure) std::rethrow_exception(failure);
        if (closed || bytes + pixels.size() > 128ULL * 1024 * 1024) return false;
        bytes += pixels.size(); queue.push_back(Frame{path, pixels, width, height}); wake.notify_one(); return true;
    }
    void finish() {
        { std::lock_guard lock(mutex); closed = true; wake.notify_one(); }
        if (worker.joinable()) worker.join();
        if (failure) std::rethrow_exception(failure);
    }
    ~FrameWriter() {
        { std::lock_guard lock(mutex); closed = true; wake.notify_one(); }
        if (worker.joinable()) worker.join();
    }
};

struct Readback {
    ComPtr<ID3D11Texture2D> texture;
    std::string clip, fields;
    unsigned index;
    int width, height;
};
struct ClipStorage { unsigned frames = 0; uint64_t bytes = 0; };

int wmain(int argc, wchar_t** argv) {
    HANDLE process = nullptr;
    try {
        if (argc != 6) throw std::runtime_error("mode_pid_seconds_limit_fresh_directory_required");
        bool wails = wcscmp(argv[1], L"wails") == 0;
        if (!wails && wcscmp(argv[1], L"slint") != 0) throw std::runtime_error("unsupported_target_kind");
        selectedPid = std::stoul(argv[2]); int seconds = std::stoi(argv[3]); unsigned limit = std::stoul(argv[4]);
        if (!selectedPid || seconds < 1 || seconds > 900 || limit < 1 || limit > 128) throw std::runtime_error("capture_limits");
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        selectedClass = wails ? L"VeloLauncherWindow" : L"VeloSlintLauncherWindow";
        EnumWindows(enumerate, 0);
        if (windowCount != 1) throw std::runtime_error("expected_one_selected_window");
        process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE, FALSE, selectedPid);
        if (!process) throw std::runtime_error("target_process_identity");
        FILETIME created{}, exited{}, kernel{}, user{};
        if (!GetProcessTimes(process, &created, &exited, &kernel, &user)) throw std::runtime_error("target_creation_time");
        ULARGE_INTEGER creation{}; creation.LowPart = created.dwLowDateTime; creation.HighPart = created.dwHighDateTime;
        fs::path directory(argv[5]);
        if (!fs::create_directory(directory)) throw std::runtime_error("directory_exists");
        std::ofstream log(directory / "events.jsonl");
        auto row = [&](const std::string& text) { log << text << '\n'; log.flush(); if (!log) throw std::runtime_error("write_trace"); };
        winrt::init_apartment(winrt::apartment_type::multi_threaded);
        if (!GraphicsCaptureSession::IsSupported()) throw std::runtime_error("window_capture_unavailable");
        ComPtr<ID3D11Device> device; ComPtr<ID3D11DeviceContext> context;
        check(D3D11CreateDevice(nullptr, D3D_DRIVER_TYPE_HARDWARE, nullptr, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            nullptr, 0, D3D11_SDK_VERSION, &device, nullptr, &context));
        ComPtr<IDXGIDevice> dxgi; check(device.As(&dxgi));
        winrt::com_ptr<IInspectable> inspectable;
        check(CreateDirect3D11DeviceFromDXGIDevice(dxgi.Get(), inspectable.put()));
        auto captureDevice = inspectable.as<winrt::Windows::Graphics::DirectX::Direct3D11::IDirect3DDevice>();
        auto interop = winrt::get_activation_factory<GraphicsCaptureItem, IGraphicsCaptureItemInterop>();
        GraphicsCaptureItem item{nullptr};
        check(interop->CreateForWindow(selectedWindow, winrt::guid_of<GraphicsCaptureItem>(), winrt::put_abi(item)));
        auto size = item.Size();
        auto pool = Direct3D11CaptureFramePool::CreateFreeThreaded(captureDevice, DirectXPixelFormat::B8G8R8A8UIntNormalized, 3, size);
        auto session = pool.CreateCaptureSession(item);
        session.IsCursorCaptureEnabled(false);
        bool secondary = false;
        try { session.IncludeSecondaryWindows(true); secondary = true; }
        catch (const winrt::hresult_error&) { /* Older Windows supports the primary surface only; report it below. */ }
        session.StartCapture();
        row("{\"event\":\"capture_ready\",\"qpc\":" + std::to_string(qpc()) + ",\"qpc_frequency\":" + std::to_string(frequency()) +
            ",\"pid\":" + std::to_string(selectedPid) + ",\"creation_filetime_100ns\":" + std::to_string(creation.QuadPart) +
            ",\"backend\":\"WindowsGraphicsCapture\",\"desktop_frame\":false,\"cursor_captured\":false,\"secondary_windows\":"+(secondary?"true":"false")+"}");
        std::cout << "RS13_FRAME_RECORDER_READY pid=" << selectedPid << std::endl;
        auto deadline = qpc() + static_cast<long long>(seconds) * frequency();
        FrameWriter writer;
        std::deque<Readback> pending;
        std::vector<ComPtr<ID3D11Texture2D>> recycled;
        std::map<std::string, ClipStorage> storage;
        int previousWidth = 0, previousHeight = 0;
        unsigned previousIndex = 0, queueRejected = 0, gpuRejected = 0;
        std::string previousFields;
        std::string clip; unsigned clips = 0, frames = 0;
        bool limited = false;
        std::vector<unsigned char> previous;
        std::string previousGeometry;
        auto consume = [&](bool blocking) {
            if (pending.empty()) return false;
            auto& ready = pending.front();
            D3D11_MAPPED_SUBRESOURCE mapped{};
            auto result = context->Map(ready.texture.Get(), 0, D3D11_MAP_READ, blocking ? 0 : D3D11_MAP_FLAG_DO_NOT_WAIT, &mapped);
            if (result == DXGI_ERROR_WAS_STILL_DRAWING) return false;
            check(result);
            std::vector<unsigned char> pixels(static_cast<size_t>(ready.width)*ready.height*4);
            for (int y=0;y<ready.height;y++) memcpy(pixels.data()+static_cast<size_t>(y)*ready.width*4,static_cast<unsigned char*>(mapped.pData)+static_cast<size_t>(y)*mapped.RowPitch,static_cast<size_t>(ready.width)*4);
            context->Unmap(ready.texture.Get(), 0);
            auto& count = storage[ready.clip];
            bool save = !ready.clip.empty() && count.frames < limit && count.bytes + pixels.size() <= 128ULL*1024*1024;
            if (save) {
                save = writer.submit(directory/ready.clip/("frame-"+std::to_string(ready.index)+".bmp"),pixels,ready.width,ready.height);
                if (save) { count.frames++; count.bytes += pixels.size(); } else { queueRejected++; }
            }
            if (!ready.clip.empty() && !save && (count.frames >= limit || count.bytes + pixels.size() > 128ULL*1024*1024)) limited = true;
            previousFields = ready.fields + ",\"observed_qpc\":" + std::to_string(qpc()) + ",\"changed\":" + (pixels != previous ? "true" : "false");
            if (!ready.clip.empty()) row("{\"event\":\"frame\",\"clip\":\""+ready.clip+"\""+previousFields+",\"saved\":"+(save?"true":"false")+"}");
            previousIndex = ready.index; previousWidth = ready.width; previousHeight = ready.height;
            previous = std::move(pixels); recycled.push_back(std::move(ready.texture)); pending.pop_front();
            return true;
        };
        while (qpc() < deadline && WaitForSingleObject(process, 0) == WAIT_TIMEOUT && IsWindow(selectedWindow)) {
            while (consume(false)) {}
            std::string next;
            {
                // Close immediately: the operator can replace the marker while
                // a frame waits for the GPU, rather than racing a long open file.
                std::ifstream command(directory / "clip.txt"); std::getline(command, next);
            }
            if (!next.empty() && next != clip) {
                if (next == "stop") break;
                if (next == "idle") { clip.clear(); }
                else {
                if (next.size() > 48 || !std::all_of(next.begin(), next.end(), [](char c) { return (c >= 'a' && c <= 'z') || (c >= '0' && c <= '9') || c == '-'; }) || ++clips > 40)
                    throw std::runtime_error("invalid_clip");
                if (!fs::create_directory(directory / next)) throw std::runtime_error("clip_exists");
                clip = next;
                row("{\"event\":\"clip_start\",\"clip\":\"" + clip + "\",\"qpc\":" + std::to_string(qpc()) + "}");
                if (!previous.empty() && writer.submit(directory/clip/("frame-"+std::to_string(previousIndex)+".bmp"), previous, previousWidth, previousHeight)) {
                    storage[clip].frames++; storage[clip].bytes += previous.size();
                    row("{\"event\":\"frame\",\"clip\":\""+clip+"\""+previousFields+",\"saved\":true,\"baseline_cached\":true}");
                }
                }
            }
            auto frame = pool.TryGetNextFrame();
            if (!frame) { Sleep(5); continue; }
            auto content = frame.ContentSize();
            if (content.Width != size.Width || content.Height != size.Height) {
                size = content; frame.Close(); pool.Recreate(captureDevice, DirectXPixelFormat::B8G8R8A8UIntNormalized, 3, size); previous.clear(); continue;
            }
            if (!IsWindowVisible(selectedWindow)) { frame.Close(); continue; }
            RECT client{}, window{}, extended{}; POINT clientOrigin{};
            GetClientRect(selectedWindow, &client); ClientToScreen(selectedWindow, &clientOrigin); GetWindowRect(selectedWindow, &window);
            bool extendedRead = SUCCEEDED(DwmGetWindowAttribute(selectedWindow,DWMWA_EXTENDED_FRAME_BOUNDS,&extended,sizeof(extended)));
            int frameX=0,frameY=0; bool mappedSurface=false;
            if(extendedRead && extended.right-extended.left==content.Width && extended.bottom-extended.top==content.Height){frameX=extended.left;frameY=extended.top;mappedSurface=true;}
            else if(window.right-window.left==content.Width && window.bottom-window.top==content.Height){frameX=window.left;frameY=window.top;mappedSurface=true;}
            else if(client.right==content.Width && client.bottom==content.Height){frameX=clientOrigin.x;frameY=clientOrigin.y;mappedSurface=true;}
            std::string geometry=std::to_string(content.Width)+","+std::to_string(content.Height)+","+std::to_string(client.right)+","+std::to_string(client.bottom);
            if(geometry!=previousGeometry){row("{\"event\":\"surface_geometry\",\"content_width\":"+std::to_string(content.Width)+",\"content_height\":"+std::to_string(content.Height)+",\"client_width\":"+std::to_string(client.right)+",\"client_height\":"+std::to_string(client.bottom)+",\"mapped\":"+(mappedSurface?"true":"false")+"}");previousGeometry=geometry;}
            // Map the client origin against an actually matching native frame
            // rectangle. Never assume Wails' invisible border is client pixels.
            if(!mappedSurface || content.Width>2048 || content.Height>2048){frame.Close();continue;}
            UINT dpi=GetDpiForWindow(selectedWindow);
            int inset = 12 * dpi / 96;
            int width = client.right - 2 * inset, height = client.bottom - 2 * inset;
            int cropX=clientOrigin.x-frameX+inset,cropY=clientOrigin.y-frameY+inset;
            if(cropX<0 || cropY<0 || cropX+width>content.Width || cropY+height>content.Height){frame.Close();continue;}
            if (width < 1 || height < 1) { frame.Close(); continue; }
            auto access = frame.Surface().as<::Windows::Graphics::DirectX::Direct3D11::IDirect3DDxgiInterfaceAccess>();
            ComPtr<ID3D11Texture2D> texture; check(access->GetInterface(IID_PPV_ARGS(&texture)));
            // Read an older copy without blocking the next captured surface.
            // Three slots bound GPU/CPU transfer storage; reject and log any
            // overflow instead of silently reporting a dense recording.
            if (pending.size() >= 3) {
                gpuRejected++;
                row("{\"event\":\"frame_drop\",\"reason\":\"gpu_readback_queue_full\",\"frame_time_100ns\":"+std::to_string(frame.SystemRelativeTime().count())+"}");
                frame.Close(); continue;
            }
            ComPtr<ID3D11Texture2D> staging;
            if (!recycled.empty()) { staging = std::move(recycled.back()); recycled.pop_back(); }
            D3D11_TEXTURE2D_DESC existing{};
            if (staging) staging->GetDesc(&existing);
            if (!staging || existing.Width != static_cast<UINT>(width) || existing.Height != static_cast<UINT>(height)) {
                D3D11_TEXTURE2D_DESC desc{}; texture->GetDesc(&desc);
                desc.Width = static_cast<UINT>(width); desc.Height = static_cast<UINT>(height); desc.MipLevels = 1; desc.ArraySize = 1;
                desc.Usage = D3D11_USAGE_STAGING; desc.BindFlags = 0; desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ; desc.MiscFlags = 0;
                staging.Reset(); check(device->CreateTexture2D(&desc, nullptr, &staging));
            }
            D3D11_BOX box{static_cast<UINT>(cropX),static_cast<UINT>(cropY),0,static_cast<UINT>(cropX+width),static_cast<UINT>(cropY+height),1};
            context->CopySubresourceRegion(staging.Get(),0,0,0,0,texture.Get(),0,&box);
            context->Flush();
            RECT afterClient{};POINT afterOrigin{};GetClientRect(selectedWindow,&afterClient);ClientToScreen(selectedWindow,&afterOrigin);
            if(afterClient.right!=client.right || afterClient.bottom!=client.bottom || afterOrigin.x!=clientOrigin.x || afterOrigin.y!=clientOrigin.y || GetDpiForWindow(selectedWindow)!=dpi){frame.Close();continue;}
            DWORD foregroundPid=0;GetWindowThreadProcessId(GetForegroundWindow(),&foregroundPid);
            auto fields = ",\"index\":"+std::to_string(frames)+",\"frame_time_100ns\":"+std::to_string(frame.SystemRelativeTime().count())+
                ",\"width\":"+std::to_string(width)+",\"height\":"+std::to_string(height)+",\"inset\":"+std::to_string(inset)+
                ",\"dpi\":"+std::to_string(dpi)+",\"crop_x\":"+std::to_string(cropX)+",\"crop_y\":"+std::to_string(cropY)+",\"foreground_target\":"+(foregroundPid==selectedPid?"true":"false");
            pending.push_back(Readback{std::move(staging), clip, std::move(fields), frames, width, height});
            frames++;frame.Close();
        }
        while (consume(true)) {}
        writer.finish();
        row("{\"event\":\"end\",\"qpc\":"+std::to_string(qpc())+",\"frames\":"+std::to_string(frames)+",\"clips\":"+std::to_string(clips)+",\"gpu_queue_rejected\":"+std::to_string(gpuRejected)+",\"write_queue_rejected\":"+std::to_string(queueRejected)+",\"storage_limited\":"+(limited?"true":"false")+"}");
        log.close(); CloseHandle(process); process=nullptr;
        // Some WebView/driver combinations block in WinRT capture teardown
        // after the target exits. This standalone read-only recorder owns no
        // persistent state; after flushing evidence, process exit releases all
        // capture/device objects without waiting on that cross-process teardown.
        ExitProcess(0);
    } catch (const std::exception& error) { std::cerr<<"RS13_WINDOW_RECORDER_ERROR "<<error.what()<<std::endl; }
    catch (const winrt::hresult_error& error) { std::cerr<<"RS13_WINDOW_RECORDER_ERROR "<<winrt::to_string(error.message())<<std::endl; }
    if(process)CloseHandle(process);return 1;
}
