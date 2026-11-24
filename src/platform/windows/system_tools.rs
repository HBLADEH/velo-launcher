use super::preferences::Root;
use super::{launch::Failure, preferences};
use crate::core::{
    Result,
    model::AppItem,
    system_tools::{self, Environment, Prompt},
};
use std::os::windows::process::CommandExt;
use std::process::Command;
use std::ptr::null;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::System::Shutdown::LockWorkStation;
use windows_sys::Win32::UI::Shell::{
    SHERB_NOCONFIRMATION, SHEmptyRecycleBinW, SHQUERYRBINFO, SHQueryRecycleBinW,
};

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
        // Match the Control Panel icon. The MMC executable and console file
        // show different, generic management icons.
        if let Some((file, index)) = match item.id.as_str() {
            "system:devices" => Some(("devmgr.dll", -201)),
            "system:apps" => Some(("appwiz.cpl", 0)),
            "system:environment" => Some(("sysdm.cpl", 0)),
            _ => None,
        } {
            item.icon_path = format!("{},{index}", windows.join("System32").join(file).display());
        }
        item.icon_url = crate::core::icons::signature(item);
    }
    Ok(items)
}

pub fn action(
    owner: isize,
    item: &AppItem,
    prompt: impl FnOnce(Prompt) -> std::result::Result<bool, Failure>,
) -> std::result::Result<(), Failure> {
    if !system_tools::special(item) {
        return Err(Failure::Unavailable);
    }
    match item.id.as_str() {
        "system:lock" => {
            // SAFETY: this is the allow-listed lock action and takes no pointers.
            if unsafe { LockWorkStation() } == 0 {
                Err(Failure::Shell)
            } else {
                Ok(())
            }
        }
        "system:shutdown" | "system:restart" => {
            let restart = item.id == "system:restart";
            if !prompt(if restart {
                Prompt::Restart
            } else {
                Prompt::Shutdown
            })? {
                return Err(Failure::Cancelled);
            }
            let root = preferences::windows_directory().map_err(|_| Failure::Unavailable)?;
            let status = Command::new(root.join("System32/shutdown.exe"))
                .args([if restart { "/r" } else { "/s" }, "/t", "0"])
                .creation_flags(0x08000000)
                .status()
                .map_err(|_| Failure::Shell)?;
            if status.success() {
                Ok(())
            } else {
                Err(Failure::Shell)
            }
        }
        "system:empty-recycle-bin" => empty_recycle_bin(
            || {
                let mut info = SHQUERYRBINFO {
                    cbSize: std::mem::size_of::<SHQUERYRBINFO>() as u32,
                    ..Default::default()
                };
                // SAFETY: the initialized output lives for the query; null selects all drives.
                (unsafe { SHQueryRecycleBinW(null(), &mut info) } >= 0).then_some(info.i64NumItems)
            },
            prompt,
            // Own the confirmation in Velo and retain Shell progress and sound.
            // SAFETY: the caller retains the owner until this blocking call completes.
            || unsafe { SHEmptyRecycleBinW(owner as HWND, null(), SHERB_NOCONFIRMATION) },
        ),
        _ => Err(Failure::Unavailable),
    }
}

fn empty_recycle_bin(
    query: impl Fn() -> Option<i64>,
    prompt: impl FnOnce(Prompt) -> std::result::Result<bool, Failure>,
    empty: impl FnOnce() -> i32,
) -> std::result::Result<(), Failure> {
    let count = query();
    if count == Some(0) {
        return prompt(Prompt::RecycleBinAlreadyEmpty).map(|_| ());
    }
    if !prompt(Prompt::EmptyRecycleBin(count))? {
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
                    |prompt| {
                        assert!(
                            matches!(prompt, Prompt::EmptyRecycleBin(actual) if actual == count)
                        );
                        Ok(false)
                    },
                    || panic!("cancelled action reached Shell")
                ),
                Err(Failure::Cancelled)
            );
        }
        assert_eq!(
            empty_recycle_bin(
                || Some(7),
                |_| Err(Failure::Shell),
                || panic!("failed confirmation reached Shell")
            ),
            Err(Failure::Shell)
        );
    }

    #[test]
    fn empty_bin_only_notifies_and_confirmed_action_preserves_shell_failures() {
        assert_eq!(
            empty_recycle_bin(
                || Some(0),
                |prompt| {
                    assert!(matches!(prompt, Prompt::RecycleBinAlreadyEmpty));
                    Ok(false)
                },
                || panic!("empty bin operation")
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
                empty_recycle_bin(|| Some(7), |_| Ok(true), || result),
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
                || 0x8000ffffu32 as i32
            ),
            Ok(())
        );
        assert_eq!(queries.get(), 2);
        assert_eq!(
            empty_recycle_bin(|| None, |_| Ok(true), || 0x8000ffffu32 as i32),
            Err(Failure::Shell)
        );
    }
}
