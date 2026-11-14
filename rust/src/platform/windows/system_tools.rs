use super::preferences::Root;
use super::{instance::wide, launch::Failure, preferences};
use crate::core::{
    Result,
    model::AppItem,
    system_tools::{self, Environment},
};
use std::os::windows::process::CommandExt;
use std::process::Command;
use std::ptr::null;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::System::Shutdown::LockWorkStation;
use windows_sys::Win32::UI::Shell::{
    SHERB_NOCONFIRMATION, SHEmptyRecycleBinW, SHQUERYRBINFO, SHQueryRecycleBinW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

pub fn discover() -> Result<Vec<AppItem>> {
    let windows = preferences::windows_directory()?;
    let policies: Vec<_> = [Root::Machine, Root::User]
        .into_iter()
        .filter_map(|root| {
            preferences::string(
                root,
                r"Software\Microsoft\Windows\CurrentVersion\Policies\Explorer",
                "SettingsPageVisibility",
            )
        })
        .collect();
    let env = Environment {
        windows: &windows,
        build: preferences::string(
            Root::Machine,
            r"SOFTWARE\Microsoft\Windows NT\CurrentVersion",
            "CurrentBuildNumber",
        )
        .and_then(|s| s.parse().ok())
        .unwrap_or(0),
        settings: preferences::string(Root::Classes, "ms-settings", "URL Protocol").is_some(),
        policies: &policies,
    };
    let mut items = system_tools::discover(&env, |p| p.is_file())?;
    for item in &mut items {
        item.icon_url = crate::core::icons::signature(item);
    }
    Ok(items)
}

pub fn action(owner: isize, item: &AppItem) -> std::result::Result<(), Failure> {
    if !system_tools::special(item) {
        return Err(Failure::Unavailable);
    }
    let owner = owner as HWND;
    // SAFETY: the owner stays live until completion, strings are owned for each
    // blocking call. Every destructive command comes from the exact allow list.
    unsafe {
        match item.id.as_str() {
            "system:lock" => {
                if LockWorkStation() == 0 {
                    Err(Failure::Shell)
                } else {
                    Ok(())
                }
            }
            "system:shutdown" | "system:restart" => {
                let answer = MessageBoxW(
                    owner,
                    wide(&format!("确定要{}吗？请先保存正在编辑的文件。", item.name)).as_ptr(),
                    wide(&format!("Velo · {}", item.name)).as_ptr(),
                    MB_OKCANCEL | MB_DEFBUTTON2 | MB_ICONWARNING | MB_SETFOREGROUND,
                );
                if answer != IDOK {
                    return Err(Failure::Cancelled);
                }
                let root = preferences::windows_directory().map_err(|_| Failure::Unavailable)?;
                let status = Command::new(root.join("System32/shutdown.exe"))
                    .args([
                        if item.id == "system:restart" {
                            "/r"
                        } else {
                            "/s"
                        },
                        "/t",
                        "0",
                    ])
                    .creation_flags(0x08000000)
                    .status()
                    .map_err(|_| Failure::Shell)?;
                if status.success() {
                    Ok(())
                } else {
                    Err(Failure::Shell)
                }
            }
            "system:empty-recycle-bin" => {
                let query = || {
                    let mut info = SHQUERYRBINFO {
                        cbSize: std::mem::size_of::<SHQUERYRBINFO>() as u32,
                        ..Default::default()
                    };
                    (SHQueryRecycleBinW(null(), &mut info) >= 0).then_some(info.i64NumItems)
                };
                empty_recycle_bin(
                    query,
                    |count| {
                        let message = match count.filter(|count| *count > 0) {
                            Some(count) => format!(
                                "确定要永久删除回收站中的 {count} 个项目吗？此操作无法撤销。"
                            ),
                            None => "确定要永久删除回收站中的所有项目吗？此操作无法撤销。".into(),
                        };
                        match MessageBoxW(
                            owner,
                            wide(&message).as_ptr(),
                            wide("Velo · 清空回收站").as_ptr(),
                            MB_OKCANCEL | MB_DEFBUTTON2 | MB_ICONWARNING | MB_SETFOREGROUND,
                        ) {
                            IDOK => Ok(true),
                            IDCANCEL => Ok(false),
                            _ => Err(Failure::Shell),
                        }
                    },
                    // SHEmptyRecycleBin can return S_OK when its own dialog is
                    // cancelled. Own the decision above; retain Shell progress
                    // and sound, but avoid a second, unobservable confirmation.
                    || SHEmptyRecycleBinW(owner, null(), SHERB_NOCONFIRMATION),
                    || {
                        if MessageBoxW(
                            owner,
                            wide("回收站已为空，无需清理。").as_ptr(),
                            wide("Velo · 清空回收站").as_ptr(),
                            MB_OK | MB_ICONINFORMATION | MB_SETFOREGROUND,
                        ) == 0
                        {
                            Err(Failure::Shell)
                        } else {
                            Ok(())
                        }
                    },
                )
            }
            _ => Err(Failure::Unavailable),
        }
    }
}

fn empty_recycle_bin(
    query: impl Fn() -> Option<i64>,
    confirm: impl FnOnce(Option<i64>) -> std::result::Result<bool, Failure>,
    empty: impl FnOnce() -> i32,
    notify_empty: impl FnOnce() -> std::result::Result<(), Failure>,
) -> std::result::Result<(), Failure> {
    let count = query();
    if count == Some(0) {
        return notify_empty();
    }
    if !confirm(count)? {
        return Err(Failure::Cancelled);
    }
    let result = empty();
    match result as u32 {
        0x800704c7 | 0x80004004 => Err(Failure::Cancelled),
        0x8000ffff if query() == Some(0) => Ok(()),
        _ if result >= 0 => Ok(()),
        _ => Err(Failure::Shell),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn cancelled_confirmation_never_calls_shell_or_reports_success() {
        for count in [Some(7), None] {
            assert_eq!(
                empty_recycle_bin(
                    || count,
                    |actual| {
                        assert_eq!(actual, count);
                        Ok(false)
                    },
                    || panic!("cancelled action reached Shell"),
                    || panic!("not empty")
                ),
                Err(Failure::Cancelled)
            );
        }
        assert_eq!(
            empty_recycle_bin(
                || Some(7),
                |_| Err(Failure::Shell),
                || panic!("failed confirmation reached Shell"),
                || panic!("not empty")
            ),
            Err(Failure::Shell)
        );
    }

    #[test]
    fn empty_bin_only_notifies_and_confirmed_action_preserves_shell_failures() {
        assert_eq!(
            empty_recycle_bin(
                || Some(0),
                |_| panic!("empty bin confirmation"),
                || panic!("empty bin operation"),
                || Ok(())
            ),
            Ok(())
        );
        for (result, expected) in [
            (0, Ok(())),
            (0x800704c7u32 as i32, Err(Failure::Cancelled)),
            (0x80004004u32 as i32, Err(Failure::Cancelled)),
            (0x80004005u32 as i32, Err(Failure::Shell)),
        ] {
            assert_eq!(
                empty_recycle_bin(|| Some(7), |_| Ok(true), || result, || panic!("not empty")),
                expected
            );
        }
    }

    #[test]
    fn shell_unexpected_error_is_success_only_when_recheck_proves_empty() {
        let queries = Cell::new(0);
        assert_eq!(
            empty_recycle_bin(
                || {
                    let n = queries.get();
                    queries.set(n + 1);
                    Some(if n == 0 { 7 } else { 0 })
                },
                |_| Ok(true),
                || 0x8000ffffu32 as i32,
                || panic!("initially not empty")
            ),
            Ok(())
        );
        assert_eq!(queries.get(), 2);
        assert_eq!(
            empty_recycle_bin(
                || None,
                |_| Ok(true),
                || 0x8000ffffu32 as i32,
                || panic!("unknown bin")
            ),
            Err(Failure::Shell)
        );
    }
}
