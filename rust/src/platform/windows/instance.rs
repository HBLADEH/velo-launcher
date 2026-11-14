//! Locks are acquired before Slint or any future data store is opened. The
//! Preview/test identities cannot target the product data directory. A product
//! build also requires the machine installer's receipt before using that path.

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
    pub migration_id: Option<String>,
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
            migration_id: None,
        })
    }

    pub fn resolve_launch(
        id: Option<String>,
        data_dir: Option<PathBuf>,
        normal_lifecycle: bool,
    ) -> Result<Self, String> {
        if !cfg!(feature = "product") || !normal_lifecycle || id.is_some() || data_dir.is_some() {
            return Self::resolve(id, data_dir);
        }
        let executable = std::env::current_exe().map_err(|_| "无法确认正式安装位置")?;
        let directory = executable.parent().ok_or("无法确认正式安装位置")?;
        if directory.join(".velo-rollback/pending.json").exists() {
            return Err("正式安装尚未完成恢复；请退出并重新运行系统安装器".into());
        }
        let receipt: serde_json::Value = serde_json::from_slice(
            &std::fs::read(directory.join(".velo-rust/installation.json"))
                .map_err(|_| "正式构建须通过系统安装器安装；测试须指定隔离参数")?,
        )
        .map_err(|_| "正式安装回执损坏；请修复安装")?;
        let migration_id = receipt["MigrationID"].as_str().ok_or("正式安装回执缺失")?;
        if receipt["Schema"] != 1
            || migration_id.len() != 32
            || !migration_id.bytes().all(|c| c.is_ascii_hexdigit())
        {
            return Err("正式安装回执无效；请修复安装".into());
        }
        let registered = product_location()?;
        if path_key(&normalize(Path::new(&registered))?) != path_key(&normalize(directory)?) {
            return Err("正式安装位置与回执不一致".into());
        }
        let base = std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA is not set")?;
        Ok(Self {
            id: "product".into(),
            data_dir: normalize(&Path::new(&base).join("Velo"))?,
            migration_id: Some(migration_id.into()),
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
        let legacy = if self.migration_id.is_some() {
            // Wails v2's mutex name comes from its checked-in UniqueId. Do not
            // open the same store while an old installed/portable Go is active.
            let (lock, exists) = Lock::open("wails-app-6ab2e7d8-15d5-456b-a8eb-f924ac650291sim")?;
            if exists {
                return Err("旧版 Velo 正在运行，请先从托盘退出后重试".into());
            }
            Some(lock)
        } else {
            None
        };
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
            _legacy: legacy,
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

fn product_location() -> Result<String, String> {
    use windows_sys::Win32::System::Registry::{
        HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RRF_SUBKEY_WOW6464KEY, RegGetValueW,
    };
    let mut buffer = vec![0u16; 32768];
    let mut size = (buffer.len() * 2) as u32;
    // SAFETY: fixed-width caller-owned buffer and terminated immutable names;
    // RegGetValueW retains no pointer. Always read the machine x64 install view.
    let result = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            wide(r"Software\Microsoft\Windows\CurrentVersion\Uninstall\HBLADEHVelo").as_ptr(),
            wide("InstallLocation").as_ptr(),
            RRF_RT_REG_SZ | RRF_SUBKEY_WOW6464KEY,
            std::ptr::null_mut(),
            buffer.as_mut_ptr().cast(),
            &mut size,
        )
    };
    if result != 0 {
        return Err("正式系统安装尚未注册".into());
    }
    let end = buffer
        .iter()
        .position(|&c| c == 0)
        .ok_or("正式安装路径无效")?;
    Ok(String::from_utf16_lossy(&buffer[..end]))
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
    _legacy: Option<Lock>,
}
pub enum Acquisition {
    Primary(Guard),
    Forwarded,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_build_still_isolates_smokes_and_explicit_test_copies() {
        let smoke = Identity::resolve_launch(None, None, false).unwrap();
        assert_eq!(smoke.id, "prototype");
        assert!(smoke.migration_id.is_none());
        assert!(Identity::resolve_launch(Some("test".into()), None, true).is_err());
        if cfg!(feature = "product") {
            // Cargo's test executable has no machine-installer receipt.
            assert!(Identity::resolve_launch(None, None, true).is_err());
        }
    }

    #[test]
    fn product_refuses_an_existing_go_instance_before_accessing_data() {
        let (_legacy, _) = Lock::open("wails-app-6ab2e7d8-15d5-456b-a8eb-f924ac650291sim").unwrap();
        let root = tempfile::tempdir().unwrap();
        let identity = Identity {
            id: format!("product-lock-test-{}", std::process::id()),
            data_dir: root.path().join("never-created"),
            migration_id: Some("abcdef0123456789abcdef0123456789".into()),
        };
        assert!(identity.acquire().is_err());
        assert!(!identity.data_dir.exists());
    }

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
