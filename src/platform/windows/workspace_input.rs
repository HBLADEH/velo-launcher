//! Opt-in mouse hook installed and released by its native message thread.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::io;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::SystemInformation::GetTickCount;
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, HHOOK, KillTimer, MSG, MSLLHOOKSTRUCT, PM_NOREMOVE, PeekMessageW,
    PostThreadMessageW, SetTimer, SetWindowsHookExW, UnhookWindowsHookEx, WH_MOUSE_LL, WM_APP,
    WM_MOUSEMOVE, WM_TIMER, WM_XBUTTONDOWN, WM_XBUTTONUP, XBUTTON1, XBUTTON2,
};

use super::workspace::{Anchor, Foreground, Invocation, current_input_desktop};
use super::workspace_actions::Resolver;
use crate::core::workspace::config::{AppKey, MouseButton};

const STOP: u32 = WM_APP + 7;
const MAX_EVENTS: usize = 16;
const CAPTURE_LIMIT_MS: u32 = 8_000;
const CAPTURE_CHECK_MS: u32 = 200;
const DRAIN_MS: u32 = 250;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    XButton1,
    XButton2,
}

impl Button {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "xbutton1" => Some(Self::XButton1),
            "xbutton2" => Some(Self::XButton2),
            _ => None,
        }
    }

    fn mouse_data(self) -> u16 {
        match self {
            Self::XButton1 => XBUTTON1,
            Self::XButton2 => XBUTTON2,
        }
    }

    pub fn from_preference(button: MouseButton) -> Self {
        match button {
            MouseButton::XButton1 => Self::XButton1,
            MouseButton::XButton2 => Self::XButton2,
        }
    }
}

#[derive(Default)]
pub struct Admission {
    pub enabled: AtomicBool,
    pub suspended: AtomicBool,
    excluded: Mutex<Vec<AppKey>>,
}

impl Admission {
    pub fn set_excluded(&self, excluded: Vec<AppKey>) {
        if let Ok(mut policy) = self.excluded.lock() {
            *policy = excluded;
        }
    }

    fn accepts(&self, window: Foreground) -> bool {
        if !self.enabled.load(Ordering::Acquire) || self.suspended.load(Ordering::Acquire) {
            return false;
        }
        let Ok(excluded) = self.excluded.try_lock() else {
            return false;
        };
        excluded.is_empty()
            || Resolver::exclusion_identity(window).is_some_and(|key| !excluded.contains(&key))
    }
}

#[derive(Clone, Copy)]
pub struct Sample {
    pub at: Instant,
    pub point: (i32, i32),
    pub distance: f64,
}

pub enum Event {
    Pressed(Invocation),
    Moved(Sample),
    Released(Sample),
    Cancelled(&'static str),
    Stopped,
}

#[derive(Default)]
struct Inbox(VecDeque<Event>);

impl Inbox {
    fn push(&mut self, event: Event) -> (bool, bool) {
        let wake = self.0.is_empty();
        if matches!(event, Event::Moved(_)) && matches!(self.0.back(), Some(Event::Moved(_))) {
            self.0.pop_back();
        }
        if self.0.len() == MAX_EVENTS {
            self.0.clear();
            self.0.push_back(Event::Cancelled("input_overflow"));
            return (false, wake);
        }
        self.0.push_back(event);
        (true, wake)
    }
}

struct Capture {
    started: Instant,
    origin: (i32, i32),
    distance: f64,
    cancelled: bool,
}

impl Capture {
    fn sample(&mut self, point: (i32, i32), at: Instant) -> Sample {
        let dx = f64::from(point.0) - f64::from(self.origin.0);
        let dy = f64::from(point.1) - f64::from(self.origin.1);
        self.distance = self.distance.max(dx.hypot(dy));
        Sample {
            at,
            point,
            distance: self.distance,
        }
    }
}

struct HookState {
    button: Button,
    capture: Option<Capture>,
    inbox: Arc<Mutex<Inbox>>,
    notify: Box<dyn Fn() + Send>,
    admission: Arc<Admission>,
    stopping: bool,
    timer: usize,
}

impl HookState {
    fn send(&self, event: Event) -> bool {
        let Some((accepted, wake)) = self.inbox.lock().ok().map(|mut queue| queue.push(event))
        else {
            return false;
        };
        if wake {
            (self.notify)();
        }
        accepted
    }

    fn timer(&mut self, ms: u32) -> bool {
        self.clear_timer();
        // SAFETY: the timer belongs to the current thread's message queue.
        self.timer = unsafe { SetTimer(null_mut(), 0, ms, None) };
        self.timer != 0
    }

    fn clear_timer(&mut self) {
        if self.timer != 0 {
            // SAFETY: called on the same thread that set this timer.
            unsafe { KillTimer(null_mut(), self.timer) };
            self.timer = 0;
        }
    }

    fn mouse(&mut self, message: u32, raw: &MSLLHOOKSTRUCT) -> bool {
        let moving = message == WM_MOUSEMOVE;
        if moving
            && self
                .capture
                .as_ref()
                .is_none_or(|capture| capture.cancelled)
        {
            return false;
        }
        if !moving
            && (!matches!(message, WM_XBUTTONDOWN | WM_XBUTTONUP)
                || (raw.mouseData >> 16) as u16 != self.button.mouse_data())
        {
            return false;
        }
        let point = (raw.pt.x, raw.pt.y);
        // SAFETY: GetTickCount uses the same timestamp units as MSLLHOOKSTRUCT.
        let delay = unsafe { GetTickCount() }.wrapping_sub(raw.time);
        let now = Instant::now();
        let at = now
            .checked_sub(Duration::from_millis(u64::from(delay)))
            .unwrap_or(now);
        if moving {
            if let Some(capture) = self.capture.as_mut().filter(|capture| !capture.cancelled) {
                let sample = capture.sample(point, at);
                if !self.send(Event::Moved(sample)) {
                    self.cancel("input_overflow");
                }
            }
            return false;
        }
        match message {
            WM_XBUTTONDOWN
                if self
                    .capture
                    .as_ref()
                    .is_some_and(|capture| !capture.cancelled) =>
            {
                true
            }
            WM_XBUTTONDOWN if !self.stopping => {
                self.capture = None;
                let Some(foreground) = Foreground::current()
                    .filter(|target| self.admission.accepts(*target) && target.is_windowed())
                else {
                    return false;
                };
                let Some(anchor) = Anchor::capture(point) else {
                    return false;
                };
                if !self.timer(CAPTURE_CHECK_MS) {
                    return false;
                }
                if !self.send(Event::Pressed(Invocation {
                    at,
                    foreground: Some(foreground),
                    anchor: Some(anchor),
                })) {
                    self.clear_timer();
                    return false;
                }
                self.capture = Some(Capture {
                    started: at,
                    origin: point,
                    distance: 0.0,
                    cancelled: false,
                });
                true
            }
            WM_XBUTTONUP => {
                let Some(mut capture) = self.capture.take() else {
                    return false;
                };
                if !self.stopping {
                    self.clear_timer();
                }
                if !capture.cancelled {
                    self.send(Event::Released(capture.sample(point, at)));
                }
                true
            }
            _ => false,
        }
    }

    fn cancel(&mut self, reason: &'static str) {
        if let Some(capture) = &mut self.capture {
            capture.cancelled = true;
        }
        self.send(Event::Cancelled(reason));
    }
}

thread_local! {
    static HOOK: RefCell<Option<HookState>> = const { RefCell::new(None) };
}

unsafe extern "system" fn on_mouse(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == 0 && lparam != 0 {
        // SAFETY: HC_ACTION supplies a valid read-only event during the call.
        let event = unsafe { &*(lparam as *const MSLLHOOKSTRUCT) };
        let consumed = HOOK.with(|state| {
            state
                .try_borrow_mut()
                .ok()
                .and_then(|mut state| state.as_mut().map(|hook| hook.mouse(wparam as u32, event)))
                .unwrap_or(false)
        });
        if consumed {
            return 1;
        }
    }
    // SAFETY: events not owned by this service retain the original parameters.
    unsafe { CallNextHookEx(null_mut(), code, wparam, lparam) }
}

struct InstalledHook(HHOOK);

impl Drop for InstalledHook {
    fn drop(&mut self) {
        // SAFETY: the message thread owns this registration.
        unsafe { UnhookWindowsHookEx(self.0) };
    }
}

pub struct Service {
    thread: Option<JoinHandle<()>>,
    id: u32,
    inbox: Arc<Mutex<Inbox>>,
}

impl Service {
    pub fn start(
        button: Button,
        admission: Arc<Admission>,
        notify: impl Fn() + Send + 'static,
    ) -> io::Result<Self> {
        let (ready, receive) = mpsc::sync_channel(1);
        let inbox = Arc::new(Mutex::new(Inbox::default()));
        let worker_inbox = inbox.clone();
        let thread = thread::Builder::new()
            .name("velo-workspace-mouse".into())
            .spawn(move || {
                let mut message = MSG::default();
                // SAFETY: create the thread queue before publishing its ID. The low
                // level hook runs on this message thread and outlives its callback.
                let hook = unsafe {
                    PeekMessageW(&mut message, null_mut(), 0, 0, PM_NOREMOVE);
                    SetWindowsHookExW(WH_MOUSE_LL, Some(on_mouse), GetModuleHandleW(null_mut()), 0)
                };
                if hook.is_null() {
                    let _ = ready.send(Err(io::Error::last_os_error()));
                    return;
                }
                let hook = InstalledHook(hook);
                HOOK.with(|state| {
                    state.replace(Some(HookState {
                        button,
                        capture: None,
                        inbox: worker_inbox,
                        notify: Box::new(notify),
                        admission,
                        stopping: false,
                        timer: 0,
                    }));
                });
                // SAFETY: the queue and registration have already been initialized.
                if ready.send(Ok(unsafe { GetCurrentThreadId() })).is_err() {
                    HOOK.with(|state| state.replace(None));
                    return;
                }
                // SAFETY: all dispatched queue messages are handled on this thread.
                while unsafe { GetMessageW(&mut message, null_mut(), 0, 0) } > 0 {
                    let done = HOOK.with(|state| {
                        let mut state = state.borrow_mut();
                        let Some(state) = state.as_mut() else {
                            return true;
                        };
                        match message.message {
                            STOP => {
                                state.stopping = true;
                                if state.capture.is_some() {
                                    state.cancel("input_disabled");
                                    if !state.timer(DRAIN_MS) {
                                        return true;
                                    }
                                }
                            }
                            WM_TIMER if message.wParam == state.timer => {
                                if state.stopping {
                                    return true;
                                }
                                if !current_input_desktop() {
                                    state.cancel("input_desktop_changed");
                                    state.clear_timer();
                                } else if state.capture.as_ref().is_some_and(|cap| {
                                    cap.started.elapsed()
                                        >= Duration::from_millis(u64::from(CAPTURE_LIMIT_MS))
                                }) {
                                    state.cancel("release_timeout");
                                    state.clear_timer();
                                }
                            }
                            _ => {}
                        }
                        state.stopping && state.capture.is_none()
                    });
                    if done {
                        break;
                    }
                }
                drop(hook);
                HOOK.with(|state| {
                    if let Some(mut state) = state.take() {
                        state.clear_timer();
                        state.send(Event::Stopped);
                    }
                });
            })?;
        match receive.recv() {
            Ok(Ok(id)) => Ok(Self {
                thread: Some(thread),
                id,
                inbox,
            }),
            failure => {
                let _ = thread.join();
                Err(match failure {
                    Ok(Err(error)) => error,
                    _ => io::Error::other("Workspace mouse hook worker stopped before setup"),
                })
            }
        }
    }

    pub fn drain(&self) -> VecDeque<Event> {
        self.inbox
            .lock()
            .map(|mut queue| std::mem::take(&mut queue.0))
            .unwrap_or_default()
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        // SAFETY: ID is published once the worker owns an initialized queue.
        unsafe { PostThreadMessageW(self.id, STOP, 0, 0) };
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_coalesce_without_discarding_edges_or_peak_travel() {
        let at = Instant::now();
        let point = (0, 0);
        let mut capture = Capture {
            started: at,
            origin: point,
            distance: 0.0,
            cancelled: false,
        };
        let mut queue = Inbox::default();
        assert_eq!(
            queue.push(Event::Pressed(Invocation {
                at,
                foreground: None,
                anchor: None,
            })),
            (true, true)
        );
        assert_eq!(
            queue.push(Event::Moved(capture.sample((400, 0), at))),
            (true, false)
        );
        assert_eq!(
            queue.push(Event::Moved(capture.sample(point, at))),
            (true, false)
        );
        assert_eq!(
            queue.push(Event::Released(capture.sample(point, at))),
            (true, false)
        );
        assert!(matches!(queue.0[0], Event::Pressed(_)));
        assert!(
            matches!(queue.0[1], Event::Moved(sample) if sample.point == point && sample.distance == 400.0)
        );
        assert!(matches!(queue.0[2], Event::Released(_)));
    }
}
