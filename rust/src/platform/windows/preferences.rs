//! Registry ownership and exact rollback stay outside the UI and data rules.
use super::instance::{Identity, wide};
use crate::core::{DataError, Result};
use std::path::PathBuf;
use std::ptr::{null, null_mut};
use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
use windows_sys::Win32::System::Registry::*;
use windows_sys::Win32::System::SystemInformation::GetWindowsDirectoryW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    SPI_GETCLIENTAREAANIMATION, SystemParametersInfoW,
};

pub enum Root {
    User,
    Machine,
    Classes,
}
pub fn string(root: Root, key: &str, name: &str) -> Option<String> {
    let root = match root {
        Root::User => HKEY_CURRENT_USER,
        Root::Machine => HKEY_LOCAL_MACHINE,
        Root::Classes => HKEY_CLASSES_ROOT,
    };
    let mut bytes = 0;
    let key = wide(key);
    let name = wide(name);
    // SAFETY: immutable terminated names; two-pass owned buffer, no retained pointers.
    unsafe {
        if RegGetValueW(
            root,
            key.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_SZ,
            null_mut(),
            null_mut(),
            &mut bytes,
        ) != ERROR_SUCCESS
        {
            return None;
        }
        let mut buffer = vec![0u16; bytes as usize / 2 + 1];
        if RegGetValueW(
            root,
            key.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_SZ,
            null_mut(),
            buffer.as_mut_ptr().cast(),
            &mut bytes,
        ) != ERROR_SUCCESS
        {
            return None;
        }
        let end = buffer.iter().position(|&u| u == 0).unwrap_or(buffer.len());
        Some(String::from_utf16_lossy(&buffer[..end]))
    }
}

pub fn windows_directory() -> Result<PathBuf> {
    let mut buffer = vec![0u16; 32768];
    // SAFETY: writable buffer and capacity in UTF-16 units.
    let length = unsafe { GetWindowsDirectoryW(buffer.as_mut_ptr(), buffer.len() as u32) } as usize;
    if length == 0 || length >= buffer.len() {
        return Err(DataError::new(
            "find Windows directory",
            std::io::Error::last_os_error(),
        ));
    }
    Ok(String::from_utf16_lossy(&buffer[..length]).into())
}

pub fn dark_theme() -> bool {
    let mut value = 1u32;
    let mut size = 4;
    // SAFETY: DWORD destination matches the requested registry type.
    unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            wide(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize").as_ptr(),
            wide("AppsUseLightTheme").as_ptr(),
            RRF_RT_REG_DWORD,
            null_mut(),
            (&mut value as *mut u32).cast(),
            &mut size,
        );
    }
    value == 0
}

pub fn reduced_motion() -> bool {
    let mut enabled = 1i32;
    // SAFETY: SPI_GETCLIENTAREAANIMATION writes a BOOL and retains no pointer.
    unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            (&mut enabled as *mut i32).cast(),
            0,
        );
    }
    enabled == 0
}

struct Key(HKEY);
impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            RegCloseKey(self.0);
        }
    }
}

#[derive(Clone)]
pub struct Startup {
    pub name: String,
    pub command: String,
}

impl Startup {
    pub fn for_instance(identity: &Identity) -> Result<Self> {
        let executable =
            std::env::current_exe().map_err(|e| DataError::new("find startup executable", e))?;
        if identity.migration_id.is_some() {
            return Ok(Self {
                name: "Velo".into(),
                command: format!("\"{}\" --background", executable.display()),
            });
        }
        // Distinct value and explicit arguments prevent a prototype from taking
        // over the production login entry or opening a different data directory.
        Ok(Self {
            name: format!("VeloSlint-{}", identity.id),
            command: format!(
                "\"{}\" --background --instance-id={} --data-dir=\"{}\"",
                executable.display(),
                identity.id,
                identity.data_dir.display()
            ),
        })
    }

    pub fn change(&self, enabled: bool) -> Result<StartupChange> {
        self.change_at(r"Software\Microsoft\Windows\CurrentVersion\Run", enabled)
    }

    fn change_at(&self, path: &str, enabled: bool) -> Result<StartupChange> {
        // Run accepts at most 260 UTF-16 code units. A longer REG_SZ can be
        // written successfully but Windows may skip it at login. Validate before
        // touching the registry; disabling must still remove an old long value.
        if enabled && self.command.encode_utf16().count() > 260 {
            return Err(DataError::new(
                "validate login startup command",
                "Windows Run command exceeds 260 UTF-16 code units",
            ));
        }
        let mut key = null_mut();
        // SAFETY: caller-owned output; Key closes the resulting HKEY on every path.
        let code = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                wide(path).as_ptr(),
                0,
                null(),
                0,
                KEY_QUERY_VALUE | KEY_SET_VALUE,
                null(),
                &mut key,
                null_mut(),
            )
        };
        if code != ERROR_SUCCESS {
            return Err(registry_error(code));
        }
        let key = Key(key);
        let name = wide(&self.name);
        let mut kind = 0;
        let mut size = 0;
        let code = unsafe {
            RegQueryValueExW(
                key.0,
                name.as_ptr(),
                null(),
                &mut kind,
                null_mut(),
                &mut size,
            )
        };
        let old = if code == ERROR_FILE_NOT_FOUND {
            None
        } else {
            if code != ERROR_SUCCESS {
                return Err(registry_error(code));
            }
            let mut bytes = vec![0; size as usize];
            let code = unsafe {
                RegQueryValueExW(
                    key.0,
                    name.as_ptr(),
                    null(),
                    &mut kind,
                    bytes.as_mut_ptr(),
                    &mut size,
                )
            };
            if code != ERROR_SUCCESS {
                return Err(registry_error(code));
            }
            bytes.truncate(size as usize);
            Some((kind, bytes))
        };
        let mut change = StartupChange {
            key,
            name,
            old,
            committed: false,
        };
        let code = if enabled {
            let command = wide(&self.command);
            unsafe {
                RegSetValueExW(
                    change.key.0,
                    change.name.as_ptr(),
                    0,
                    REG_SZ,
                    command.as_ptr().cast(),
                    (command.len() * 2) as u32,
                )
            }
        } else {
            unsafe { RegDeleteValueW(change.key.0, change.name.as_ptr()) }
        };
        if code != ERROR_SUCCESS && !(code == ERROR_FILE_NOT_FOUND && !enabled) {
            change.committed = true;
            return Err(registry_error(code));
        }
        Ok(change)
    }
}

pub struct StartupChange {
    key: Key,
    name: Vec<u16>,
    old: Option<(u32, Vec<u8>)>,
    committed: bool,
}
impl StartupChange {
    pub fn commit(mut self) {
        self.committed = true;
    }
    pub fn rollback(&mut self) -> Result<()> {
        if self.committed {
            return Ok(());
        }
        let code = match &self.old {
            Some((kind, bytes)) => unsafe {
                RegSetValueExW(
                    self.key.0,
                    self.name.as_ptr(),
                    0,
                    *kind,
                    bytes.as_ptr(),
                    bytes.len() as u32,
                )
            },
            None => unsafe { RegDeleteValueW(self.key.0, self.name.as_ptr()) },
        };
        if code != ERROR_SUCCESS && code != ERROR_FILE_NOT_FOUND {
            return Err(registry_error(code));
        }
        self.committed = true;
        Ok(())
    }
}
impl Drop for StartupChange {
    fn drop(&mut self) {
        if self.rollback().is_err() {
            eprintln!("event=startup_rollback_failed");
        }
    }
}
fn registry_error(code: u32) -> DataError {
    DataError::new(
        "update login startup",
        std::io::Error::from_raw_os_error(code as i32),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn login_command_limit_counts_utf16_and_rejection_preserves_the_old_value() {
        let path = format!(r"Software\VeloSlintTests\run-limit-{}", std::process::id());
        let mut startup = Startup {
            name: "RunLimit".into(),
            command: "中".repeat(260),
        };
        startup.change_at(&path, true).unwrap().commit();
        // Supplementary characters use two UTF-16 units each, even though Rust
        // counts each as one char. The boundary is unrelated to UTF-8 bytes.
        startup.command = "😀".repeat(130);
        startup.change_at(&path, true).unwrap().commit();
        let saved = string(Root::User, &path, &startup.name).unwrap();
        for too_long in ["x".repeat(261), format!("{saved}x")] {
            startup.command = too_long;
            let error = startup.change_at(&path, true).err().unwrap();
            assert_eq!(error.operation, "validate login startup command");
            assert_eq!(string(Root::User, &path, &startup.name).unwrap(), saved);
        }
        startup.change_at(&path, false).unwrap().commit();
        assert!(string(Root::User, &path, &startup.name).is_none());
        // SAFETY: this test owns the now-empty synthetic registry key.
        unsafe { RegDeleteKeyW(HKEY_CURRENT_USER, wide(&path).as_ptr()) };
    }

    #[test]
    fn login_value_rollback_restores_exact_type_and_bytes_and_removal_is_idempotent() {
        let name = format!("VeloSlint-rs11-{}", std::process::id());
        let startup = Startup {
            name: name.clone(),
            command: r#""D:\中文 O'Brien\Velo.exe" --background"#.into(),
        };
        let path = format!(r"Software\VeloSlintTests\{}", name);
        let first = startup.change_at(&path, true).unwrap();
        first.commit();
        assert_eq!(string(Root::User, &path, &name).unwrap(), startup.command);
        let mut changed = Startup {
            name: name.clone(),
            command: "replacement".into(),
        }
        .change_at(&path, true)
        .unwrap();
        assert_eq!(string(Root::User, &path, &name).unwrap(), "replacement");
        changed.rollback().unwrap();
        drop(changed);
        assert_eq!(string(Root::User, &path, &name).unwrap(), startup.command);
        startup.change_at(&path, false).unwrap().commit();
        startup.change_at(&path, false).unwrap().commit();
        assert!(string(Root::User, &path, &name).is_none());
        unsafe {
            RegDeleteKeyW(HKEY_CURRENT_USER, wide(&path).as_ptr());
        }
    }
}
