//! Thread-owned RegisterHotKey registrations. Rebinding acquires the new chord
//! before releasing the old one; a conflict never leaves the launcher stranded.

use std::fmt;
use std::ptr::null_mut;
use std::sync::mpsc::{self, SyncSender};
use std::thread::JoinHandle;

use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetMessageW, MSG, PM_NOREMOVE, PeekMessageW, PostThreadMessageW, WM_APP, WM_HOTKEY, WM_QUIT,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key {
    pub modifiers: u32,
    pub code: u32,
}

impl Key {
    pub fn parse(text: &str) -> Result<Self, String> {
        let parts: Vec<_> = text.trim().split('+').map(str::trim).collect();
        if parts.len() < 2 {
            return Err("快捷键须包含 Ctrl、Alt、Shift 或 Win 修饰键".into());
        }
        let mut modifiers = 0;
        for part in &parts[..parts.len() - 1] {
            let flag = match part.to_ascii_uppercase().as_str() {
                "ALT" => 1,
                "CTRL" | "CONTROL" => 2,
                "SHIFT" => 4,
                "WIN" | "SUPER" => 8,
                _ => return Err(format!("未知修饰键: {part}")),
            };
            if modifiers & flag != 0 {
                return Err(format!("重复修饰键: {part}"));
            }
            modifiers |= flag;
        }
        let last = parts.last().unwrap().to_ascii_uppercase();
        let code = match last.as_str() {
            "SPACE" => 0x20,
            "ENTER" => 0x0d,
            "TAB" => 9,
            _ if last.len() == 1 && last.as_bytes()[0].is_ascii_alphanumeric() => {
                u32::from(last.as_bytes()[0])
            }
            _ => last
                .strip_prefix('F')
                .and_then(|number| number.parse::<u32>().ok())
                .filter(|number| (1..=24).contains(number))
                .map(|number| 0x70 + number - 1)
                .ok_or("支持字母、数字、Space、Enter、Tab 或 F1–F24")?,
        };
        Ok(Self { modifiers, code })
    }

    pub fn from_native(code: u32, modifiers: u32) -> Option<Self> {
        let key = Self { modifiers, code };
        (modifiers != 0 && Self::parse(&key.to_string()).is_ok()).then_some(key)
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (flag, name) in [(2, "Ctrl"), (1, "Alt"), (4, "Shift"), (8, "Win")] {
            if self.modifiers & flag != 0 {
                write!(f, "{name}+")?;
            }
        }
        match self.code {
            0x20 => f.write_str("Space"),
            0x0d => f.write_str("Enter"),
            9 => f.write_str("Tab"),
            0x70..=0x87 => write!(f, "F{}", self.code - 0x70 + 1),
            0x30..=0x39 | 0x41..=0x5a => write!(f, "{}", char::from_u32(self.code).unwrap()),
            _ => f.write_str("Unsupported"),
        }
    }
}

pub enum Event {
    Pressed(Key),
    Stopped,
}

enum Request {
    Bind(Key, SyncSender<Result<(), String>>),
}

struct Registration {
    id: i32,
    key: Key,
}

impl Registration {
    fn acquire(id: i32, key: Key) -> Result<Self, String> {
        // SAFETY: thread-owned registration; no HWND or borrowed memory crosses threads.
        if unsafe { RegisterHotKey(null_mut(), id, key.modifiers | MOD_NOREPEAT, key.code) } == 0 {
            return Err(format!(
                "快捷键被占用或不可用: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(Self { id, key })
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        // SAFETY: Drop runs on the same thread that acquired this registration.
        unsafe { UnregisterHotKey(null_mut(), self.id) };
    }
}

pub struct Service {
    sender: SyncSender<Request>,
    thread_id: u32,
    worker: Option<JoinHandle<()>>,
}

impl Service {
    pub fn start(callback: impl Fn(Event) + Send + 'static) -> std::io::Result<Self> {
        let (sender, requests) = mpsc::sync_channel(1);
        let (ready, thread_id) = mpsc::sync_channel(1);
        let worker = std::thread::Builder::new()
            .name("velo-hotkey".into())
            .spawn(move || {
                let mut message = MSG::default();
                // SAFETY: initialize this thread's message queue before publishing its ID.
                unsafe { PeekMessageW(&mut message, null_mut(), 0, 0, PM_NOREMOVE) };
                let _ = ready.send(unsafe { GetCurrentThreadId() });
                let mut registration: Option<Registration> = None;
                // GetMessage blocks while idle. All registrations and requests are serialized here.
                while unsafe { GetMessageW(&mut message, null_mut(), 0, 0) } > 0 {
                    if message.message == WM_HOTKEY {
                        if let Some(current) = &registration
                            && message.wParam == current.id as usize
                            && (message.lParam as u32 & 0x0f) == current.key.modifiers
                            && ((message.lParam as u32 >> 16) & 0xffff) == current.key.code
                        {
                            callback(Event::Pressed(current.key));
                        }
                    } else if message.message == WM_APP + 1 {
                        match requests.recv() {
                            Ok(Request::Bind(key, result)) => {
                                let outcome = if registration.as_ref().is_some_and(|r| r.key == key)
                                {
                                    Ok(())
                                } else {
                                    let next = registration.as_ref().map_or(1, |r| 3 - r.id);
                                    Registration::acquire(next, key)
                                        .map(|new| registration = Some(new))
                                };
                                let _ = result.send(outcome);
                            }
                            Err(_) => return,
                        }
                    }
                }
                drop(registration);
                callback(Event::Stopped);
            })?;
        let thread_id = thread_id.recv().map_err(std::io::Error::other)?;
        Ok(Self {
            sender,
            thread_id,
            worker: Some(worker),
        })
    }

    pub fn bind(&self, key: Key) -> Result<(), String> {
        let (sender, result) = mpsc::sync_channel(1);
        self.sender
            .try_send(Request::Bind(key, sender))
            .map_err(|_| "快捷键服务已停止或忙碌")?;
        // SAFETY: a successful start guarantees the target owns a message queue.
        if unsafe { PostThreadMessageW(self.thread_id, WM_APP + 1, 0, 0) } == 0 {
            return Err(format!(
                "快捷键服务不可用: {}",
                std::io::Error::last_os_error()
            ));
        }
        result.recv().map_err(|_| "快捷键服务已停止")?
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        // Do not enqueue into a possibly full request channel during shutdown.
        unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, 0, 0) };
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
    fn parser_preserves_the_go_chord_contract() {
        for text in [
            "Alt+Space",
            "Ctrl+Alt+K",
            "Win+Shift+F12",
            " control + shift + f24 ",
            "Alt+9",
        ] {
            let parsed = Key::parse(text).unwrap();
            assert_eq!(Key::parse(&parsed.to_string()).unwrap(), parsed);
        }
        for text in [
            "Space",
            "Ctrl+Ctrl+A",
            "Ctrl+F25",
            "Meta+Q",
            "Alt+",
            "Shift+Unknown",
            "Alt++A",
        ] {
            assert!(Key::parse(text).is_err(), "accepted {text}");
        }
        assert_eq!(Key::from_native(0x41, 2).unwrap().to_string(), "Ctrl+A");
        assert!(Key::from_native(0x10, 4).is_none());
        assert!(Key::from_native(0x41, 0).is_none());
    }

    #[test]
    fn native_conflict_rollback_dispatch_rebind_and_cleanup() {
        fn post(service: &Service, id: usize, key: Key) {
            unsafe {
                PostThreadMessageW(
                    service.thread_id,
                    WM_HOTKEY,
                    id,
                    ((key.code << 16) | key.modifiers) as isize,
                )
            };
        }
        let (events, received) = mpsc::sync_channel(16);
        let service = Service::start(move |event| {
            if let Event::Pressed(key) = event {
                let _ = events.send(key);
            }
        })
        .unwrap();
        let old = Key::parse("Ctrl+Alt+Shift+F24").unwrap();
        let conflict = Key::parse("Ctrl+Alt+Shift+F23").unwrap();
        let new = Key::parse("Ctrl+Alt+Shift+F22").unwrap();
        let other = Service::start(|_| {}).unwrap();
        service.bind(old).unwrap();
        other.bind(conflict).unwrap();
        assert!(service.bind(conflict).is_err());
        assert!(other.bind(old).is_err());
        post(&service, 1, old);
        assert_eq!(received.recv_timeout(Duration::from_secs(2)).unwrap(), old);
        service.bind(new).unwrap();
        other.bind(old).unwrap();
        // A stale queued notification from the former ID must be ignored.
        post(&service, 1, old);
        post(&service, 2, new);
        assert_eq!(received.recv_timeout(Duration::from_secs(2)).unwrap(), new);
        assert!(received.try_recv().is_err());
        for _ in 0..12 {
            post(&service, 2, new);
        }
        for _ in 0..12 {
            assert_eq!(received.recv_timeout(Duration::from_secs(2)).unwrap(), new);
        }
        let third = Key::parse("Ctrl+Alt+Shift+F21").unwrap();
        service.bind(third).unwrap();
        // The first ID is reused now; lParam must also match the current chord.
        post(&service, 1, old);
        post(&service, 1, third);
        assert_eq!(
            received.recv_timeout(Duration::from_secs(2)).unwrap(),
            third
        );
        assert!(received.try_recv().is_err());
        drop(service);
        other.bind(third).unwrap();
        drop(other);
        let fresh = Service::start(|_| {}).unwrap();
        fresh.bind(new).unwrap();
    }
}
