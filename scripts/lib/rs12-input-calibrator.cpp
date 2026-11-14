// Isolated native integration test. Machine-generated input is never physical evidence.
// Emits fixed 'a' only while the exact recorded Velo HWND owns foreground/focus.
#define NOMINMAX
#include <windows.h>
#include <algorithm>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <sstream>
#include <stdexcept>
#include <string>
#include <map>

namespace fs = std::filesystem;
static HANDLE output = INVALID_HANDLE_VALUE;
static HANDLE process = nullptr;
static HWND target = nullptr;
static DWORD pid = 0;
static uint64_t expectedCreation = 0;
static std::wstring edition;
static HANDLE preciseTimer=nullptr;
static HWINEVENTHOOK foregroundHook=nullptr;
static bool callbackFailed=false;
static long long frequency() { LARGE_INTEGER x; QueryPerformanceFrequency(&x); return x.QuadPart; }
static long long qpc() { LARGE_INTEGER x; QueryPerformanceCounter(&x); return x.QuadPart; }
static uint64_t filetime(FILETIME x) { ULARGE_INTEGER n{}; n.LowPart=x.dwLowDateTime; n.HighPart=x.dwHighDateTime; return n.QuadPart; }
static void probePause(){
    if(preciseTimer){LARGE_INTEGER due{};due.QuadPart=-20000;if(!SetWaitableTimer(preciseTimer,&due,0,nullptr,nullptr,FALSE)||WaitForSingleObject(preciseTimer,50)!=WAIT_OBJECT_0)throw std::runtime_error("precise_probe_timer_failed");}
    else Sleep(2);
}
static void row(const std::string& line) {
    auto text=line+"\n"; DWORD n=0;
    if(!WriteFile(output,text.data(),static_cast<DWORD>(text.size()),&n,nullptr)||n!=text.size())throw std::runtime_error("write_trace");
}
static void clockPair(const char* kind) {
    FILETIME now{};auto before=qpc();GetSystemTimePreciseAsFileTime(&now);auto after=qpc();
    row("{\"event\":\"clock\",\"kind\":\""+std::string(kind)+"\",\"before_qpc\":"+std::to_string(before)+",\"after_qpc\":"+std::to_string(after)+",\"utc_filetime_100ns\":"+std::to_string(filetime(now))+"}");
}
static bool owner(HWND window) { DWORD p=0;return window&&GetWindowThreadProcessId(window,&p)!=0&&p==pid; }
static bool alive() { return process&&WaitForSingleObject(process,0)==WAIT_TIMEOUT; }
static bool modifiersClear() { for(int key:{VK_CONTROL,VK_SHIFT,VK_MENU,VK_LWIN,VK_RWIN})if(GetAsyncKeyState(key)<0)return false;return true; }
static bool editableHost() {
    if(!alive()||!owner(target)||!IsWindowVisible(target)||GetForegroundWindow()!=target)return false;
    DWORD thread=GetWindowThreadProcessId(target,nullptr);GUITHREADINFO info{};info.cbSize=sizeof(info);
    if(!GetGUIThreadInfo(thread,&info)||!info.hwndFocus)return false;
    return info.hwndFocus==target||IsChild(target,info.hwndFocus);
}
static void nativeFocus(const char* kind){
    HWND foreground=GetForegroundWindow();DWORD foregroundPid=0;DWORD thread=GetWindowThreadProcessId(foreground,&foregroundPid);
    GUITHREADINFO info{};info.cbSize=sizeof(info);bool got=thread&&GetGUIThreadInfo(thread,&info);
    HWND root=GetAncestor(foreground,GA_ROOTOWNER);
    const char* category="other";
    HANDLE ownerProcess=OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION,FALSE,foregroundPid);
    if(ownerProcess){wchar_t path[32768]{};DWORD n=32768;if(QueryFullProcessImageNameW(ownerProcess,0,path,&n)){
        auto basename=fs::path(path).filename().wstring();
        if(_wcsicmp(basename.c_str(),L"Codex.exe")==0)category="test_host";
        else if(_wcsicmp(basename.c_str(),L"explorer.exe")==0)category="desktop_shell";
        else if(_wcsicmp(basename.c_str(),L"TextInputHost.exe")==0||_wcsicmp(basename.c_str(),L"ctfmon.exe")==0)category="ime_host";
    }CloseHandle(ownerProcess);}
    if(foregroundPid==pid&&pid)category="target";
    row("{\"event\":\"focus_state\",\"kind\":\""+std::string(kind)+"\",\"qpc\":"+std::to_string(qpc())+",\"foreground_category\":\""+category+"\",\"foreground_target\":"+(owner(foreground)?"true":"false")+",\"foreground_root_target\":"+(owner(root)?"true":"false")+",\"focus_available\":"+(got?"true":"false")+",\"focus_in_target\":"+((got&&owner(info.hwndFocus))?"true":"false")+",\"target_visible\":"+((target&&IsWindowVisible(target))?"true":"false")+"}");
}
static void CALLBACK onForeground(HWINEVENTHOOK,DWORD,HWND,LONG,LONG,DWORD,DWORD){try{nativeFocus("foreground_event");}catch(...){callbackFailed=true;}}
static void pump(){MSG message{};while(PeekMessageW(&message,nullptr,0,0,PM_REMOVE)){TranslateMessage(&message);DispatchMessageW(&message);}if(callbackFailed)throw std::runtime_error("focus_callback_failed");}
static HWND findTarget() {
    struct Find{HWND window=nullptr;int count=0;} found;
    EnumWindows([](HWND window,LPARAM value)->BOOL{
        auto& result=*reinterpret_cast<Find*>(value);wchar_t cls[128]{};
        if(owner(window)&&GetClassNameW(window,cls,128)>0&&wcscmp(cls,edition==L"wails"?L"VeloLauncherWindow":L"VeloSlintLauncherWindow")==0){result.window=window;result.count++;}return TRUE;
    },reinterpret_cast<LPARAM>(&found));
    if(found.count>1)throw std::runtime_error("ambiguous_target_hwnd");return found.window;
}
static std::string readShared(const fs::path& path) {
    HANDLE file=CreateFileW(path.c_str(),GENERIC_READ,FILE_SHARE_READ|FILE_SHARE_WRITE|FILE_SHARE_DELETE,nullptr,OPEN_EXISTING,FILE_ATTRIBUTE_NORMAL,nullptr);
    if(file==INVALID_HANDLE_VALUE){if(GetLastError()==ERROR_FILE_NOT_FOUND)return "";throw std::runtime_error("open_input_log");}
    LARGE_INTEGER size{};if(!GetFileSizeEx(file,&size)||size.QuadPart>4*1024*1024){CloseHandle(file);throw std::runtime_error("input_log_limit");}
    std::string text(static_cast<size_t>(size.QuadPart),'\0');DWORD n=0;BOOL ok=ReadFile(file,text.data(),static_cast<DWORD>(text.size()),&n,nullptr);CloseHandle(file);
    if(!ok)throw std::runtime_error("read_input_log");text.resize(n);auto last=text.find_last_of('\n');if(last==std::string::npos)return "";text.resize(last+1);return text;
}
static std::map<std::string,std::string> fields(const std::string& line){
    std::map<std::string,std::string> result;std::istringstream stream(line);std::string part;
    while(stream>>part){auto equals=part.find('=');if(equals!=std::string::npos)result.emplace(part.substr(0,equals),part.substr(equals+1));}return result;
}
static uint64_t accepted(const fs::path& log,uint64_t lowerUnixUs,size_t skipLines){
    std::istringstream stream(readShared(log));std::string line;uint64_t first=0;size_t index=0;
    while(std::getline(stream,line)){
        if(index++<skipLines)continue;
        auto f=fields(line);if(f["pid"]!=std::to_string(pid))continue;
        if(f["dropped"]!="0")throw std::runtime_error("product_event_loss");
        if(f["event"]=="input_accepted"&&f["focused"]=="true"&&f["native_focused"]=="true"&&f["visible"]=="true"&&f["composing"]=="false"&&f["synthetic"]!="true"&&f["trusted"]!="false"&&f["utf8_bytes"]!="0"&&!f["utf8_bytes"].empty()){
            auto time=std::stoull(f["unix_us"]);if(time>=lowerUnixUs&&(!first||time<first))first=time;
        }
    }return first;
}
static bool sendUnicode(bool strict=true){
    if(!editableHost()||!modifiersClear()){if(strict)throw std::runtime_error("target_focus_or_modifiers_changed");return false;}
    INPUT keys[2]{};for(auto& key:keys){key.type=INPUT_KEYBOARD;key.ki.wScan=L'a';key.ki.dwFlags=KEYEVENTF_UNICODE;key.ki.dwExtraInfo=0x56454c4f52533132ULL;}
    keys[1].ki.dwFlags|=KEYEVENTF_KEYUP;
    if(SendInput(2,keys,sizeof(INPUT))!=2)throw std::runtime_error("send_input_failed_no_retry");
    return true;
}
static void hotkey(){
    if(!alive()||!owner(target)||IsWindowVisible(target)||!modifiersClear())throw std::runtime_error("hotkey_requires_hidden_exact_target_and_clear_modifiers");
    INPUT keys[4]{};WORD codes[4]={VK_MENU,VK_SPACE,VK_SPACE,VK_MENU};
    for(int i=0;i<4;i++){keys[i].type=INPUT_KEYBOARD;keys[i].ki.wVk=codes[i];keys[i].ki.dwExtraInfo=0x56454c4f52533132ULL;if(i>=2)keys[i].ki.dwFlags=KEYEVENTF_KEYUP;}
    if(SendInput(4,keys,sizeof(INPUT))!=4)throw std::runtime_error("send_hotkey_failed_no_retry");
}
static void clearInput(){
    if(!editableHost()||!modifiersClear())throw std::runtime_error("clear_requires_exact_focused_test_window");
    INPUT keys[6]{};WORD codes[6]={VK_CONTROL,'A','A',VK_CONTROL,VK_BACK,VK_BACK};
    for(int i=0;i<6;i++){keys[i].type=INPUT_KEYBOARD;keys[i].ki.wVk=codes[i];keys[i].ki.dwExtraInfo=0x56454c4f52533132ULL;if(i==2||i==3||i==5)keys[i].ki.dwFlags=KEYEVENTF_KEYUP;}
    if(SendInput(6,keys,sizeof(INPUT))!=6)throw std::runtime_error("send_clear_failed_no_retry");
}
static void ensureDesktop(){
    HDESK desktop=OpenInputDesktop(0,FALSE,DESKTOP_READOBJECTS);if(!desktop)throw std::runtime_error("interactive_desktop_unavailable");
    wchar_t name[64]{};DWORD n=0;bool ok=GetUserObjectInformationW(desktop,UOI_NAME,name,sizeof(name),&n)!=FALSE;CloseDesktop(desktop);
    if(!ok||_wcsicmp(name,L"Default")!=0)throw std::runtime_error("interactive_desktop_not_default");
}
static int selfTest(const fs::path& trace){
    output=CreateFileW(trace.c_str(),GENERIC_WRITE,FILE_SHARE_READ,nullptr,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,nullptr);
    if(output==INVALID_HANDLE_VALUE)throw std::runtime_error("preserve_self_test_trace");
    pid=GetCurrentProcessId();process=OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION|SYNCHRONIZE,FALSE,pid);
    WNDCLASSW cls{};cls.lpfnWndProc=DefWindowProcW;cls.hInstance=GetModuleHandleW(nullptr);cls.lpszClassName=L"VeloRS12InputGuardFixture";
    if(!RegisterClassW(&cls))throw std::runtime_error("register_hidden_guard_fixture");
    target=CreateWindowExW(0,cls.lpszClassName,L"",WS_OVERLAPPEDWINDOW,0,0,64,64,nullptr,nullptr,cls.hInstance,nullptr);
    if(!target)throw std::runtime_error("create_hidden_guard_fixture");
    bool hiddenRejected=false,foreignRejected=false,clearRejected=false;
    try{sendUnicode();}catch(const std::exception&){hiddenRejected=true;}
    try{clearInput();}catch(const std::exception&){clearRejected=true;}
    DestroyWindow(target);target=GetDesktopWindow();
    try{sendUnicode();}catch(const std::exception&){foreignRejected=true;}
    if(!hiddenRejected||!foreignRejected||!clearRejected)throw std::runtime_error("input_guard_self_test_failed");
    row("{\"event\":\"guard_self_test\",\"passed\":true,\"hidden_rejected\":true,\"foreign_rejected\":true,\"clear_rejected\":true,\"input_sent\":false}");
    CloseHandle(process);process=nullptr;CloseHandle(output);output=INVALID_HANDLE_VALUE;return 0;
}
int wmain(int argc,wchar_t** argv){
    try{
        if(argc==3&&wcscmp(argv[1],L"self-test")==0)return selfTest(argv[2]);
        if(argc!=8)throw std::runtime_error("usage_mode_edition_pidfile_log_expected_executable_newtrace_readyfile");
        std::wstring mode=argv[1];edition=argv[2];if((mode!=L"startup"&&mode!=L"hotkey"&&mode!=L"query"&&mode!=L"clear")||(edition!=L"wails"&&edition!=L"slint"))throw std::runtime_error("unsupported_mode");
        fs::path pidfile=argv[3],log=argv[4],expectedExe=fs::absolute(argv[5]),trace=argv[6],ready=argv[7];
        if(expectedExe.wstring().find(L"\\.reasonix\\")==std::wstring::npos||log.wstring().find(L"\\.reasonix\\")==std::wstring::npos)throw std::runtime_error("isolated_fixture_paths_required");
        ensureDesktop();
        output=CreateFileW(trace.c_str(),GENERIC_WRITE,FILE_SHARE_READ,nullptr,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,nullptr);if(output==INVALID_HANDLE_VALUE)throw std::runtime_error("preserve_trace");
        clockPair("start");
        foregroundHook=SetWinEventHook(EVENT_SYSTEM_FOREGROUND,EVENT_SYSTEM_FOREGROUND,nullptr,onForeground,0,0,WINEVENT_OUTOFCONTEXT);
        if(!foregroundHook)throw std::runtime_error("install_foreground_observer");
        preciseTimer=CreateWaitableTimerExW(nullptr,nullptr,0x00000002,TIMER_ALL_ACCESS);
        row(std::string("{\"event\":\"probe_timer\",\"high_resolution\":")+(preciseTimer?"true":"false")+",\"requested_interval_ms\":2,\"global_timer_period_changed\":false}");
        HANDLE signal=CreateFileW(ready.c_str(),GENERIC_WRITE,FILE_SHARE_READ,nullptr,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,nullptr);if(signal==INVALID_HANDLE_VALUE)throw std::runtime_error("preserve_ready_marker");CloseHandle(signal);
        auto waitStart=qpc();
        while(!fs::exists(pidfile)){pump();if(qpc()-waitStart>frequency()*20)throw std::runtime_error("target_launch_timeout");Sleep(1);}
        {std::ifstream info(pidfile);info>>pid>>expectedCreation;if(!info||!pid||!expectedCreation)throw std::runtime_error("invalid_process_identity_file");}
        process=OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION|SYNCHRONIZE,FALSE,pid);if(!process)throw std::runtime_error("open_target_process");
        FILETIME created{},exited{},kernel{},user{};if(!GetProcessTimes(process,&created,&exited,&kernel,&user)||filetime(created)!=expectedCreation)throw std::runtime_error("process_creation_changed");
        wchar_t executable[32768]{};DWORD capacity=32768;if(!QueryFullProcessImageNameW(process,0,executable,&capacity)||_wcsicmp(executable,expectedExe.c_str())!=0)throw std::runtime_error("target_executable_changed");
        auto locate=qpc();while(!(target=findTarget())){pump();if(!alive()||qpc()-locate>frequency()*15)throw std::runtime_error("target_window_timeout");Sleep(1);}
        nativeFocus("target_attached");
        if(mode==L"clear"){clearInput();row("{\"event\":\"clear_sent\",\"qpc\":"+std::to_string(qpc())+",\"machine_input\":true}");}
        else{
            FILETIME wall{};auto before=qpc();GetSystemTimePreciseAsFileTime(&wall);auto after=qpc();
            const uint64_t epoch=116444736000000000ULL;
            uint64_t startFiletime=mode==L"startup"?expectedCreation:filetime(wall);
            auto startLower=mode==L"startup"?before-static_cast<long long>((filetime(wall)-expectedCreation)*frequency()/10000000ULL):before;
            auto startUpper=mode==L"startup"?after-static_cast<long long>((filetime(wall)-expectedCreation)*frequency()/10000000ULL):after;
            auto beforeLog=readShared(log);size_t skipLines=mode==L"startup"?0:std::count(beforeLog.begin(),beforeLog.end(),'\n');
            if(mode==L"hotkey")hotkey();
            row("{\"event\":\"phase_start\",\"pid\":"+std::to_string(pid)+",\"start_lower_qpc\":"+std::to_string(startLower)+",\"start_upper_qpc\":"+std::to_string(startUpper)+",\"qpc_frequency\":"+std::to_string(frequency())+",\"start_filetime_100ns\":"+std::to_string(startFiletime)+",\"machine_input\":true}");
            uint64_t end=0;unsigned probes=0,preSendFocusDeferrals=0;long long firstProbe=0,previousProbe=0,maxGap=0;bool focusTransitionLogged=false;
            auto deadline=qpc()+frequency()*15;
            while(qpc()<deadline&&alive()){
                pump();
                end=accepted(log,(startFiletime-epoch)/10,skipLines);if(end)break;
                if(editableHost()&&modifiersClear()&&(mode!=L"query"||probes==0)){
                    auto at=qpc();
                    if(probes>=256)throw std::runtime_error("probe_budget_exhausted");
                    // Focus can change between the polling predicate and the
                    // immediate send guard. If nothing was sent, defer safely.
                    if(!sendUnicode(false)){preSendFocusDeferrals++;nativeFocus("pre_send_guard_deferred");probePause();continue;}
                    if(!firstProbe)firstProbe=at;if(previousProbe)maxGap=std::max(maxGap,at-previousProbe);previousProbe=at;probes++;
                    row("{\"event\":\"unicode_probe\",\"qpc\":"+std::to_string(at)+",\"sequence\":"+std::to_string(probes)+",\"machine_input\":true}");
                }else if(probes>0&&!editableHost()){
                    if(!owner(GetForegroundWindow())){nativeFocus("guard_abort");throw std::runtime_error("foreground_left_target_process_during_probe");}
                    // A WebView focus handoff can briefly have no focused child.
                    // Keep waiting without sending input; do not mislabel it as
                    // another process stealing foreground or dispatch blindly.
                    if(!focusTransitionLogged){nativeFocus("target_focus_handoff_wait");focusTransitionLogged=true;}
                }
                probePause();
            }
            clockPair("accepted_or_timeout");
            if(!end)throw std::runtime_error("no_focused_noncomposing_accepted_character");
            if(!probes)throw std::runtime_error("unattributed_input_before_machine_probe");
            row("{\"event\":\"accepted\",\"unix_us\":"+std::to_string(end)+",\"probes\":"+std::to_string(probes)+",\"pre_send_focus_deferrals\":"+std::to_string(preSendFocusDeferrals)+",\"first_probe_qpc\":"+std::to_string(firstProbe)+",\"max_probe_gap_qpc\":"+std::to_string(maxGap)+",\"machine_input\":true}");
        }
        clockPair("end");if(foregroundHook){UnhookWinEvent(foregroundHook);foregroundHook=nullptr;}if(preciseTimer){CloseHandle(preciseTimer);preciseTimer=nullptr;}CloseHandle(process);process=nullptr;CloseHandle(output);output=INVALID_HANDLE_VALUE;return 0;
    }catch(const std::exception& error){
        try{if(output!=INVALID_HANDLE_VALUE)row("{\"event\":\"failure\",\"reason\":\""+std::string(error.what())+"\",\"machine_input\":true}");}catch(...){}
        if(foregroundHook)UnhookWinEvent(foregroundHook);if(preciseTimer)CloseHandle(preciseTimer);if(process)CloseHandle(process);if(output!=INVALID_HANDLE_VALUE)CloseHandle(output);
        std::cerr<<"RS12_INPUT_CALIBRATION_ERROR "<<error.what()<<std::endl;return 1;
    }
}
