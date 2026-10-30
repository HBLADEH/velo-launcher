//! Explicit native integration probe for RS-06. Message injection is labelled
//! and never presented as real pointer input, display removal, or resume.
//! No keyboard injection, system settings changes, Shell actions or data access.

use std::ptr::{null, null_mut};
use windows_sys::Win32::Foundation::{HWND, LPARAM, POINT, RECT};
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::HiDpi::*;
use windows_sys::Win32::UI::Shell::{NIN_SELECT, NOTIFYICONIDENTIFIER, Shell_NotifyIconGetRect};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}

unsafe extern "system" fn monitor_callback(
    monitor: HMONITOR,
    _: HDC,
    _: *mut RECT,
    _: LPARAM,
) -> i32 {
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    let (mut x, mut y) = (96, 96);
    unsafe {
        GetMonitorInfoW(monitor, &mut info);
        GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut x, &mut y);
    }
    println!(
        "event=probe_monitor x={} y={} width={} height={} work_x={} work_y={} work_width={} work_height={} dpi={x} primary={}",
        info.rcMonitor.left,
        info.rcMonitor.top,
        info.rcMonitor.right - info.rcMonitor.left,
        info.rcMonitor.bottom - info.rcMonitor.top,
        info.rcWork.left,
        info.rcWork.top,
        info.rcWork.right - info.rcWork.left,
        info.rcWork.bottom - info.rcWork.top,
        info.dwFlags & 1 != 0
    );
    1
}

unsafe extern "system" fn window_callback(hwnd: HWND, _: LPARAM) -> i32 {
    let mut class = [0u16; 256];
    let length = unsafe { GetClassNameW(hwnd, class.as_mut_ptr(), 256) };
    let class = String::from_utf16_lossy(&class[..length.max(0) as usize]);
    if !matches!(
        class.as_str(),
        "VeloSlintLauncherWindow" | "VeloSlintDesktopService"
    ) {
        return 1;
    }
    let mut rect = RECT::default();
    let mut pid = 0;
    unsafe {
        GetWindowRect(hwnd, &mut rect);
        GetWindowThreadProcessId(hwnd, &mut pid);
    }
    println!(
        "event=probe_window class={class} hwnd={:X} pid={pid} visible={} dpi={} x={} y={} width={} height={}",
        hwnd as isize,
        unsafe { IsWindowVisible(hwnd) } != 0,
        unsafe { GetDpiForWindow(hwnd) },
        rect.left,
        rect.top,
        rect.right - rect.left,
        rect.bottom - rect.top
    );
    if class == "VeloSlintDesktopService" {
        let id = NOTIFYICONIDENTIFIER {
            cbSize: std::mem::size_of::<NOTIFYICONIDENTIFIER>() as u32,
            hWnd: hwnd,
            uID: 1,
            ..Default::default()
        };
        let mut icon = RECT::default();
        let found = unsafe { Shell_NotifyIconGetRect(&id, &mut icon) } >= 0;
        println!(
            "event=probe_tray hwnd={:X} registered={found} x={} y={} width={} height={}",
            hwnd as isize,
            icon.left,
            icon.top,
            icon.right - icon.left,
            icon.bottom - icon.top
        );
    }
    1
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.is_empty() {
        unsafe {
            EnumDisplayMonitors(null_mut(), null(), Some(monitor_callback), 0);
            EnumWindows(Some(window_callback), 0);
        }
        return Ok(());
    }
    let id = args.get(1).ok_or(
        "Expected: --tray-left|--tray-menu|--taskbar-created|--environment ID, or --move HWND X Y",
    )?;
    if args[0] == "--move" {
        let hwnd = isize::from_str_radix(id.trim_start_matches("0x"), 16)? as HWND;
        let x: i32 = args.get(2).ok_or("Missing X")?.parse()?;
        let y: i32 = args.get(3).ok_or("Missing Y")?.parse()?;
        unsafe {
            SetWindowPos(
                hwnd,
                null_mut(),
                x,
                y,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOSIZE,
            );
        }
    } else {
        let hwnd = unsafe {
            FindWindowW(
                wide("VeloSlintDesktopService").as_ptr(),
                wide(&format!("Velo Slint instance {id}")).as_ptr(),
            )
        };
        if hwnd.is_null() {
            return Err("Instance endpoint not found".into());
        }
        let (message, lparam) = match args[0].as_str() {
            "--tray-left" => (WM_APP + 20, (1 << 16) | NIN_SELECT as isize),
            "--tray-menu" => (WM_APP + 20, (1 << 16) | WM_CONTEXTMENU as isize),
            "--taskbar-created" => (
                unsafe { RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()) },
                0,
            ),
            "--environment" => (WM_DISPLAYCHANGE, 0),
            _ => return Err("Unknown probe action".into()),
        };
        unsafe {
            PostMessageW(hwnd, message, 0, lparam);
        }
        let mut point = POINT::default();
        unsafe {
            GetCursorPos(&mut point);
        }
        println!(
            "event=probe_message_injected action={} hwnd={:X} cursor_x={} cursor_y={}",
            args[0], hwnd as isize, point.x, point.y
        );
    }
    Ok(())
}
