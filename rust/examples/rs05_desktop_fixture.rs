//! Disposable Win32 foreground/owned/fullscreen/conflict fixture for RS-05.
//! No product data, shell actions, or injected keyboard input. Escape exits.

use std::ptr::{null, null_mut};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    COLOR_WINDOW, GetMonitorInfoW, MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MonitorFromWindow,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, RegisterHotKey, UnregisterHotKey,
};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_KEYDOWN if wparam == 0x1b => {
            unsafe { DestroyWindow(hwnd) };
            0
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            0
        }
        WM_HOTKEY => {
            println!("event=fixture_reserved_hotkey");
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let fullscreen = args.iter().any(|arg| arg == "--fullscreen");
    let owner = args
        .iter()
        .find_map(|arg| arg.strip_prefix("--owner="))
        .map(|text| isize::from_str_radix(text, 16))
        .transpose()?
        .unwrap_or(0) as HWND;
    let reserve_alt_space = args.iter().any(|arg| arg == "--reserve-alt-space");
    let reserve_f23 = args.iter().any(|arg| arg == "--reserve-f23");
    let class = wide("VeloRs05DisposableFixture");
    let title = wide(if fullscreen {
        "RS-05 fixture: fullscreen"
    } else if !owner.is_null() {
        "RS-05 fixture: owned popup"
    } else {
        "RS-05 fixture: foreground"
    });
    unsafe {
        let instance = GetModuleHandleW(null());
        let window_class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: class.as_ptr(),
            hbrBackground: (COLOR_WINDOW + 1) as _,
            ..Default::default()
        };
        if RegisterClassW(&window_class) == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        GetMonitorInfoW(
            MonitorFromWindow(null_mut(), MONITOR_DEFAULTTOPRIMARY),
            &mut info,
        );
        let (x, y, width, height) = if fullscreen {
            (
                info.rcMonitor.left,
                info.rcMonitor.top,
                info.rcMonitor.right - info.rcMonitor.left,
                info.rcMonitor.bottom - info.rcMonitor.top,
            )
        } else {
            (80, 80, 620, 260)
        };
        let style = if fullscreen || !owner.is_null() {
            WS_POPUP | WS_BORDER
        } else {
            WS_OVERLAPPEDWINDOW
        };
        let hwnd = CreateWindowExW(
            0,
            class.as_ptr(),
            title.as_ptr(),
            style,
            x,
            y,
            width,
            height,
            owner,
            null_mut(),
            instance,
            null(),
        );
        if hwnd.is_null() {
            return Err(std::io::Error::last_os_error().into());
        }
        let reserved = if reserve_alt_space {
            Some((MOD_ALT, 0x20))
        } else if reserve_f23 {
            Some((MOD_CONTROL | MOD_ALT | MOD_SHIFT, 0x86))
        } else {
            None
        };
        if let Some((modifiers, key)) = reserved
            && RegisterHotKey(hwnd, 1, modifiers | MOD_NOREPEAT, key) == 0
        {
            DestroyWindow(hwnd);
            return Err(std::io::Error::last_os_error().into());
        }
        let static_class = wide("STATIC");
        let text = wide(
            "RS-05 disposable desktop fixture\n\nUse Alt+Space to call Velo. Press Escape to exit this fixture.\nNo product files or settings are accessed.",
        );
        CreateWindowExW(
            0,
            static_class.as_ptr(),
            text.as_ptr(),
            WS_CHILD | WS_VISIBLE,
            20,
            24,
            width - 50,
            120,
            hwnd,
            null_mut(),
            instance,
            null(),
        );
        ShowWindow(hwnd, SW_SHOW);
        // The test runner starts helpers with STARTF_USESHOWWINDOW/SW_HIDE.
        // The second call applies the fixture's explicit interactive visibility.
        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);
        println!(
            "event=fixture_ready hwnd={:X} fullscreen={fullscreen} owned={} reserved={}",
            hwnd as isize,
            !owner.is_null(),
            reserved.is_some()
        );
        let mut message = MSG::default();
        while GetMessageW(&mut message, null_mut(), 0, 0) > 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        if reserved.is_some() {
            UnregisterHotKey(hwnd, 1);
        }
        UnregisterClassW(class.as_ptr(), instance);
    }
    Ok(())
}
