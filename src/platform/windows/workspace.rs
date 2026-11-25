//! Native snapshot, placement and hit region for Workspace.

use std::{io, time::Instant};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Dwm::{DWMWA_TRANSITIONS_FORCEDISABLED, DwmSetWindowAttribute};
use windows_sys::Win32::Graphics::Gdi::{
    ClientToScreen, CreateRoundRectRgn, DeleteObject, HRGN, MONITOR_DEFAULTTONEAREST,
    MonitorFromPoint, SetWindowRgn,
};
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GA_ROOT, GUITHREADINFO, GWL_EXSTYLE, GWL_STYLE, GetAncestor, GetClassNameW, GetClientRect,
    GetCursorPos, GetDesktopWindow, GetForegroundWindow, GetGUIThreadInfo, GetShellWindow,
    GetWindowLongPtrW, GetWindowRect, GetWindowThreadProcessId, HTTRANSPARENT, IsWindowVisible,
    MA_NOACTIVATE, STYLESTRUCT, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_NOZORDER, SetForegroundWindow, SetWindowLongPtrW, SetWindowPos, WINDOWPOS,
    WM_MOUSEACTIVATE, WM_NCACTIVATE, WM_NCDESTROY, WM_NCHITTEST, WM_STYLECHANGING,
    WM_WINDOWPOSCHANGING, WS_CAPTION, WS_EX_NOACTIVATE, WindowFromPoint,
};

use super::geometry::{self, Bounds, Layout};
use super::window::NativeWindow;
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Foreground {
    hwnd: isize,
    process: u32,
    thread: u32,
}

impl Foreground {
    fn identify(hwnd: HWND) -> Option<Self> {
        if hwnd.is_null() {
            return None;
        }
        let mut process = 0;
        // SAFETY: process is writable. An invalid or destroyed HWND returns zero.
        let thread = unsafe { GetWindowThreadProcessId(hwnd, &mut process) };
        (thread != 0).then_some(Self {
            hwnd: hwnd as isize,
            process,
            thread,
        })
    }

    pub fn current() -> Option<Self> {
        // SAFETY: querying the foreground yields a borrowed HWND.
        Self::identify(unsafe { GetForegroundWindow() })
    }

    fn under(point: (i32, i32)) -> Option<Self> {
        // SAFETY: all HWNDs returned here are borrowed. GetAncestor accepts a window
        // hit by WindowFromPoint, and the snapshot verifies PID and thread below.
        unsafe {
            let hit = WindowFromPoint(POINT {
                x: point.0,
                y: point.1,
            });
            let root = GetAncestor(hit, GA_ROOT);
            Self::identify(root).filter(|window| window.external_application())
        }
    }

    pub fn external_application(self) -> bool {
        if self.process == std::process::id() {
            return false;
        }
        let hwnd = self.hwnd as HWND;
        let mut class = [0u16; 64];
        // SAFETY: class is writable and no borrowed HWND is retained here.
        unsafe {
            if hwnd == GetDesktopWindow() || hwnd == GetShellWindow() {
                return false;
            }
            let len = GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32);
            len > 0
                && ![
                    "WorkerW",
                    "Progman",
                    "Shell_TrayWnd",
                    "Shell_SecondaryTrayWnd",
                ]
                .iter()
                .any(|name| {
                    name.encode_utf16()
                        .eq(class[..len as usize].iter().copied())
                })
        }
    }

    pub fn is_windowed(self) -> bool {
        super::window::fullscreen_state(self.hwnd as HWND) == Some(false)
    }

    pub fn process(self) -> u32 {
        self.process
    }

    pub fn thread(self) -> u32 {
        self.thread
    }

    pub fn focus(self) -> Option<Self> {
        let mut info = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        // SAFETY: the info buffer is writable and the focus handle is borrowed.
        unsafe {
            (GetGUIThreadInfo(self.thread, &mut info) != 0
                && GetAncestor(info.hwndFocus, GA_ROOT) == self.hwnd as HWND)
                .then(|| Self::identify(info.hwndFocus))
                .flatten()
        }
    }

    pub fn exists(self) -> bool {
        Self::identify(self.hwnd as HWND) == Some(self)
    }

    pub fn restore(self, workspace: NativeWindow) -> bool {
        let hwnd = self.hwnd as HWND;
        if hwnd == workspace.hwnd() || Self::identify(hwnd) != Some(self) {
            return false;
        }
        // SAFETY: the target is a still-visible window with the captured identity.
        // The caller currently owns the foreground and never forces activation.
        unsafe {
            IsWindowVisible(hwnd) != 0
                && SetForegroundWindow(hwnd) != 0
                && GetForegroundWindow() == hwnd
        }
    }
}

pub fn current_input_desktop() -> bool {
    use windows_sys::Win32::System::StationsAndDesktops::{
        CloseDesktop, DESKTOP_READOBJECTS, GetThreadDesktop, GetUserObjectInformationW, HDESK,
        OpenInputDesktop, UOI_NAME,
    };
    use windows_sys::Win32::System::Threading::GetCurrentThreadId;

    fn name(desktop: HDESK) -> Option<[u16; 128]> {
        let mut name = [0u16; 128];
        let mut length = 0;
        // SAFETY: name is a writable UTF-16 buffer and desktop stays open.
        (!desktop.is_null()
            && unsafe {
                GetUserObjectInformationW(
                    desktop,
                    UOI_NAME,
                    name.as_mut_ptr().cast(),
                    std::mem::size_of_val(&name) as u32,
                    &mut length,
                ) != 0
            })
        .then_some(name)
    }
    // SAFETY: the thread desktop is borrowed. Only the opened input desktop is closed.
    unsafe {
        let input = OpenInputDesktop(0, 0, DESKTOP_READOBJECTS);
        if input.is_null() {
            return false;
        }
        let matching = name(input)
            .is_some_and(|expected| name(GetThreadDesktop(GetCurrentThreadId())) == Some(expected));
        CloseDesktop(input);
        matching
    }
}

#[derive(Clone, Copy)]
pub struct Invocation {
    pub at: Instant,
    pub foreground: Option<Foreground>,
    pub anchor: Option<Anchor>,
    pub hovered: Option<Foreground>,
    pub focus: Option<Foreground>,
}

impl Invocation {
    pub fn capture() -> Self {
        let at = Instant::now();
        Self::new(
            at,
            Foreground::current(),
            cursor().and_then(Anchor::capture),
        )
    }

    pub(super) fn new(at: Instant, foreground: Option<Foreground>, anchor: Option<Anchor>) -> Self {
        Self {
            at,
            foreground,
            anchor,
            hovered: anchor.and_then(|anchor| Foreground::under(anchor.point)),
            focus: foreground.and_then(Foreground::focus),
        }
    }

    pub fn after_panel_hides(self) -> Self {
        Self::new(self.at, Foreground::current(), self.anchor)
    }

    pub fn display_unchanged(self) -> bool {
        self.anchor
            .is_some_and(|anchor| Anchor::capture(anchor.point) == Some(anchor))
    }
}

pub fn cursor() -> Option<(i32, i32)> {
    let mut cursor = POINT::default();
    // SAFETY: cursor is a writable screen coordinate in physical pixels.
    (unsafe { GetCursorPos(&mut cursor) } != 0).then_some((cursor.x, cursor.y))
}

pub fn prepare_frame(native: NativeWindow) -> io::Result<()> {
    let hwnd = native.hwnd();
    let disabled = 1i32;
    // Winit retains WS_CAPTION to support native window behavior. The
    // popup gives that space to its contents, including on its first mixed-DPI frame.
    // SAFETY: the UI thread owns the HWND. DWM copies the BOOL synchronously.
    unsafe {
        if SetWindowSubclass(hwnd, Some(workspace_frame), 0, 0) == 0 {
            return Err(io::Error::other("Workspace frame subclass unavailable"));
        }
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
        if style & WS_CAPTION as isize != 0
            && (SetWindowLongPtrW(hwnd, GWL_STYLE, style & !(WS_CAPTION as isize)) == 0
                || SetWindowPos(
                    hwnd,
                    std::ptr::null_mut(),
                    0,
                    0,
                    0,
                    0,
                    SWP_FRAMECHANGED | SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
                ) == 0)
        {
            return Err(io::Error::last_os_error());
        }
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_TRANSITIONS_FORCEDISABLED as u32,
            (&raw const disabled).cast(),
            std::mem::size_of_val(&disabled) as u32,
        )
    };
    Ok(())
}

pub fn set_passive(native: NativeWindow, passive: bool) -> io::Result<()> {
    let hwnd = native.hwnd();
    // SAFETY: updating subclass data and extended styles happens on the owning thread.
    unsafe {
        if SetWindowSubclass(hwnd, Some(workspace_frame), 0, usize::from(passive)) == 0 {
            return Err(io::Error::other("Workspace activation policy unavailable"));
        }
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let no_activate = WS_EX_NOACTIVATE as isize;
        let desired = if passive {
            style | no_activate
        } else {
            style & !no_activate
        };
        if desired != style {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, desired);
            if GetWindowLongPtrW(hwnd, GWL_EXSTYLE) & no_activate != desired & no_activate {
                return Err(io::Error::last_os_error());
            }
        }
    }
    Ok(())
}

unsafe extern "system" fn workspace_frame(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    data: usize,
) -> LRESULT {
    if message == WM_STYLECHANGING && wparam as i32 == GWL_STYLE && lparam != 0 {
        // Winit reapplies its style on show and hide. Preserve other style bits.
        // SAFETY: WM_STYLECHANGING carries a writable STYLESTRUCT for this call.
        unsafe { (*(lparam as *mut STYLESTRUCT)).styleNew &= !WS_CAPTION };
    }
    if data != 0 {
        match message {
            WM_STYLECHANGING if wparam as i32 == GWL_EXSTYLE && lparam != 0 => {
                // SAFETY: the style change belongs to this HWND until the callback returns.
                unsafe { (*(lparam as *mut STYLESTRUCT)).styleNew |= WS_EX_NOACTIVATE };
            }
            WM_WINDOWPOSCHANGING if lparam != 0 => {
                // Winit shows its window through Win32. Prevent activation even
                // when it reapplies its own visibility flags on a reused HWND.
                // SAFETY: WINDOWPOS is writable for WM_WINDOWPOSCHANGING.
                unsafe { (*(lparam as *mut WINDOWPOS)).flags |= SWP_NOACTIVATE };
            }
            WM_NCHITTEST => return HTTRANSPARENT as LRESULT,
            WM_MOUSEACTIVATE => return MA_NOACTIVATE as LRESULT,
            _ => {}
        }
    }
    if message == WM_NCDESTROY {
        // SAFETY: the subclass is installed on this window and thread until destruction.
        unsafe { RemoveWindowSubclass(hwnd, Some(workspace_frame), id) };
    }
    let lparam = if message == WM_NCACTIVATE {
        // Winit still tracks activation. A value of -1 tells DefWindowProc to
        // skip the classic caption paint, which can bleed into the clipped tiles.
        -1
    } else {
        lparam
    };
    // SAFETY: DefSubclassProc forwards the current message on this thread.
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Anchor {
    pub point: (i32, i32),
    monitor: isize,
    work: Bounds,
    dpi: u32,
}

impl Anchor {
    pub(super) fn capture(point: (i32, i32)) -> Option<Self> {
        let position = POINT {
            x: point.0,
            y: point.1,
        };
        // SAFETY: MonitorFromPoint returns a borrowed monitor for these screen coordinates.
        let monitor = unsafe { MonitorFromPoint(position, MONITOR_DEFAULTTONEAREST) };
        let (work, dpi) = geometry::monitor_work_area(monitor)?;
        Some(Self {
            point,
            monitor: monitor as isize,
            work,
            dpi,
        })
    }

    pub fn layout(self, width: i32, height: i32, center_y: i32) -> Option<Layout> {
        let physical = |dip| ((i64::from(dip) * i64::from(self.dpi) + 48) / 96) as i32;
        let (width, height) = (physical(width), physical(height));
        if width > self.work.width || height > self.work.height {
            return None;
        }
        let centered = Bounds {
            x: self.point.0 - width / 2,
            y: self.point.1 - physical(center_y),
            width,
            height,
        };
        Some(Layout {
            work: self.work,
            dpi: self.dpi,
            bounds: geometry::placement(self.work, (width, height), Some(centered)),
        })
    }

    pub fn dpi(self) -> u32 {
        self.dpi
    }
}

pub fn apply_layout(native: NativeWindow, layout: Layout) -> io::Result<()> {
    // SAFETY: the HWND is owned by the UI thread during placement.
    let previous_dpi = unsafe { GetDpiForWindow(native.hwnd()) };
    if !geometry::apply(native, layout.bounds) {
        return Err(io::Error::last_os_error());
    }
    // A cross-monitor move synchronously delivers WM_DPICHANGED. Winit may
    // scale the supplied physical size again, so restore it before showing.
    if previous_dpi != layout.dpi && !geometry::apply(native, layout.bounds) {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Region {
    client: Bounds,
    dpi: u32,
}

impl Region {
    pub fn read(native: NativeWindow) -> io::Result<Self> {
        let hwnd = native.hwnd();
        let mut outer = RECT::default();
        let mut client = RECT::default();
        let mut origin = POINT::default();
        // SAFETY: these synchronous queries use a UI-owned HWND and writable outputs.
        if unsafe {
            GetWindowRect(hwnd, &mut outer) == 0
                || GetClientRect(hwnd, &mut client) == 0
                || ClientToScreen(hwnd, &mut origin) == 0
        } {
            return Err(io::Error::last_os_error());
        }
        // Regions are in window coordinates. Rendering is relative to the client area.
        Ok(Self {
            client: Bounds {
                x: origin.x - outer.left,
                y: origin.y - outer.top,
                width: client.right,
                height: client.bottom,
            },
            // SAFETY: hwnd belongs to the live Workspace adapter.
            dpi: unsafe { GetDpiForWindow(hwnd) }.max(96),
        })
    }

    pub fn apply(self, native: NativeWindow, center_only: bool) -> io::Result<()> {
        let Bounds {
            mut x,
            mut y,
            mut width,
            mut height,
        } = self.client;
        let physical = |dip| ((i64::from(dip) * i64::from(self.dpi) + 48) / 96) as i32;
        if center_only {
            // Same middle cell as WorkspaceGesturePad, including its two gutters.
            let cell_width = (width - physical(32)) / 3;
            let cell_height = (height - physical(32)) / 3;
            x += (width - cell_width) / 2;
            y += (height - cell_height) / 2;
            width = cell_width;
            height = cell_height;
        }
        let diameter = physical(24);
        // SAFETY: GDI owns no borrowed storage. OwnedRegion transfers the result to HWND.
        let region = unsafe { CreateRoundRectRgn(x, y, x + width, y + height, diameter, diameter) };
        OwnedRegion::new(region)?.attach(native)
    }
}

struct OwnedRegion(HRGN);

impl OwnedRegion {
    fn new(region: HRGN) -> io::Result<Self> {
        if region.is_null() {
            Err(io::Error::last_os_error())
        } else {
            Ok(Self(region))
        }
    }

    fn attach(self, native: NativeWindow) -> io::Result<()> {
        // SAFETY: the system takes ownership on success. Only failure needs DeleteObject.
        if unsafe { SetWindowRgn(native.hwnd(), self.0, 1) } == 0 {
            return Err(io::Error::last_os_error());
        }
        std::mem::forget(self);
        Ok(())
    }
}

impl Drop for OwnedRegion {
    fn drop(&mut self) {
        // SAFETY: the region is owned until SetWindowRgn accepts it.
        unsafe { DeleteObject(self.0) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_anchors_its_controls_and_clamps_to_a_signed_work_area() {
        let anchor = Anchor {
            point: (-760, 330),
            monitor: 0,
            work: Bounds {
                x: -1600,
                y: -120,
                width: 1600,
                height: 900,
            },
            dpi: 120,
        };
        let panel = anchor.layout(300, 348, 172).unwrap();
        assert_eq!(
            panel.bounds,
            Bounds {
                x: -947,
                y: 115,
                width: 375,
                height: 435
            }
        );

        let corner = Anchor {
            point: (-1, 779),
            ..anchor
        };
        let pad = corner.layout(278, 218, 109).unwrap();
        assert_eq!(
            pad.bounds,
            Bounds {
                x: -348,
                y: 507,
                width: 348,
                height: 273
            }
        );

        let tiny = Anchor {
            work: Bounds {
                width: 350,
                ..anchor.work
            },
            ..anchor
        };
        assert!(tiny.layout(300, 348, 172).is_none());
    }
}
