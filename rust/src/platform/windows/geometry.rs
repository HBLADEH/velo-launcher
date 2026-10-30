//! Physical work-area placement, separate from Slint's logical content size.
//! Negative monitor coordinates stay signed throughout the conversion.

use super::window::NativeWindow;
use windows_sys::Win32::Foundation::{POINT, RECT};
use windows_sys::Win32::Graphics::Gdi::{
    GetMonitorInfoW, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint,
    MonitorFromWindow,
};
use windows_sys::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetWindowRect, SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Bounds {
    fn from_rect(rect: RECT) -> Self {
        Self {
            x: rect.left,
            y: rect.top,
            width: rect.right - rect.left,
            height: rect.bottom - rect.top,
        }
    }
}

pub fn content_height(home: bool, settings: bool, importing: bool, results: usize) -> i32 {
    if home || settings || importing {
        660
    } else {
        156usize
            .saturating_add(results.max(1).saturating_mul(54))
            .clamp(160, 720) as i32
    }
}

pub fn placement(work: Bounds, size: (i32, i32), previous: Option<Bounds>) -> Bounds {
    let width = size.0.clamp(1, work.width.max(1));
    let height = size.1.clamp(1, work.height.max(1));
    let (x, y) = previous.map_or(
        (work.x + (work.width - width) / 2, work.y + work.height / 5),
        |old| (old.x, old.y),
    );
    Bounds {
        x: x.clamp(work.x, work.x + work.width.max(1) - width),
        y: y.clamp(work.y, work.y + work.height.max(1) - height),
        width,
        height,
    }
}

pub fn monitor_dpi(monitor: HMONITOR) -> u32 {
    let (mut x, mut y) = (96, 96);
    if unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut x, &mut y) } < 0 {
        96
    } else {
        x.max(96)
    }
}

pub struct Layout {
    pub bounds: Bounds,
    pub work: Bounds,
    pub dpi: u32,
}

pub fn layout(native: NativeWindow, logical_height: i32, positioned: bool) -> Option<Layout> {
    let mut old = RECT::default();
    if unsafe { GetWindowRect(native.hwnd(), &mut old) } == 0 {
        return None;
    }
    let monitor = if positioned {
        unsafe { MonitorFromWindow(native.hwnd(), MONITOR_DEFAULTTONEAREST) }
    } else {
        let mut point = POINT::default();
        unsafe {
            GetCursorPos(&mut point);
            MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST)
        }
    };
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if unsafe { GetMonitorInfoW(monitor, &mut info) } == 0 {
        return None;
    }
    let dpi = monitor_dpi(monitor);
    let physical = |logical: i32| ((i64::from(logical) * i64::from(dpi) + 48) / 96) as i32;
    let work = Bounds::from_rect(info.rcWork);
    let bounds = placement(
        work,
        (physical(640), physical(logical_height.clamp(160, 720))),
        positioned.then(|| Bounds::from_rect(old)),
    );
    Some(Layout { bounds, work, dpi })
}

pub fn apply(native: NativeWindow, bounds: Bounds) -> bool {
    let mut old = RECT::default();
    if unsafe { GetWindowRect(native.hwnd(), &mut old) } != 0 && Bounds::from_rect(old) == bounds {
        return true;
    }
    unsafe {
        SetWindowPos(
            native.hwnd(),
            std::ptr::null_mut(),
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
            SWP_NOACTIVATE | SWP_NOZORDER,
        ) != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_sizes_match_the_vue_list_contract_and_restore_home_from_search() {
        assert_eq!(content_height(true, false, false, 0), 660);
        assert_eq!(content_height(false, true, false, 1), 660);
        assert_eq!(content_height(false, false, true, 0), 660);
        assert_eq!(content_height(false, false, false, 0), 210);
        assert_eq!(content_height(false, false, false, 1), 210);
        assert_eq!(content_height(false, false, false, 8), 588);
        assert_eq!(content_height(false, false, false, 28), 720);
    }

    #[test]
    fn placement_survives_negative_origins_taskbars_small_screens_and_removal() {
        let work = Bounds {
            x: -1920,
            y: -200,
            width: 1920,
            height: 1040,
        };
        let old = Bounds {
            x: -1700,
            y: -100,
            width: 640,
            height: 660,
        };
        assert_eq!(placement(work, (640, 660), Some(old)), old);
        let primary = Bounds {
            x: 0,
            y: 0,
            width: 1920,
            height: 1040,
        };
        assert_eq!(
            placement(primary, (640, 660), Some(old)),
            Bounds { x: 0, y: 0, ..old }
        );
        let tiny = Bounds {
            x: -600,
            y: -480,
            width: 600,
            height: 440,
        };
        assert_eq!(placement(tiny, (1280, 1320), Some(old)), tiny);
        for dpi in [96, 120, 144, 192] {
            let bounds = placement(work, (640 * dpi / 96, 660 * dpi / 96), None);
            assert!(bounds.x >= work.x && bounds.y >= work.y);
            assert!(bounds.x + bounds.width <= work.x + work.width);
            assert!(bounds.y + bounds.height <= work.y + work.height);
        }
    }
}
