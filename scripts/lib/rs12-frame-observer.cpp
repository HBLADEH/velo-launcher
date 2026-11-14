// External RS-12 observation only: no injected input, activation, or product writes.
// Desktop Duplication timestamps describe the composed desktop, not photon scanout.
#define NOMINMAX
#include <windows.h>
#include <d3d11.h>
#include <dxgi1_2.h>
#include <wrl/client.h>
#include <algorithm>
#include <atomic>
#include <cstdint>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <stdexcept>
#include <string>
#include <vector>

using Microsoft::WRL::ComPtr;
namespace fs = std::filesystem;

static long long ticks() { LARGE_INTEGER t; QueryPerformanceCounter(&t); return t.QuadPart; }
static long long frequency() { LARGE_INTEGER f; QueryPerformanceFrequency(&f); return f.QuadPart; }
static void check(HRESULT hr, const char* operation) {
    if (FAILED(hr)) throw std::runtime_error(std::string(operation) + ":" + std::to_string(static_cast<unsigned long>(hr)));
}
class File {
    HANDLE handle = INVALID_HANDLE_VALUE;
public:
    explicit File(const fs::path& path) {
        handle = CreateFileW(path.c_str(), GENERIC_WRITE, FILE_SHARE_READ, nullptr, CREATE_NEW, FILE_ATTRIBUTE_NORMAL, nullptr);
        if (handle == INVALID_HANDLE_VALUE) throw std::runtime_error("create_new_evidence_file");
    }
    File(const File&) = delete;
    File& operator=(const File&) = delete;
    ~File() { if (handle != INVALID_HANDLE_VALUE) CloseHandle(handle); }
    void write(const void* data, size_t bytes) {
        DWORD written = 0;
        if (bytes > MAXDWORD || !WriteFile(handle, data, static_cast<DWORD>(bytes), &written, nullptr) || written != bytes)
            throw std::runtime_error("write_evidence_file");
    }
    void row(const std::string& text) { auto line = text + "\n"; write(line.data(), line.size()); }
};
static File* trace = nullptr;
static HWND target = nullptr;
static DWORD targetPid = 0;
static HANDLE targetProcess = nullptr;
static bool calibration = false;
static bool resizeCalibration = false;
static bool stopped = false;
static unsigned sequence = 0;
static unsigned savedFrames = 0;
static unsigned frameCount = 0;
static unsigned decodedFrames = 0;
static unsigned rejectedFrames = 0;
static uint64_t savedBytes = 0;
static bool saveBudgetExhausted = false;
static bool hookFailed = false;
static long long tickQuantumQpc = 0;
static long long inputArmQpc = 0;
static long long nextImageQpc = 0;
static unsigned armImages = 0;

static bool owned(HWND window) {
    DWORD pid = 0;
    return window && GetWindowThreadProcessId(window, &pid) != 0 && pid == targetPid;
}
static bool foregroundOwned() { return owned(GetForegroundWindow()); }
static bool targetExists() { return owned(target) && targetProcess && WaitForSingleObject(targetProcess, 0) == WAIT_TIMEOUT; }
static uint64_t filetimeValue(FILETIME value) { ULARGE_INTEGER n{}; n.LowPart = value.dwLowDateTime; n.HighPart = value.dwHighDateTime; return n.QuadPart; }
static uint64_t cpuTime() {
    FILETIME created{}, exited{}, kernel{}, user{};
    if (!GetProcessTimes(GetCurrentProcess(), &created, &exited, &kernel, &user)) throw std::runtime_error("observer_cpu_time");
    return filetimeValue(kernel) + filetimeValue(user);
}
static void clockPair(const char* kind) {
    FILETIME utc{};
    auto before = ticks();
    GetSystemTimePreciseAsFileTime(&utc);
    auto after = ticks();
    ULARGE_INTEGER value{}; value.LowPart = utc.dwLowDateTime; value.HighPart = utc.dwHighDateTime;
    trace->row("{\"event\":\"clock\",\"kind\":\"" + std::string(kind) + "\",\"before_qpc\":" + std::to_string(before) +
        ",\"after_qpc\":" + std::to_string(after) + ",\"utc_filetime_100ns\":" + std::to_string(value.QuadPart) + "}");
}
static void state(const char* kind) {
    HWND foreground = GetForegroundWindow();
    DWORD pid = 0;
    bool valid = GetWindowThreadProcessId(foreground, &pid) != 0;
    bool rootOwned = owned(GetAncestor(foreground, GA_ROOTOWNER));
    trace->row("{\"event\":\"native\",\"kind\":\"" + std::string(kind) + "\",\"qpc\":" + std::to_string(ticks()) +
        ",\"visible\":" + (IsWindowVisible(target) ? "true" : "false") +
        ",\"foreground_target\":" + (valid && pid == targetPid ? "true" : "false") +
        ",\"foreground_root_owned\":" + (rootOwned ? "true" : "false") + "}");
}
static void CALLBACK nativeEvent(HWINEVENTHOOK, DWORD event, HWND window, LONG object, LONG child, DWORD, DWORD) {
    try {
        if (event == EVENT_SYSTEM_FOREGROUND) state("foreground");
        else if (window == target && object == OBJID_WINDOW && child == CHILDID_SELF)
            state(event == EVENT_OBJECT_SHOW ? "show" : "hide");
    } catch (...) { hookFailed = true; stopped = true; }
}
static LRESULT CALLBACK keyboard(int code, WPARAM message, LPARAM parameter) {
    if (code == HC_ACTION && (message == WM_KEYDOWN || message == WM_SYSKEYDOWN)) {
        auto* key = reinterpret_cast<KBDLLHOOKSTRUCT*>(parameter);
        bool altSpace = key->vkCode == VK_SPACE && (key->flags & LLKHF_ALTDOWN) != 0;
        bool fixedInput = (key->vkCode == 'A' || key->vkCode == VK_BACK || key->vkCode == VK_ESCAPE) && foregroundOwned();
        if (altSpace || fixedInput) {
            try {
                auto observed = ticks();
                if (key->vkCode == 'A' && (key->flags & (LLKHF_INJECTED | LLKHF_ALTDOWN)) == 0 &&
                    GetAsyncKeyState(VK_CONTROL) >= 0 && GetAsyncKeyState(VK_SHIFT) >= 0 && foregroundOwned()) {
                    inputArmQpc = observed; nextImageQpc = observed; armImages = 0;
                }
                auto nowMs = static_cast<DWORD>(GetTickCount64());
                auto ageMs = static_cast<DWORD>(nowMs - key->time);
                auto lower = observed - static_cast<long long>(ageMs) * frequency() / 1000 - tickQuantumQpc;
                const char* kind = altSpace ? "alt_space" : key->vkCode == 'A' ? "fixed_a" : key->vkCode == VK_BACK ? "backspace" : "escape";
                trace->row("{\"event\":\"key\",\"kind\":\"" + std::string(kind) + "\",\"qpc\":" + std::to_string(observed) +
                    ",\"event_lower_qpc\":" + std::to_string(lower) + ",\"injected\":" + ((key->flags & LLKHF_INJECTED) ? "true" : "false") +
                    ",\"control\":" + (GetAsyncKeyState(VK_CONTROL) < 0 ? "true" : "false") +
                    ",\"shift\":" + (GetAsyncKeyState(VK_SHIFT) < 0 ? "true" : "false") +
                    ",\"alt\":" + ((key->flags & LLKHF_ALTDOWN) ? "true" : "false") +
                    ",\"visible\":" + (IsWindowVisible(target) ? "true" : "false") + ",\"foreground_target\":" + (foregroundOwned() ? "true" : "false") + "}");
            } catch (...) { hookFailed = true; stopped = true; }
        }
    }
    return CallNextHookEx(nullptr, code, message, parameter);
}
static HWND findWindow(DWORD pid, const wchar_t* wantedClass) {
    struct Lookup { DWORD pid; const wchar_t* cls; HWND found; int count; } lookup{pid, wantedClass, nullptr, 0};
    EnumWindows([](HWND window, LPARAM parameter) -> BOOL {
        auto& query = *reinterpret_cast<Lookup*>(parameter);
        DWORD owner = 0; wchar_t cls[128]{};
        if (GetWindowThreadProcessId(window, &owner) != 0 && owner == query.pid &&
            GetClassNameW(window, cls, 128) > 0 && wcscmp(cls, query.cls) == 0) { query.found = window; query.count++; }
        return TRUE;
    }, reinterpret_cast<LPARAM>(&lookup));
    if (lookup.count != 1) throw std::runtime_error("expected_one_target_window");
    return lookup.found;
}
static bool unoccluded(const RECT& bounds) {
    for (HWND above = GetWindow(target, GW_HWNDPREV); above; above = GetWindow(above, GW_HWNDPREV)) {
        if (!IsWindowVisible(above) || IsIconic(above)) continue;
        RECT other{}, intersection{};
        if (GetWindowRect(above, &other) && IntersectRect(&intersection, &bounds, &other) &&
            intersection.right > intersection.left && intersection.bottom > intersection.top) return false;
    }
    return true;
}
static uint64_t hashPixels(const std::vector<unsigned char>& pixels, int width, int height, int skipTop) {
    uint64_t hash = 14695981039346656037ULL;
    for (int y = std::min(skipTop, height); y < height; y++)
        for (int x = 0; x < width * 4; x++) { hash ^= pixels[static_cast<size_t>(y) * width * 4 + x]; hash *= 1099511628211ULL; }
    return hash;
}
static void bitmap(const fs::path& path, const std::vector<unsigned char>& pixels, int width, int height) {
    BITMAPFILEHEADER file{}; file.bfType = 0x4d42; file.bfOffBits = sizeof(file) + sizeof(BITMAPINFOHEADER);
    file.bfSize = file.bfOffBits + static_cast<DWORD>(pixels.size());
    BITMAPINFOHEADER info{}; info.biSize = sizeof(info); info.biWidth = width; info.biHeight = -height;
    info.biPlanes = 1; info.biBitCount = 32; info.biCompression = BI_RGB;
    File output(path); output.write(&file, sizeof(file)); output.write(&info, sizeof(info)); output.write(pixels.data(), pixels.size());
}
static LRESULT CALLBACK patternWindow(HWND window, UINT message, WPARAM wparam, LPARAM lparam) {
    if (message == WM_TIMER) {
        sequence++;
        if (resizeCalibration && sequence == 20) SetWindowPos(window, nullptr, 160, 160, 520, 340, SWP_NOZORDER | SWP_NOACTIVATE);
        if (resizeCalibration && sequence == 40) SetWindowPos(window, nullptr, 520, 200, 420, 280, SWP_NOZORDER | SWP_NOACTIVATE);
        if (resizeCalibration && sequence == 60) SetWindowPos(window, nullptr, 160, 160, 420, 300, SWP_NOZORDER | SWP_NOACTIVATE);
        InvalidateRect(window, nullptr, FALSE); return 0;
    }
    if (message == WM_PAINT) {
        PAINTSTRUCT paint{}; auto begin = ticks(); HDC dc = BeginPaint(window, &paint);
        RECT client{}; GetClientRect(window, &client);
        HBRUSH brush = CreateSolidBrush(RGB(72, 80 + sequence % 80, 112)); FillRect(dc, &client, brush); DeleteObject(brush);
        for (int bit = 0; bit < 8; bit++) {
            RECT block{16 + bit * 16, 16, 28 + bit * 16, 28};
            FillRect(dc, &block, static_cast<HBRUSH>(GetStockObject((sequence & (1U << bit)) ? WHITE_BRUSH : BLACK_BRUSH)));
        }
        EndPaint(window, &paint);
        try {
            if (trace) trace->row("{\"event\":\"pattern_paint\",\"sequence\":" + std::to_string(sequence) +
                ",\"begin_qpc\":" + std::to_string(begin) + ",\"end_qpc\":" + std::to_string(ticks()) + "}");
        } catch (...) { hookFailed = true; stopped = true; }
        return 0;
    }
    if (message == WM_CLOSE) { stopped = true; return 0; }
    return DefWindowProcW(window, message, wparam, lparam);
}
static void ensureDesktop() {
    HDESK desktop = OpenInputDesktop(0, FALSE, DESKTOP_READOBJECTS);
    if (!desktop) throw std::runtime_error("input_desktop_unavailable_or_locked");
    wchar_t name[128]{}; DWORD needed = 0;
    bool read = GetUserObjectInformationW(desktop, UOI_NAME, name, sizeof(name), &needed) != FALSE;
    CloseDesktop(desktop);
    if (!read || _wcsicmp(name, L"Default") != 0) throw std::runtime_error("interactive_desktop_not_available");
}
static void capture(int seconds, int saveLimit, const fs::path& directory) {
    ComPtr<IDXGIFactory1> factory; check(CreateDXGIFactory1(IID_PPV_ARGS(&factory)), "create_dxgi_factory");
    RECT bounds{}; if (!GetWindowRect(target, &bounds)) throw std::runtime_error("target_bounds");
    HMONITOR monitor = MonitorFromWindow(target, MONITOR_DEFAULTTONEAREST);
    ComPtr<IDXGIAdapter1> adapter; ComPtr<IDXGIOutput1> output;
    DXGI_OUTPUT_DESC outputDesc{};
    for (UINT a = 0; !output; a++) {
        ComPtr<IDXGIAdapter1> candidate;
        if (factory->EnumAdapters1(a, &candidate) == DXGI_ERROR_NOT_FOUND) break;
        for (UINT n = 0; n < 16; n++) {
            ComPtr<IDXGIOutput> current;
            if (candidate->EnumOutputs(n, &current) == DXGI_ERROR_NOT_FOUND) break;
            DXGI_OUTPUT_DESC desc{}; check(current->GetDesc(&desc), "output_description");
            if (desc.Monitor == monitor) { adapter = candidate; check(current.As(&output), "output_interface"); outputDesc = desc; break; }
        }
    }
    if (!output) throw std::runtime_error("target_monitor_not_found");
    if (outputDesc.Rotation != DXGI_MODE_ROTATION_IDENTITY) throw std::runtime_error("rotated_output_not_supported");
    ComPtr<ID3D11Device> device; ComPtr<ID3D11DeviceContext> context;
    D3D_FEATURE_LEVEL level{};
    check(D3D11CreateDevice(adapter.Get(), D3D_DRIVER_TYPE_UNKNOWN, nullptr, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
        nullptr, 0, D3D11_SDK_VERSION, &device, &level, &context), "create_d3d_device");
    ComPtr<IDXGIOutputDuplication> duplication;
    check(output->DuplicateOutput(device.Get(), &duplication), "duplicate_output");
    ComPtr<ID3D11Texture2D> staging; int oldWidth = 0, oldHeight = 0;
    RECT previousBounds{}; bool boundsReady = false; long long geometryObservedQpc = 0;
    uint64_t lastHash = 0; bool hashReady = false;
    auto deadline = ticks() + static_cast<long long>(seconds) * frequency();
    auto lastClock = ticks();
    FILETIME created{}, exited{}, kernel{}, user{};
    if (!GetProcessTimes(targetProcess, &created, &exited, &kernel, &user)) throw std::runtime_error("target_process_creation_time");
    trace->row("{\"event\":\"capture_ready\",\"qpc\":" + std::to_string(ticks()) + ",\"qpc_frequency\":" + std::to_string(frequency()) +
        ",\"pid\":" + std::to_string(targetPid) + ",\"creation_filetime_100ns\":" + std::to_string(filetimeValue(created)) +
        ",\"capture_schema\":2,\"geometry_freshness_checked\":true" +
        ",\"calibration\":" + (calibration ? "true" : "false") + "}");
    std::cout << "RS12_FRAME_OBSERVER_READY pid=" << targetPid << std::endl;
    while (!stopped && ticks() < deadline && targetExists()) {
        MSG message{}; while (PeekMessageW(&message, nullptr, 0, 0, PM_REMOVE)) { TranslateMessage(&message); DispatchMessageW(&message); }
        if (hookFailed) throw std::runtime_error("native_callback_write_failed");
        if (ticks() - lastClock >= frequency()) { clockPair("periodic"); lastClock = ticks(); }
        DXGI_OUTDUPL_FRAME_INFO info{}; ComPtr<IDXGIResource> resource;
        HRESULT result = duplication->AcquireNextFrame(5, &info, &resource);
        if (result == DXGI_ERROR_WAIT_TIMEOUT) continue;
        check(result, "acquire_desktop_frame");
        try {
            // Read only the selected HWND's crop. No full-desktop pixels are saved.
            if (!IsWindowVisible(target) || !GetWindowRect(target, &bounds) || !unoccluded(bounds) || info.LastPresentTime.QuadPart == 0) { rejectedFrames++; duplication->ReleaseFrame(); continue; }
            RECT client{}; POINT clientOrigin{};
            if (!GetClientRect(target, &client) || !ClientToScreen(target, &clientOrigin)) throw std::runtime_error("target_client_bounds");
            // Exclude native borders and rounded transparent corners; the crop
            // must not include another application behind the selected HWND.
            int inset = 12 * GetDpiForWindow(target) / 96;
            bounds = RECT{clientOrigin.x + inset, clientOrigin.y + inset,
                clientOrigin.x + client.right - inset, clientOrigin.y + client.bottom - inset};
            if (!boundsReady || !EqualRect(&previousBounds, &bounds)) {
                previousBounds = bounds; boundsReady = true; hashReady = false; geometryObservedQpc = ticks();
                trace->row("{\"event\":\"geometry_observed\",\"qpc\":" + std::to_string(geometryObservedQpc) +
                    ",\"width\":" + std::to_string(bounds.right - bounds.left) + ",\"height\":" + std::to_string(bounds.bottom - bounds.top) + "}");
                // The acquired desktop frame may predate the new HWND geometry.
                // Keep it out instead of cropping an old surface at new bounds.
                rejectedFrames++; duplication->ReleaseFrame(); continue;
            }
            if (info.LastPresentTime.QuadPart < geometryObservedQpc) { rejectedFrames++; duplication->ReleaseFrame(); continue; }
            auto screen = outputDesc.DesktopCoordinates;
            if (bounds.left < screen.left || bounds.top < screen.top || bounds.right > screen.right || bounds.bottom > screen.bottom) {
                rejectedFrames++; duplication->ReleaseFrame(); continue;
            }
            int width = bounds.right - bounds.left, height = bounds.bottom - bounds.top;
            if (width <= 0 || height <= 0 || width > 2048 || height > 2048) throw std::runtime_error("target_crop_out_of_bounds");
            ComPtr<ID3D11Texture2D> texture; check(resource.As(&texture), "frame_texture");
            D3D11_TEXTURE2D_DESC sourceDesc{}; texture->GetDesc(&sourceDesc);
            if (sourceDesc.Format != DXGI_FORMAT_B8G8R8A8_UNORM) throw std::runtime_error("unsupported_desktop_pixel_format");
            if (width != oldWidth || height != oldHeight) {
                D3D11_TEXTURE2D_DESC desc{}; texture->GetDesc(&desc);
                desc.Width = width; desc.Height = height; desc.MipLevels = 1; desc.ArraySize = 1;
                desc.Usage = D3D11_USAGE_STAGING; desc.BindFlags = 0; desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ; desc.MiscFlags = 0;
                staging.Reset(); check(device->CreateTexture2D(&desc, nullptr, &staging), "create_crop_staging");
                oldWidth = width; oldHeight = height; hashReady = false;
            }
            D3D11_BOX box{static_cast<UINT>(bounds.left - screen.left), static_cast<UINT>(bounds.top - screen.top), 0,
                static_cast<UINT>(bounds.right - screen.left), static_cast<UINT>(bounds.bottom - screen.top), 1};
            context->CopySubresourceRegion(staging.Get(), 0, 0, 0, 0, texture.Get(), 0, &box);
            D3D11_MAPPED_SUBRESOURCE mapped{}; check(context->Map(staging.Get(), 0, D3D11_MAP_READ, 0, &mapped), "map_crop");
            std::vector<unsigned char> pixels(static_cast<size_t>(width) * height * 4);
            for (int y = 0; y < height; y++) memcpy(pixels.data() + static_cast<size_t>(y) * width * 4,
                static_cast<unsigned char*>(mapped.pData) + static_cast<size_t>(y) * mapped.RowPitch, static_cast<size_t>(width) * 4);
            context->Unmap(staging.Get(), 0);
            RECT afterClient{}, afterBounds{}; POINT afterOrigin{};
            if (!GetClientRect(target, &afterClient) || !ClientToScreen(target, &afterOrigin) || !IsWindowVisible(target)) {
                rejectedFrames++; duplication->ReleaseFrame(); continue;
            }
            afterBounds = RECT{afterOrigin.x + inset, afterOrigin.y + inset,
                afterOrigin.x + afterClient.right - inset, afterOrigin.y + afterClient.bottom - inset};
            if (!EqualRect(&afterBounds, &bounds) || !unoccluded(afterBounds)) {
                boundsReady = false; rejectedFrames++; duplication->ReleaseFrame(); continue;
            }
            auto observed = ticks();
            auto hash = hashPixels(pixels, width, height, 0);
            auto resultsHash = hashPixels(pixels, width, height, 150 * GetDpiForWindow(target) / 96 - inset);
            bool changed = !hashReady || hash != lastHash;
            // Reserve local image storage for the actual fixed-key rounds.
            // Idle caret blinking must not consume the budget before testing.
            bool imageDue = changed && (calibration || (inputArmQpc > 0 && info.LastPresentTime.QuadPart >= nextImageQpc &&
                info.LastPresentTime.QuadPart - inputArmQpc <= frequency() && armImages < 6));
            bool saveImage = imageDue && savedFrames < static_cast<unsigned>(saveLimit) && savedBytes + pixels.size() <= 128ULL * 1024 * 1024;
            int pattern = -1;
            if (calibration) {
                POINT origin{}; ClientToScreen(target, &origin);
                int x0 = origin.x - bounds.left, y0 = origin.y - bounds.top;
                pattern = 0;
                for (int bit = 0; bit < 8; bit++) {
                    int x = x0 + 22 + bit * 16, y = y0 + 22;
                    if (x < 0 || x >= width || y < 0 || y >= height) { pattern = -1; break; }
                    auto at = (static_cast<size_t>(y) * width + x) * 4;
                    if (pixels[at] + pixels[at + 1] + pixels[at + 2] > 3 * 128) pattern |= 1 << bit;
                }
                if (pattern > 0) decodedFrames++;
            }
            trace->row("{\"event\":\"frame\",\"present_qpc\":" + std::to_string(info.LastPresentTime.QuadPart) +
                ",\"observed_qpc\":" + std::to_string(observed) + ",\"accumulated_frames\":" + std::to_string(info.AccumulatedFrames) +
                ",\"geometry_observed_qpc\":" + std::to_string(geometryObservedQpc) +
                ",\"width\":" + std::to_string(width) + ",\"height\":" + std::to_string(height) + ",\"hash\":\"" + std::to_string(hash) +
                "\",\"results_hash\":\"" + std::to_string(resultsHash) + "\",\"changed\":" + (changed ? "true" : "false") +
                ",\"foreground_target\":" + (foregroundOwned() ? "true" : "false") + ",\"pattern\":" + std::to_string(pattern) +
                ",\"image_index\":" + (saveImage ? std::to_string(frameCount) : "-1") + ",\"input_arm_qpc\":" + std::to_string(inputArmQpc) + "}");
            if (saveImage) {
                bitmap(directory / ("frame-" + std::to_string(frameCount) + ".bmp"), pixels, width, height);
                savedFrames++; savedBytes += pixels.size();
                if (!calibration) { armImages++; nextImageQpc = info.LastPresentTime.QuadPart + frequency() / 10; }
            } else if (imageDue && savedFrames < static_cast<unsigned>(saveLimit) && savedBytes + pixels.size() > 128ULL * 1024 * 1024) {
                saveBudgetExhausted = true;
            }
            lastHash = hash; hashReady = true; frameCount++;
            duplication->ReleaseFrame();
        } catch (...) { duplication->ReleaseFrame(); throw; }
    }
}
int wmain(int argc, wchar_t** argv) {
    HHOOK keys = nullptr; HWINEVENTHOOK foreground = nullptr, visibility = nullptr;
    try {
        auto observerStart = ticks(); auto cpuStart = cpuTime();
        if (argc != 6) throw std::runtime_error("usage_mode_pid_seconds_save_limit_new_directory");
        resizeCalibration = wcscmp(argv[1], L"calibrate-resize") == 0;
        calibration = wcscmp(argv[1], L"calibrate") == 0 || resizeCalibration;
        bool wails = wcscmp(argv[1], L"wails") == 0;
        if (!calibration && !wails && wcscmp(argv[1], L"slint") != 0) throw std::runtime_error("unsupported_mode");
        targetPid = std::stoul(argv[2]); int seconds = std::stoi(argv[3]); int saveLimit = std::stoi(argv[4]);
        if (seconds < 1 || seconds > 900 || saveLimit < 0 || saveLimit > 128) throw std::runtime_error("invalid_capture_limits");
        if (calibration && seconds > 3) throw std::runtime_error("calibration_maximum_three_seconds");
        ensureDesktop(); SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        fs::path directory(argv[5]);
        if (!fs::create_directory(directory)) throw std::runtime_error("evidence_directory_already_exists");
        File log(directory / "events.jsonl"); trace = &log;
        clockPair("start");
        if (calibration) {
            targetPid = GetCurrentProcessId();
            WNDCLASSW cls{}; cls.lpfnWndProc = patternWindow; cls.hInstance = GetModuleHandleW(nullptr); cls.lpszClassName = L"VeloRS12FrameCalibration";
            if (!RegisterClassW(&cls)) throw std::runtime_error("register_calibration_class");
            target = CreateWindowExW(WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW, cls.lpszClassName, L"Velo RS-12 Frame Calibration",
                WS_OVERLAPPEDWINDOW, 160, 160, 420, 280, nullptr, nullptr, cls.hInstance, nullptr);
            if (!target) throw std::runtime_error("create_calibration_window");
            ShowWindow(target, SW_SHOWNOACTIVATE); SetTimer(target, 1, 16, nullptr);
        } else target = findWindow(targetPid, wails ? L"VeloLauncherWindow" : L"VeloSlintLauncherWindow");
        targetProcess = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE, FALSE, targetPid);
        if (!targetProcess) throw std::runtime_error("open_target_process_identity");
        DWORD adjustment = 0, increment = 0; BOOL disabled = FALSE;
        if (!GetSystemTimeAdjustment(&adjustment, &increment, &disabled)) throw std::runtime_error("keyboard_clock_quantum");
        tickQuantumQpc = (static_cast<long long>(increment) * frequency() + 9999999) / 10000000;
        if (!calibration) {
            keys = SetWindowsHookExW(WH_KEYBOARD_LL, keyboard, GetModuleHandleW(nullptr), 0);
            foreground = SetWinEventHook(EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND, nullptr, nativeEvent, 0, 0, WINEVENT_OUTOFCONTEXT);
            visibility = SetWinEventHook(EVENT_OBJECT_SHOW, EVENT_OBJECT_HIDE, nullptr, nativeEvent, targetPid, 0, WINEVENT_OUTOFCONTEXT);
            if (!keys || !foreground || !visibility) throw std::runtime_error("install_passive_hooks");
            state("attached");
        }
        capture(seconds, saveLimit, directory);
        if (keys) { UnhookWindowsHookEx(keys); keys = nullptr; }
        if (foreground) { UnhookWinEvent(foreground); foreground = nullptr; }
        if (visibility) { UnhookWinEvent(visibility); visibility = nullptr; }
        clockPair("end");
        trace->row("{\"event\":\"end\",\"qpc\":" + std::to_string(ticks()) + ",\"frames\":" + std::to_string(frameCount) +
            ",\"saved_frames\":" + std::to_string(savedFrames) + ",\"decoded_frames\":" + std::to_string(decodedFrames) +
            ",\"saved_bytes\":" + std::to_string(savedBytes) + ",\"save_budget_exhausted\":" + (saveBudgetExhausted ? "true" : "false") +
            ",\"observer_cpu_100ns\":" + std::to_string(cpuTime() - cpuStart) + ",\"observer_elapsed_qpc\":" + std::to_string(ticks() - observerStart) +
            ",\"rejected_frames\":" + std::to_string(rejectedFrames) + ",\"target_ended\":" + (!targetExists() ? "true" : "false") + "}");
        if (calibration) DestroyWindow(target);
        CloseHandle(targetProcess); targetProcess = nullptr;
        if (calibration && decodedFrames < 20) throw std::runtime_error("insufficient_decoded_calibration_frames");
        return 0;
    } catch (const std::exception& error) {
        trace = nullptr;
        if (keys) UnhookWindowsHookEx(keys);
        if (foreground) UnhookWinEvent(foreground);
        if (visibility) UnhookWinEvent(visibility);
        if (calibration && target) DestroyWindow(target);
        if (targetProcess) CloseHandle(targetProcess);
        std::cerr << "RS12_FRAME_OBSERVER_ERROR " << error.what() << std::endl;
        return 1;
    }
}
