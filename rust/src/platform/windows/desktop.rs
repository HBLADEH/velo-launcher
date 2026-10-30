//! One blocking Win32 message loop owns the tray, popup menu and instance
//! activation endpoint. It never touches a Slint component or product data.

use std::cell::Cell;
use std::ptr::{null, null_mut};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread::JoinHandle;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    MONITOR_DEFAULTTONEAREST, MonitorFromPoint, MonitorFromWindow,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetSystemMetricsForDpi,
    SetThreadDpiAwarenessContext,
};
use windows_sys::Win32::UI::Shell::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use super::{
    geometry,
    instance::{self, Identity, wide},
};

const TRAY_MESSAGE: u32 = WM_APP + 20;
const OWNER_MESSAGE: u32 = WM_APP + 21;
const RETRY_TIMER: usize = 1;
const NIN_KEYSELECT: u32 = NIN_SELECT | 1; // Shell's keyboard activation notification.
const ICON: &[u8] = include_bytes!("../../../ui/assets/velo.ico");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum Event {
    Open = 1,
    Settings = 2,
    Quit = 3,
    MenuClosed = 4,
    EnvironmentChanged = 5,
    TrayRestored = 6,
    TrayUnavailable = 7,
    SecondInstance = 8,
}

struct Context {
    callback: Box<dyn Fn(Event)>,
    icon: Cell<HICON>,
    added: Cell<bool>,
    retries: Cell<u32>,
    menu_active: Arc<AtomicBool>,
    taskbar_created: u32,
    activate: u32,
}

impl Context {
    fn notify_data(&self, hwnd: HWND) -> NOTIFYICONDATAW {
        NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: 1,
            ..Default::default()
        }
    }

    fn icon_size(&self, hwnd: HWND) -> i32 {
        let mut rect = RECT::default();
        let id = NOTIFYICONIDENTIFIER {
            cbSize: std::mem::size_of::<NOTIFYICONIDENTIFIER>() as u32,
            hWnd: hwnd,
            uID: 1,
            ..Default::default()
        };
        let monitor = if unsafe { Shell_NotifyIconGetRect(&id, &mut rect) } >= 0 {
            unsafe {
                MonitorFromPoint(
                    POINT {
                        x: (rect.left + rect.right) / 2,
                        y: (rect.top + rect.bottom) / 2,
                    },
                    MONITOR_DEFAULTTONEAREST,
                )
            }
        } else {
            let taskbar = unsafe { FindWindowW(wide("Shell_TrayWnd").as_ptr(), null()) };
            unsafe { MonitorFromWindow(taskbar, MONITOR_DEFAULTTONEAREST) }
        };
        unsafe { GetSystemMetricsForDpi(SM_CXSMICON, geometry::monitor_dpi(monitor)) }.max(16)
    }

    fn refresh_icon(&self, hwnd: HWND) -> bool {
        let size = self.icon_size(hwnd);
        let Ok(icon) = create_icon(size) else {
            return false;
        };
        let mut data = self.notify_data(hwnd);
        data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP;
        data.uCallbackMessage = TRAY_MESSAGE;
        data.hIcon = icon;
        for (target, source) in data.szTip.iter_mut().zip(wide("Velo · Rust + Slint")) {
            *target = source;
        }
        let operation = if self.added.get() {
            NIM_MODIFY
        } else {
            NIM_ADD
        };
        if unsafe { Shell_NotifyIconW(operation, &data) } == 0 {
            unsafe { DestroyIcon(icon) };
            return false;
        }
        let old = self.icon.replace(icon);
        if !old.is_null() {
            unsafe { DestroyIcon(old) };
        }
        self.added.set(true);
        data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
        unsafe {
            Shell_NotifyIconW(NIM_SETVERSION, &data);
            KillTimer(hwnd, RETRY_TIMER);
        }
        eprintln!("event=tray_icon registered=true physical_size={size}");
        true
    }

    fn recover(&self, hwnd: HWND) {
        if self.refresh_icon(hwnd) {
            (self.callback)(Event::TrayRestored);
        } else {
            (self.callback)(Event::TrayUnavailable);
            self.retries.set(0);
            // Explorer may broadcast before its notification area is ready.
            // Only recovery uses a timer, capped at ten retries.
            unsafe { SetTimer(hwnd, RETRY_TIMER, 1000, None) };
        }
    }

    fn remove_icon(&self, hwnd: HWND) {
        unsafe {
            KillTimer(hwnd, RETRY_TIMER);
            Shell_NotifyIconW(NIM_DELETE, &self.notify_data(hwnd));
            let old = self.icon.replace(null_mut());
            if !old.is_null() {
                DestroyIcon(old);
            }
        }
        self.added.set(false);
    }

    fn menu(&self, hwnd: HWND) {
        if self.menu_active.swap(true, Ordering::SeqCst) {
            return;
        }
        unsafe {
            let menu = CreatePopupMenu();
            if !menu.is_null() {
                AppendMenuW(menu, MF_STRING, 1, wide("打开 Velo").as_ptr());
                AppendMenuW(menu, MF_STRING, 2, wide("设置…").as_ptr());
                AppendMenuW(menu, MF_SEPARATOR, 0, null());
                AppendMenuW(menu, MF_STRING, 3, wide("退出").as_ptr());
                SetMenuDefaultItem(menu, 1, 0);
                let mut point = POINT::default();
                GetCursorPos(&mut point);
                SetForegroundWindow(hwnd);
                let selected = TrackPopupMenu(
                    menu,
                    TPM_RIGHTBUTTON | TPM_RETURNCMD,
                    point.x,
                    point.y,
                    0,
                    hwnd,
                    null(),
                );
                DestroyMenu(menu);
                PostMessageW(hwnd, WM_NULL, 0, 0);
                match selected {
                    1 => (self.callback)(Event::Open),
                    2 => (self.callback)(Event::Settings),
                    3 => (self.callback)(Event::Quit),
                    _ => {}
                }
            }
        }
        self.menu_active.store(false, Ordering::SeqCst);
        (self.callback)(Event::MenuClosed);
    }
}

unsafe extern "system" fn procedure(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_NCCREATE {
        let create = unsafe { &*(lparam as *const CREATESTRUCTW) };
        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize) };
    }
    let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const Context;
    if ptr.is_null() {
        return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
    }
    // Shared reference only: TrackPopupMenu can reenter this procedure. All
    // mutable window state uses Cells; the allocation outlives DestroyWindow.
    let context = unsafe { &*ptr };
    match message {
        msg if msg == context.activate => {
            (context.callback)(Event::SecondInstance);
            1
        }
        msg if msg == context.taskbar_created => {
            context.remove_icon(hwnd);
            context.recover(hwnd);
            0
        }
        TRAY_MESSAGE => {
            match lparam as u32 & 0xffff {
                NIN_SELECT | NIN_KEYSELECT => (context.callback)(Event::Open),
                WM_CONTEXTMENU => context.menu(hwnd),
                _ => {}
            }
            0
        }
        OWNER_MESSAGE => {
            unsafe { SetWindowLongPtrW(hwnd, GWLP_HWNDPARENT, wparam as isize) };
            0
        }
        WM_DISPLAYCHANGE | WM_SETTINGCHANGE | WM_DPICHANGED => {
            context.recover(hwnd);
            (context.callback)(Event::EnvironmentChanged);
            0
        }
        WM_POWERBROADCAST if matches!(wparam, 7 | 18) => {
            context.recover(hwnd);
            (context.callback)(Event::EnvironmentChanged);
            1
        }
        WM_TIMER if wparam == RETRY_TIMER => {
            context.retries.set(context.retries.get() + 1);
            if context.refresh_icon(hwnd) {
                (context.callback)(Event::TrayRestored);
            } else if context.retries.get() >= 10 {
                unsafe { KillTimer(hwnd, RETRY_TIMER) };
            }
            0
        }
        WM_CLOSE => {
            if context.menu_active.load(Ordering::SeqCst) {
                unsafe { EndMenu() };
            }
            unsafe { DestroyWindow(hwnd) };
            0
        }
        WM_DESTROY => {
            context.remove_icon(hwnd);
            unsafe { PostQuitMessage(0) };
            0
        }
        WM_NCDESTROY => {
            unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) };
            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

// The embedded multi-resolution ICO is the repository's existing Velo icon.
// Pick a source at least as large as the requested taskbar DPI before scaling.
fn create_icon(size: i32) -> Result<HICON, String> {
    let count = u16::from_le_bytes([ICON[4], ICON[5]]) as usize;
    let mut entries = (0..count)
        .filter_map(|index| {
            let entry = ICON.get(6 + index * 16..6 + (index + 1) * 16)?;
            let width = if entry[0] == 0 {
                256
            } else {
                i32::from(entry[0])
            };
            let length = u32::from_le_bytes(entry[8..12].try_into().ok()?) as usize;
            let offset = u32::from_le_bytes(entry[12..16].try_into().ok()?) as usize;
            Some((width, ICON.get(offset..offset.checked_add(length)?)?))
        })
        .collect::<Vec<_>>();
    entries.sort_by_key(|(width, _)| *width);
    let (_, pixels) = entries
        .iter()
        .find(|(width, _)| *width >= size)
        .or(entries.last())
        .ok_or("Invalid embedded ICO")?;
    let icon = unsafe {
        CreateIconFromResourceEx(
            pixels.as_ptr(),
            pixels.len() as u32,
            1,
            0x00030000,
            size,
            size,
            LR_DEFAULTCOLOR,
        )
    };
    if icon.is_null() {
        Err(std::io::Error::last_os_error().to_string())
    } else {
        Ok(icon)
    }
}

pub struct Service {
    hwnd: isize,
    menu_active: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Service {
    pub fn start(
        identity: &Identity,
        callback: impl Fn(Event) + Send + 'static,
    ) -> Result<Self, String> {
        let title = wide(&identity.window_title());
        let menu_active = Arc::new(AtomicBool::new(false));
        let menu = menu_active.clone();
        let (ready, result) = mpsc::sync_channel(1);
        let worker = std::thread::Builder::new()
            .name("velo-desktop".into())
            .spawn(move || {
                let class = wide(instance::WINDOW_CLASS);
                unsafe {
                    SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
                    let instance = GetModuleHandleW(null());
                    let window_class = WNDCLASSW {
                        lpfnWndProc: Some(procedure),
                        hInstance: instance,
                        lpszClassName: class.as_ptr(),
                        ..Default::default()
                    };
                    if RegisterClassW(&window_class) == 0 {
                        let _ = ready.send(Err(std::io::Error::last_os_error().to_string()));
                        return;
                    }
                    let context = Box::new(Context {
                        callback: Box::new(callback),
                        icon: Cell::new(null_mut()),
                        added: Cell::new(false),
                        retries: Cell::new(0),
                        menu_active: menu,
                        taskbar_created: RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()),
                        activate: instance::activation_message(),
                    });
                    let hwnd = CreateWindowExW(
                        WS_EX_TOOLWINDOW,
                        class.as_ptr(),
                        title.as_ptr(),
                        WS_POPUP,
                        0,
                        0,
                        0,
                        0,
                        null_mut(),
                        null_mut(),
                        instance,
                        (&*context as *const Context).cast(),
                    );
                    if hwnd.is_null() {
                        let _ = ready.send(Err(std::io::Error::last_os_error().to_string()));
                    } else {
                        context.recover(hwnd);
                        let _ = ready.send(Ok(hwnd as isize));
                        let mut message = MSG::default();
                        while GetMessageW(&mut message, null_mut(), 0, 0) > 0 {
                            TranslateMessage(&message);
                            DispatchMessageW(&message);
                        }
                        if IsWindow(hwnd) != 0 {
                            DestroyWindow(hwnd);
                        }
                        eprintln!("event=desktop_stopped tray_removed=true");
                    }
                    UnregisterClassW(class.as_ptr(), instance);
                }
            })
            .map_err(|e| e.to_string())?;
        match result.recv().map_err(|e| e.to_string())? {
            Ok(hwnd) => Ok(Self {
                hwnd,
                menu_active,
                worker: Some(worker),
            }),
            Err(error) => {
                let _ = worker.join();
                Err(error)
            }
        }
    }

    pub fn set_owner(&self, owner: isize) {
        unsafe { PostMessageW(self.hwnd as HWND, OWNER_MESSAGE, owner as usize, 0) };
    }
    pub fn menu_active(&self) -> bool {
        self.menu_active.load(Ordering::SeqCst)
    }
    pub fn foreground(&self) -> bool {
        unsafe { GetForegroundWindow() == self.hwnd as HWND }
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        unsafe { PostMessageW(self.hwnd as HWND, WM_CLOSE, 0, 0) };
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn native_endpoint_second_launch_tray_recovery_and_cleanup() {
        let identity = Identity::resolve(
            Some(format!("desktop-test-{}", std::process::id())),
            Some(std::env::temp_dir().join(format!("velo-desktop-test-{}", std::process::id()))),
        )
        .unwrap();
        let instance::Acquisition::Primary(guard) = identity.acquire().unwrap() else {
            panic!("not primary")
        };
        let (events, received) = mpsc::channel();
        let service = Service::start(&identity, move |event| {
            let _ = events.send(event);
        })
        .unwrap();
        assert_eq!(
            received.recv_timeout(Duration::from_secs(2)).unwrap(),
            Event::TrayRestored
        );
        assert!(matches!(
            identity.acquire(),
            Ok(instance::Acquisition::Forwarded)
        ));
        assert_eq!(
            received.recv_timeout(Duration::from_secs(2)).unwrap(),
            Event::SecondInstance
        );
        unsafe {
            PostMessageW(
                service.hwnd as HWND,
                RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()),
                0,
                0,
            );
        }
        assert_eq!(
            received.recv_timeout(Duration::from_secs(2)).unwrap(),
            Event::TrayRestored
        );
        let hwnd = service.hwnd;
        drop(service);
        assert_eq!(unsafe { IsWindow(hwnd as HWND) }, 0);
        drop(guard);
        assert!(matches!(
            identity.acquire(),
            Ok(instance::Acquisition::Primary(_))
        ));
    }
}
