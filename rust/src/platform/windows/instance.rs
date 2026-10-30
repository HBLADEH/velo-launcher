//! Locks are acquired before Slint or any future data store is opened. The
//! prototype deliberately cannot target the Go application's data directory.

use std::path::{Component, Path, PathBuf};
use std::ptr::null;
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, FindWindowW, GetWindowThreadProcessId, RegisterWindowMessageW,
    SMTO_ABORTIFHUNG, SendMessageTimeoutW,
};

pub const WINDOW_CLASS: &str = "VeloSlintDesktopService";

pub fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}

pub fn activation_message() -> u32 {
    unsafe { RegisterWindowMessageW(wide("Velo.Slint.Activate.1").as_ptr()) }
}

#[derive(Clone)]
pub struct Identity {
    pub id: String,
    pub data_dir: PathBuf,
}

impl Identity {
    pub fn resolve(id: Option<String>, data_dir: Option<PathBuf>) -> Result<Self, String> {
        if id.is_some() != data_dir.is_some() {
            return Err("测试副本须同时指定 --instance-id 和 --data-dir".into());
        }
        let base = std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA is not set")?;
        let id = id.unwrap_or_else(|| "prototype".into());
        if id.is_empty()
            || id.len() > 48
            || !id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
        {
            return Err("instance-id 须为 1–48 个英文字母、数字、- 或 _".into());
        }
        let data_dir =
            normalize(&data_dir.unwrap_or_else(|| Path::new(&base).join("VeloSlintPrototype")))?;
        let product = normalize(&Path::new(&base).join("Velo"))?;
        let key = path_key(&data_dir);
        let product_key = path_key(&product);
        if key == product_key || key.starts_with(&format!("{product_key}\\")) {
            return Err("Rust 原型禁止使用 Go/Velo 产品数据目录；请指定独立测试目录".into());
        }
        if data_dir.is_file() {
            return Err("data-dir must be a directory".into());
        }
        Ok(Self {
            id: id.to_ascii_lowercase(),
            data_dir,
        })
    }

    pub fn window_title(&self) -> String {
        format!("Velo Slint instance {}", self.id)
    }

    pub fn acquire(&self) -> Result<Acquisition, String> {
        let (instance, exists) = Lock::open(&format!("Local\\VeloSlint.Instance.{}", self.id))?;
        if exists {
            // A first launch may still be between CreateMutex and CreateWindow.
            // Never launch a competing UI if the endpoint is late or hung.
            self.wake(Duration::from_secs(4))?;
            return Ok(Acquisition::Forwarded);
        }
        let (data, exists) = Lock::open(&format!(
            "Local\\VeloSlint.Data.{:016x}",
            fingerprint(&path_key(&self.data_dir))
        ))?;
        if exists {
            return Err(
                "该测试数据目录已由另一个实例使用，请同时更换 instance-id 和 data-dir".into(),
            );
        }
        Ok(Acquisition::Primary(Guard {
            _instance: instance,
            _data: data,
        }))
    }

    fn wake(&self, timeout: Duration) -> Result<(), String> {
        let class = wide(WINDOW_CLASS);
        let title = wide(&self.window_title());
        let message = activation_message();
        if message == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let started = Instant::now();
        while started.elapsed() < timeout {
            let hwnd = unsafe { FindWindowW(class.as_ptr(), title.as_ptr()) };
            if !hwnd.is_null() {
                let mut pid = 0;
                unsafe {
                    GetWindowThreadProcessId(hwnd, &mut pid);
                    AllowSetForegroundWindow(pid);
                    let mut result = 0;
                    if SendMessageTimeoutW(hwnd, message, 0, 0, SMTO_ABORTIFHUNG, 1000, &mut result)
                        != 0
                        && result == 1
                    {
                        eprintln!("event=instance_forwarded target_pid={pid}");
                        return Ok(());
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(40));
        }
        Err("已有实例尚未就绪或没有响应；未启动第二个实例".into())
    }
}

// Resolve existing ancestors too, so junction/symlink spellings share a lock.
fn normalize(path: &Path) -> Result<PathBuf, String> {
    let absolute = std::path::absolute(path).map_err(|e| e.to_string())?;
    let mut clean = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::ParentDir => {
                clean.pop();
            }
            Component::CurDir => {}
            other => clean.push(other),
        }
    }
    let mut ancestor = clean.as_path();
    let mut suffix = Vec::new();
    while !ancestor.exists() {
        suffix.push(
            ancestor
                .file_name()
                .ok_or("Invalid data directory")?
                .to_owned(),
        );
        ancestor = ancestor.parent().ok_or("Invalid data directory")?;
    }
    let mut resolved = ancestor.canonicalize().map_err(|e| e.to_string())?;
    for component in suffix.into_iter().rev() {
        resolved.push(component);
    }
    Ok(resolved)
}

fn path_key(path: &Path) -> String {
    path.to_string_lossy()
        .trim_start_matches("\\\\?\\")
        .trim_end_matches('\\')
        .to_lowercase()
}

fn fingerprint(text: &str) -> u64 {
    text.bytes().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    })
}

struct Lock(HANDLE);
impl Lock {
    fn open(name: &str) -> Result<(Self, bool), String> {
        let handle = unsafe { CreateMutexW(null(), 0, wide(name).as_ptr()) };
        if handle.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let exists = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
        Ok((Self(handle), exists))
    }
}
impl Drop for Lock {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

pub struct Guard {
    _instance: Lock,
    _data: Lock,
}
pub enum Acquisition {
    Primary(Guard),
    Forwarded,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prototype_and_test_copies_cannot_open_product_or_share_a_data_store() {
        assert!(Identity::resolve(Some("a".into()), None).is_err());
        let base = PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap());
        for path in [
            base.join("Velo"),
            base.join("Velo/cache"),
            base.join("x/../Velo"),
        ] {
            assert!(Identity::resolve(Some("test".into()), Some(path)).is_err());
        }
        let root = base.join(format!("VeloSlintTests/{}", std::process::id()));
        let first = Identity::resolve(
            Some(format!("test-{}", std::process::id())),
            Some(root.clone()),
        )
        .unwrap();
        let Acquisition::Primary(guard) = first.acquire().unwrap() else {
            panic!("not primary")
        };
        let other = Identity::resolve(
            Some(format!("test-other-{}", std::process::id())),
            Some(root),
        )
        .unwrap();
        assert!(other.acquire().is_err());
        let separate = Identity::resolve(
            Some(format!("test-separate-{}", std::process::id())),
            Some(base.join(format!("VeloSlintTests/{}-other", std::process::id()))),
        )
        .unwrap();
        assert!(matches!(separate.acquire(), Ok(Acquisition::Primary(_))));
        drop(guard);
        assert!(matches!(first.acquire(), Ok(Acquisition::Primary(_))));
    }
}
