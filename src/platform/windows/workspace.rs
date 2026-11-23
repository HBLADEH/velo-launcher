//! Native snapshot, placement and hit region for the Workspace preview.

use std::{io, time::Instant};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Dwm::{DWMWA_TRANSITIONS_FORCEDISABLED, DwmSetWindowAttribute};
use windows_sys::Win32::Graphics::Gdi::{
    ALTERNATE, ClientToScreen, CombineRgn, CreatePolygonRgn, CreateRoundRectRgn, DeleteObject,
    HRGN, MONITOR_DEFAULTTONEAREST, MonitorFromPoint, RGN_OR, SetWindowRgn,
};
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GWL_STYLE, GetClientRect, GetCursorPos, GetForegroundWindow, GetWindowLongPtrW, GetWindowRect,
    GetWindowThreadProcessId, IsWindowVisible, STYLESTRUCT, SWP_FRAMECHANGED, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetForegroundWindow, SetWindowLongPtrW, SetWindowPos,
    WM_NCACTIVATE, WM_NCDESTROY, WM_STYLECHANGING, WS_CAPTION,
};

use super::geometry::{self, Bounds, Layout};
use super::window::NativeWindow;
use crate::core::workspace::radial::{self, Outline, Point};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Shaped,
    Rounded,
    Radial,
}

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

    pub fn is_windowed(self) -> bool {
        super::window::fullscreen_state(self.hwnd as HWND) == Some(false)
    }

    pub fn process(self) -> u32 {
        self.process
    }

    pub fn thread(self) -> u32 {
        self.thread
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
}

impl Invocation {
    pub fn capture() -> Self {
        let at = Instant::now();
        let foreground = Foreground::current();
        Self {
            at,
            foreground,
            anchor: cursor().and_then(Anchor::capture),
        }
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

pub fn prepare_radial_frame(native: NativeWindow) -> io::Result<()> {
    let hwnd = native.hwnd();
    let disabled = 1i32;
    // Winit retains WS_CAPTION to support native window behavior. The radial
    // popup gives that space to its tiles, including on its first mixed-DPI frame.
    // SAFETY: the UI thread owns the HWND. DWM copies the BOOL synchronously.
    unsafe {
        if SetWindowSubclass(hwnd, Some(radial_frame), 0, 0) == 0 {
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

unsafe extern "system" fn radial_frame(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    _data: usize,
) -> LRESULT {
    if message == WM_STYLECHANGING && wparam as i32 == GWL_STYLE && lparam != 0 {
        // Winit reapplies its style on show and hide. Preserve other style bits.
        // SAFETY: WM_STYLECHANGING carries a writable STYLESTRUCT for this call.
        unsafe { (*(lparam as *mut STYLESTRUCT)).styleNew &= !WS_CAPTION };
    }
    if message == WM_NCDESTROY {
        // SAFETY: the subclass is installed on this window and thread until destruction.
        unsafe { RemoveWindowSubclass(hwnd, Some(radial_frame), id) };
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

    pub fn layout(self, shape: Shape) -> Option<Layout> {
        let Self {
            point, work, dpi, ..
        } = self;
        Some(Layout {
            work,
            dpi,
            bounds: match shape {
                Shape::Radial => radial_placement(work, dpi, point)?,
                _ => placement(work, dpi, point),
            },
        })
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

pub fn point_on_board(x: f64, y: f64, width: i32, height: i32) -> Point {
    Point {
        x: x as f32 * radial::WIDTH / width.max(1) as f32,
        y: y as f32 * radial::HEIGHT / height.max(1) as f32,
    }
}

fn placement(work: Bounds, dpi: u32, cursor: (i32, i32)) -> Bounds {
    let physical = |dip| ((i64::from(dip) * i64::from(dpi) + 48) / 96) as i32;
    let width = physical(360).clamp(1, work.width.max(1));
    let height = physical(252).clamp(1, work.height.max(1));
    let centered = Bounds {
        x: cursor.0 - width / 2,
        y: cursor.1 - height / 2,
        width,
        height,
    };
    geometry::placement(work, (width, height), Some(centered))
}

fn radial_placement(work: Bounds, dpi: u32, cursor: (i32, i32)) -> Option<Bounds> {
    let dpi_scale = dpi as f32 / 96.0;
    let width = (radial::WIDTH * dpi_scale).round();
    let height = (radial::HEIGHT * dpi_scale).round();
    let scale = 1.0f32
        .min(work.width as f32 / width)
        .min(work.height as f32 / height);
    if scale < 0.85 {
        return None;
    }
    let (width, height) = (
        (width * scale).round() as i32,
        (height * scale).round() as i32,
    );
    let centered = Bounds {
        x: cursor.0 - width / 2,
        y: cursor.1 - (radial::CENTER.y * height as f32 / radial::HEIGHT).round() as i32,
        width,
        height,
    };
    Some(geometry::placement(work, (width, height), Some(centered)))
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Region {
    client: Bounds,
    origin: (i32, i32),
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
            origin: (origin.x, origin.y),
            // SAFETY: hwnd belongs to the live Workspace adapter.
            dpi: unsafe { GetDpiForWindow(hwnd) }.max(96),
        })
    }

    pub fn board_point(self, x: f64, y: f64) -> Point {
        point_on_board(x, y, self.client.width, self.client.height)
    }

    pub fn screen_to_board(self, point: (i32, i32)) -> Point {
        self.board_point(
            f64::from(point.0 - self.origin.0),
            f64::from(point.1 - self.origin.1),
        )
    }

    pub fn scale(self) -> f32 {
        self.client.width as f32 * 96.0 / self.dpi as f32 / radial::WIDTH
    }

    pub fn apply_radial<'a>(
        self,
        native: NativeWindow,
        outlines: impl IntoIterator<Item = &'a Outline>,
    ) -> io::Result<()> {
        let mut union: Option<OwnedRegion> = None;
        for outline in outlines {
            let points: Vec<_> = outline
                .points()
                .iter()
                .map(|point| POINT {
                    x: self.client.x
                        + (point.x * self.client.width as f32 / radial::WIDTH).round() as i32,
                    y: self.client.y
                        + (point.y * self.client.height as f32 / radial::HEIGHT).round() as i32,
                })
                .collect();
            // SAFETY: GDI copies the initialized array before returning.
            let piece = OwnedRegion::new(unsafe {
                CreatePolygonRgn(points.as_ptr(), points.len() as i32, ALTERNATE)
            })?;
            if let Some(union) = &mut union {
                // SAFETY: all handles are owned here. GDI permits dest and src1 to alias.
                if unsafe { CombineRgn(union.0, union.0, piece.0, RGN_OR) } == 0 {
                    return Err(io::Error::last_os_error());
                }
            } else {
                union = Some(piece);
            }
        }
        union
            .ok_or_else(|| io::Error::other("empty Workspace region"))?
            .attach(native)
    }

    pub fn apply(self, native: NativeWindow, shape: Shape) -> io::Result<()> {
        let Bounds {
            x,
            y,
            width,
            height,
        } = self.client;
        // SAFETY: points are copied by GDI and their count matches the stack array.
        let region = unsafe {
            match shape {
                Shape::Shaped => {
                    let points = [
                        (3, 0),
                        (97, 0),
                        (100, 4),
                        (100, 24),
                        (69, 40),
                        (69, 60),
                        (100, 76),
                        (100, 96),
                        (97, 100),
                        (3, 100),
                        (0, 96),
                        (0, 76),
                        (31, 60),
                        (31, 40),
                        (0, 24),
                        (0, 4),
                    ]
                    .map(|(px, py)| POINT {
                        x: x + width * px / 100,
                        y: y + height * py / 100,
                    });
                    CreatePolygonRgn(points.as_ptr(), points.len() as i32, ALTERNATE)
                }
                Shape::Rounded => {
                    let diameter = (28 * self.dpi as i32 / 96).min(width).min(height);
                    CreateRoundRectRgn(x, y, x + width, y + height, diameter, diameter)
                }
                Shape::Radial => {
                    return Err(io::Error::other("radial Workspace requires outlines"));
                }
            }
        };
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
    fn workspace_centers_on_pointer_and_stays_inside_signed_work_area() {
        let work = Bounds {
            x: -1600,
            y: -180,
            width: 1600,
            height: 860,
        };
        assert_eq!(
            placement(work, 144, (-710, 200)),
            Bounds {
                x: -980,
                y: 11,
                width: 540,
                height: 378
            }
        );
        assert_eq!(placement(work, 120, (-1600, -180)).x, work.x);
        let corner = placement(work, 120, (-1, 679));
        assert_eq!(corner.x + corner.width, 0);
        assert_eq!(corner.y + corner.height, 680);
        let tiny = Bounds {
            width: 160,
            height: 128,
            ..work
        };
        assert_eq!(placement(tiny, 96, (-1500, -100)), tiny);
    }

    #[test]
    fn radial_placement_scales_and_clamps_to_one_monitor() {
        let work = Bounds {
            x: -1600,
            y: -100,
            width: 1600,
            height: 900,
        };
        assert_eq!(
            radial_placement(work, 144, (-1, 799)),
            Some(Bounds {
                x: -630,
                y: 164,
                width: 630,
                height: 636,
            })
        );
        let compact = Bounds {
            width: 375,
            height: 385,
            ..work
        };
        let fitted = radial_placement(compact, 96, (compact.x, compact.y)).unwrap();
        assert_eq!(
            (fitted.x, fitted.y, fitted.width, fitted.height),
            (-1600, -100, 375, 379)
        );
        let tiny = Bounds {
            width: 320,
            ..compact
        };
        assert!(radial_placement(tiny, 96, (tiny.x, tiny.y)).is_none());
    }
}
