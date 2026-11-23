//! Small, UI-thread-only Win32 operations; Slint owns the HWND and its lifetime.

use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::Graphics::Dwm::{DWMWA_BORDER_COLOR, DwmSetWindowAttribute};
use windows_sys::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
};
use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetFocus, SetFocus};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GA_ROOTOWNER, GWL_STYLE, GetAncestor, GetClassNameW, GetForegroundWindow, GetWindowLongPtrW,
    GetWindowRect, GetWindowThreadProcessId, IsChild, IsWindowVisible, SW_RESTORE,
    SetForegroundWindow, ShowWindow, WS_CAPTION,
};

#[derive(Clone, Copy)]
pub struct NativeWindow(isize);

impl NativeWindow {
    pub fn new(hwnd: isize) -> Self {
        Self(hwnd)
    }

    pub fn hwnd(self) -> HWND {
        self.0 as HWND
    }

    pub fn set_border_color(self, [red, green, blue]: [u8; 3]) -> bool {
        let color = u32::from_le_bytes([red, green, blue, 0]);
        // Windows 10 does not support this attribute. The caller keeps the Slint outline then.
        unsafe {
            DwmSetWindowAttribute(
                self.hwnd(),
                DWMWA_BORDER_COLOR as u32,
                (&raw const color).cast(),
                std::mem::size_of_val(&color) as u32,
            ) >= 0
        }
    }

    pub fn active(self) -> bool {
        unsafe {
            let foreground = GetForegroundWindow();
            !foreground.is_null()
                && (foreground == self.hwnd()
                    || GetAncestor(foreground, GA_ROOTOWNER) == self.hwnd()
                    || IsChild(self.hwnd(), foreground) != 0)
        }
    }

    pub fn visible(self) -> bool {
        unsafe { IsWindowVisible(self.hwnd()) != 0 }
    }

    pub fn focused(self) -> bool {
        self.active() && unsafe { GetFocus() == self.hwnd() }
    }

    pub fn moving(self) -> bool {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GUI_INMOVESIZE, GUITHREADINFO, GetGUIThreadInfo,
        };
        let mut info = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        unsafe {
            GetGUIThreadInfo(GetCurrentThreadId(), &mut info) != 0
                && info.flags & GUI_INMOVESIZE != 0
        }
    }

    pub fn activate(self) {
        // Slint owns the host and TextInput on this UI thread. A launcher started
        // by a background parent can lack foreground permission; briefly share
        // the foreground input queue, preserving Slint's native focus state.
        unsafe {
            ShowWindow(self.hwnd(), SW_RESTORE);
            SetForegroundWindow(self.hwnd());
            if !self.active() {
                let foreground = GetForegroundWindow();
                let foreground_thread = GetWindowThreadProcessId(foreground, std::ptr::null_mut());
                let caller = GetCurrentThreadId();
                if foreground_thread != 0
                    && foreground_thread != caller
                    && AttachThreadInput(caller, foreground_thread, 1) != 0
                {
                    SetForegroundWindow(self.hwnd());
                    if self.active() {
                        SetFocus(self.hwnd());
                    }
                    AttachThreadInput(caller, foreground_thread, 0);
                }
            }
            if self.active() {
                SetFocus(self.hwnd());
            }
        }
    }

    pub fn foreground_fullscreen(self) -> bool {
        // SAFETY: GetForegroundWindow returns a borrowed handle.
        !self.active() && fullscreen_state(unsafe { GetForegroundWindow() }) == Some(true)
    }
}

pub(super) fn fullscreen_state(hwnd: HWND) -> Option<bool> {
    if hwnd.is_null() {
        return None;
    }
    let mut class = [0u16; 256];
    let mut bounds = RECT::default();
    let mut monitor = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: all output buffers live through these synchronous Win32 calls.
    unsafe {
        let length = GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32);
        if length <= 0 {
            return None;
        }
        if [
            b"Progman".as_slice(),
            b"WorkerW",
            b"Shell_TrayWnd",
            b"Shell_SecondaryTrayWnd",
        ]
        .iter()
        .any(|name| {
            name.len() == length as usize
                && name
                    .iter()
                    .zip(class)
                    .all(|(left, right)| u16::from(*left) == right)
        }) || GetWindowLongPtrW(hwnd, GWL_STYLE) as u32 & WS_CAPTION == WS_CAPTION
        {
            return Some(false);
        }
        (GetWindowRect(hwnd, &mut bounds) != 0
            && GetMonitorInfoW(
                MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST),
                &mut monitor,
            ) != 0)
            .then(|| covers_monitor(bounds, monitor.rcMonitor))
    }
}

fn covers_monitor(bounds: RECT, monitor: RECT) -> bool {
    monitor.right > monitor.left
        && monitor.bottom > monitor.top
        && bounds.left <= monitor.left
        && bounds.top <= monitor.top
        && bounds.right >= monitor.right
        && bounds.bottom >= monitor.bottom
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fullscreen_uses_the_foreground_monitors_full_bounds() {
        let monitor = RECT {
            left: -1920,
            top: -200,
            right: 0,
            bottom: 880,
        };
        for (bounds, want) in [
            (monitor, true),
            (
                RECT {
                    left: -1921,
                    top: -201,
                    right: 1,
                    bottom: 881,
                },
                true,
            ),
            (
                RECT {
                    left: -1920,
                    top: -200,
                    right: 0,
                    bottom: 840,
                },
                false,
            ),
            (
                RECT {
                    left: 0,
                    top: 0,
                    right: 1920,
                    bottom: 1080,
                },
                false,
            ),
        ] {
            assert_eq!(covers_monitor(bounds, monitor), want);
        }
        assert!(!covers_monitor(RECT::default(), RECT::default()));
    }
}
