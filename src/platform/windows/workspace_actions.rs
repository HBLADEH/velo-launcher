use std::collections::VecDeque;
use std::io;
use std::mem::size_of;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use windows::Win32::UI::Shell::{FOLDERID_Documents, FOLDERID_Downloads, FOLDERID_Pictures};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyboardLayout, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT,
    KEYEVENTF_KEYUP, SendInput, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT, VkKeyScanExW,
};

use crate::core::workspace::{
    actions::{Failure, Success, WorkspaceAction},
    config::AppKey,
};

use super::{
    launch,
    processes::AppProcess,
    shell,
    workspace::{Foreground, Invocation, current_input_desktop},
};

#[derive(Clone)]
pub struct Application {
    pub key: AppKey,
    born: u64,
}

fn resolve(window: Foreground, cached: impl FnOnce(u64) -> Option<AppKey>) -> Option<Application> {
    if !window.exists() {
        return None;
    }
    let process = AppProcess::open(window.process()).ok()?;
    if !integrity_allows_shortcut(&process) {
        return None;
    }
    let born = process.born()?;
    let key = cached(born).or_else(|| process.key())?;
    (process.born() == Some(born) && window.exists()).then_some(Application { key, born })
}

fn integrity_allows_shortcut(process: &AppProcess) -> bool {
    process
        .integrity()
        .zip(AppProcess::own_integrity())
        .is_some_and(|(target, own)| target <= own)
}

const CACHE_CAPACITY: usize = 32;
const CACHE_LIFETIME: Duration = Duration::from_secs(15);

struct Lookup {
    request: Mutex<(bool, Option<Invocation>)>,
    wake: Condvar,
    result: Mutex<Option<(Instant, Foreground, Application)>>,
}

pub struct Resolver {
    shared: Arc<Lookup>,
    worker: Option<JoinHandle<()>>,
}

impl Resolver {
    pub fn start() -> io::Result<Self> {
        let shared = Arc::new(Lookup {
            request: Mutex::new((false, None)),
            wake: Condvar::new(),
            result: Mutex::new(None),
        });
        let state = shared.clone();
        let worker = thread::Builder::new()
            .name("velo-workspace-context".into())
            .spawn(move || {
                let mut cache: VecDeque<(Foreground, Instant, Application)> = VecDeque::new();
                loop {
                    let Ok(queue) = state.request.lock() else {
                        break;
                    };
                    let Ok(mut queue) = state
                        .wake
                        .wait_while(queue, |(stop, request)| !*stop && request.is_none())
                    else {
                        break;
                    };
                    if queue.0 {
                        break;
                    }
                    let request = queue.1.take();
                    drop(queue);
                    let Some(request) = request else { continue };
                    let Some(window) = request.foreground else {
                        continue;
                    };
                    cache.retain(|(_, at, _)| at.elapsed() < CACHE_LIFETIME);
                    let application = resolve(window, |born| {
                        cache
                            .iter()
                            .find(|(target, _, app)| {
                                target.process() == window.process() && app.born == born
                            })
                            .map(|(_, _, app)| app.key.clone())
                    });
                    if let Some(app) = &application {
                        cache.retain(|(target, _, _)| *target != window);
                        if cache.len() == CACHE_CAPACITY {
                            cache.pop_front();
                        }
                        cache.push_back((window, Instant::now(), app.clone()));
                    }
                    if let Ok(mut result) = state.result.lock() {
                        *result = application.map(|app| (Instant::now(), window, app));
                    }
                }
            })?;
        Ok(Self {
            shared,
            worker: Some(worker),
        })
    }

    pub fn request(&self, invocation: Invocation) {
        if invocation.foreground.is_some()
            && let Ok(mut pending) = self.shared.request.lock()
        {
            pending.1 = Some(invocation);
            self.shared.wake.notify_one();
        }
    }

    pub fn recent(&self, window: Option<Foreground>) -> Option<Application> {
        let window = window?;
        let result = self.shared.result.lock().ok()?;
        let (received, target, app) = result.as_ref()?;
        (*target == window && received.elapsed() < CACHE_LIFETIME && window.exists())
            .then(|| app.clone())
    }

    pub(super) fn exclusion_identity(window: Foreground) -> Option<AppKey> {
        if !window.exists() {
            return None;
        }
        AppProcess::open(window.process()).ok()?.key_without_disk()
    }
}

impl Drop for Resolver {
    fn drop(&mut self) {
        if let Ok(mut request) = self.shared.request.lock() {
            request.0 = true;
            self.shared.wake.notify_one();
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub fn open(action: WorkspaceAction, owner: isize) -> Result<Success, Failure> {
    let _apartment = shell::Apartment::new().map_err(|_| Failure::Shell)?;
    let id = match action {
        WorkspaceAction::Downloads => &FOLDERID_Downloads,
        WorkspaceAction::Documents => &FOLDERID_Documents,
        WorkspaceAction::Pictures => &FOLDERID_Pictures,
        WorkspaceAction::ThisPc => {
            launch::open_url("shell:MyComputerFolder").map_err(|_| Failure::Shell)?;
            return Ok(Success::Opened);
        }
        _ => return Err(Failure::Unavailable),
    };
    let path = shell::known_folder(id).map_err(|_| Failure::Unavailable)?;
    launch::open_native(&path.to_string_lossy(), owner, false).map_err(|_| Failure::Shell)?;
    Ok(Success::Opened)
}

pub fn shortcut(
    action: WorkspaceAction,
    original: Foreground,
    application: Application,
) -> Result<Success, Failure> {
    use WorkspaceAction::*;
    if original.process() == std::process::id() || Foreground::current() != Some(original) {
        return Err(Failure::TargetChanged);
    }
    let process = AppProcess::open(original.process()).map_err(|_| Failure::Permission)?;
    if process.born() != Some(application.born) {
        return Err(Failure::TargetChanged);
    }
    if !integrity_allows_shortcut(&process) || !current_input_desktop() {
        return Err(Failure::Permission);
    }
    let (shift, key) = match action {
        CodeCommands => (true, b'P'.into()),
        CodeFind => (false, b'F'.into()),
        EdgeNewTab => (false, b'T'.into()),
        EdgeAddress => (false, b'L'.into()),
        EdgeBookmarks => (true, b'O'.into()),
        CodeTerminal => {
            // SAFETY: the checked target thread owns this keyboard layout.
            let code =
                unsafe { VkKeyScanExW(u16::from(b'\x60'), GetKeyboardLayout(original.thread())) };
            if !(1..=255).contains(&code) {
                return Err(Failure::Keyboard);
            }
            (false, code as u16)
        }
        _ => return Err(Failure::Unavailable),
    };
    if [VK_CONTROL, VK_MENU, VK_SHIFT, VK_LWIN, VK_RWIN, key]
        .into_iter()
        // SAFETY: this only queries keys, after authorization and desktop checks.
        .any(|vk| unsafe { GetAsyncKeyState(i32::from(vk)) } < 0)
    {
        return Err(Failure::KeysHeld);
    }
    if Foreground::current() != Some(original) || process.born() != Some(application.born) {
        return Err(Failure::TargetChanged);
    }
    let shifted = [VK_CONTROL, VK_SHIFT, key];
    let plain = [VK_CONTROL, key];
    let keys = if shift {
        shifted.as_slice()
    } else {
        plain.as_slice()
    };
    let packets = keys
        .iter()
        .map(|&key| packet(key, false))
        .chain(keys.iter().rev().map(|&key| packet(key, true)))
        .collect::<Vec<_>>();
    // SAFETY: SendInput synchronously copies this batch of paired key events.
    let accepted = unsafe {
        SendInput(
            packets.len() as u32,
            packets.as_ptr(),
            size_of::<INPUT>() as i32,
        )
    } as usize;
    if accepted == packets.len() {
        return Ok(Success::ShortcutSubmitted);
    }
    // If Windows accepted a prefix, release only the keys that prefix put down.
    let down = accepted.min(keys.len());
    let up = accepted.saturating_sub(keys.len());
    if let Some(remaining) = keys.get(..down.saturating_sub(up))
        && !remaining.is_empty()
    {
        let releases = remaining
            .iter()
            .rev()
            .map(|&key| packet(key, true))
            .collect::<Vec<_>>();
        // SAFETY: this batch contains key-up events only.
        unsafe {
            SendInput(
                releases.len() as u32,
                releases.as_ptr(),
                size_of::<INPUT>() as i32,
            )
        };
    }
    Err(Failure::Keyboard)
}

fn packet(key: u16, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                dwFlags: if up { KEYEVENTF_KEYUP } else { 0 },
                ..Default::default()
            },
        },
    }
}
