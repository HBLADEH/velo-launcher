use super::preferences::Root;
use super::{instance::Identity, preferences};
use crate::core::{DataError, Result, update};
use std::io::Write;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn writable(directory: &Path) -> bool {
    tempfile::Builder::new()
        .prefix(".velo-writable-")
        .tempfile_in(directory)
        .is_ok()
}
pub fn target(portable: bool, identity: &Identity) -> Result<PathBuf> {
    if !portable && (!cfg!(feature = "product") || identity.migration_id.is_some()) {
        for root in [Root::Machine, Root::User] {
            if let Some(uninstaller) = preferences::string(
                root,
                if cfg!(feature = "product") {
                    r"Software\Microsoft\Windows\CurrentVersion\Uninstall\HBLADEHVelo"
                } else {
                    r"Software\Microsoft\Windows\CurrentVersion\Uninstall\HBLADEHVeloSlint"
                },
                "UninstallString",
            ) && let Some(directory) = Path::new(uninstaller.trim().trim_matches('"')).parent()
            {
                let executable = directory.join(if cfg!(feature = "product") {
                    "velo-launcher.exe"
                } else {
                    "velo-slint.exe"
                });
                if executable.is_file() {
                    return Ok(executable);
                }
            }
        }
    }
    std::env::current_exe().map_err(|e| DataError::new("find update target", e))
}
pub fn prepare(
    directory: &Path,
    target: &Path,
    source: &Path,
    portable: bool,
    hash: &str,
    identity: &Identity,
) -> Result<PathBuf> {
    let arguments = if identity.migration_id.is_some() {
        Vec::new()
    } else {
        vec![
            format!("--instance-id={}", identity.id),
            format!("--data-dir=\"{}\"", identity.data_dir.display()),
        ]
    };
    let script = update::script(
        target,
        source,
        portable,
        hash,
        std::process::id(),
        &arguments,
    );
    let mut file = tempfile::Builder::new()
        .prefix("velo-update-")
        .suffix(".ps1")
        .tempfile_in(directory)
        .map_err(|e| DataError::new("create update script", e))?;
    file.write_all(b"\xef\xbb\xbf")
        .and_then(|_| file.write_all(script.as_bytes()))
        .and_then(|_| file.as_file().sync_all())
        .map_err(|e| DataError::new("write update script", e))?;
    file.into_temp_path()
        .keep()
        .map_err(|e| DataError::new("keep update script", e))
}
pub fn schedule(path: &Path) -> Result<()> {
    let root = preferences::windows_directory()?;
    Command::new(root.join("System32/WindowsPowerShell/v1.0/powershell.exe"))
        .env_remove("PSModulePath")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-WindowStyle",
            "Hidden",
            "-File",
        ])
        // PowerShell 5.1 rejects a verbatim \\?\ script path in its
        // AuthorizationManager even when execution policy permits the file.
        .arg(powershell_path(path))
        .creation_flags(0x08000000 | 0x00000200)
        .spawn()
        .map_err(|e| DataError::new("start update script", e))?;
    Ok(())
}

fn powershell_path(path: &Path) -> PathBuf {
    let units: Vec<_> = path.as_os_str().encode_wide().collect();
    let disk_prefix: Vec<_> = r"\\?\".encode_utf16().collect();
    let unc_prefix: Vec<_> = r"\\?\UNC\".encode_utf16().collect();
    let normalized = if units.starts_with(&unc_prefix) {
        r"\\"
            .encode_utf16()
            .chain(units[unc_prefix.len()..].iter().copied())
            .collect()
    } else if units.starts_with(&disk_prefix)
        && units.get(5) == Some(&(b':' as u16))
        && units.get(6) == Some(&(b'\\' as u16))
    {
        units[disk_prefix.len()..].to_vec()
    } else {
        units
    };
    std::ffi::OsString::from_wide(&normalized).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn powershell_entrypoint_uses_shell_compatible_disk_and_unc_paths() {
        assert_eq!(
            powershell_path(Path::new(r"\\?\D:\中文 空格\update.ps1")),
            Path::new(r"D:\中文 空格\update.ps1")
        );
        assert_eq!(
            powershell_path(Path::new(r"\\?\UNC\server\share\update.ps1")),
            Path::new(r"\\server\share\update.ps1")
        );
        for path in [
            r"D:\中文 空格\update.ps1",
            r"relative.ps1",
            r"\\?\Volume{example}\update.ps1",
        ] {
            assert_eq!(powershell_path(Path::new(path)), Path::new(path));
        }
    }
}
