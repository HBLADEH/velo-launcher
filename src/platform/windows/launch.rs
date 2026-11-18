use std::io::{self, ErrorKind};
use std::path::Path;
use std::ptr::{null, null_mut};
use windows_sys::Win32::Foundation::{
    ERROR_CANCELLED, ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND, HWND,
};
use windows_sys::Win32::System::Com::CoTaskMemFree;
use windows_sys::Win32::UI::Shell::{
    SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SHELLEXECUTEINFOW, SHParseDisplayName, ShellExecuteExW,
    ShellExecuteW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use super::shell::{Apartment, ShortcutResolver, wide};
use crate::core::indexer::{Resolver, extension};
use crate::core::launch::{self, Action, CommandError};
use crate::core::model::AppItem;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    MissingTarget,
    Unavailable,
    Cancelled,
    Shell,
}

impl Failure {
    pub fn message(self) -> &'static str {
        match self {
            Self::MissingTarget => "找不到应用，可能已卸载或移动",
            Self::Unavailable => "应用已移动、删除或不支持此操作，请刷新后重试",
            Self::Cancelled => "已取消启动",
            Self::Shell => "Windows 无法打开此应用，请检查权限或文件关联后重试",
        }
    }
}

pub fn execute(
    apartment: &Apartment,
    owner: isize,
    item: &AppItem,
    action: Action,
) -> Result<(), Failure> {
    if action == Action::Launch && item.path.starts_with("velo-action:") {
        return super::system_tools::action(owner, item);
    }
    let windows = std::env::var_os("WINDIR").ok_or(Failure::Unavailable)?;
    let command = launch::command(item, action, Path::new(&windows), |path| {
        std::fs::metadata(path).map(|metadata| metadata.is_dir())
    })
    .map_err(|error| match error {
        CommandError::MissingTarget => Failure::MissingTarget,
        CommandError::Unavailable => Failure::Unavailable,
    })?;
    if action == Action::Launch
        && item.path.starts_with(launch::APPS_FOLDER_PREFIX)
        && missing_apps_folder_target(&item.path)?
    {
        return Err(Failure::MissingTarget);
    }
    let path = wide(&command.path).map_err(|_| Failure::Unavailable)?;
    let arguments = wide(&command.arguments).map_err(|_| Failure::Unavailable)?;
    let directory = wide(&command.directory).map_err(|_| Failure::Unavailable)?;
    let verb = wide(command.verb).map_err(|_| Failure::Unavailable)?;
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
        hwnd: owner as HWND,
        lpVerb: if command.verb.is_empty() {
            null()
        } else {
            verb.as_ptr()
        },
        lpFile: path.as_ptr(),
        lpParameters: arguments.as_ptr(),
        lpDirectory: directory.as_ptr(),
        nShow: SW_SHOWNORMAL,
        hProcess: null_mut(),
        ..Default::default()
    };
    // SAFETY: caller holds an STA; HWND remains owned by the UI until worker
    // completion. NOASYNC keeps Shell from retaining stack-backed strings.
    if unsafe { ShellExecuteExW(&mut info) } == 0 {
        let code = io::Error::last_os_error()
            .raw_os_error()
            .map(|code| code as u32);
        return Err(match code {
            Some(ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND | ERROR_CANCELLED)
                if action != Action::OpenDirectory
                    && Path::new(&item.path).is_absolute()
                    && missing_file_target(apartment, &command.path) =>
            {
                Failure::MissingTarget
            }
            Some(ERROR_CANCELLED) => Failure::Cancelled,
            _ => Failure::Shell,
        });
    }
    Ok(())
}

fn missing_file_target(apartment: &Apartment, path: &str) -> bool {
    let missing = |path: &Path| {
        path.is_absolute()
            && std::fs::metadata(path).is_err_and(|error| error.kind() == ErrorKind::NotFound)
    };
    let target = Path::new(path);
    missing(target)
        || (extension(path, "lnk")
            && ShortcutResolver(apartment)
                .resolve(target)
                .is_ok_and(|shortcut| missing(Path::new(&shortcut.exec_path))))
}

fn missing_apps_folder_target(path: &str) -> Result<bool, Failure> {
    let path = wide(path).map_err(|_| Failure::Unavailable)?;
    let mut item = null_mut();
    let status = unsafe { SHParseDisplayName(path.as_ptr(), null_mut(), &mut item, 0, null_mut()) };
    if !item.is_null() {
        unsafe { CoTaskMemFree(item.cast()) };
    }
    Ok([ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND]
        .into_iter()
        .any(|code| status == windows::core::HRESULT::from_win32(code).0))
}

pub fn open_native(path: &str, owner: isize, locate: bool) -> Result<(), Failure> {
    let target = std::path::Path::new(path);
    if !target.is_absolute()
        || !std::fs::metadata(target).is_ok_and(|metadata| metadata.is_dir() || metadata.is_file())
    {
        return Err(Failure::Unavailable);
    }
    if locate {
        let path = wide(path).map_err(|_| Failure::Unavailable)?;
        let mut item = null_mut();
        // SAFETY: STA is held by the caller. Shell allocates this absolute PIDL;
        // free it after the synchronous call that opens its parent and selects it.
        let parsed = unsafe {
            windows_sys::Win32::UI::Shell::SHParseDisplayName(
                path.as_ptr(),
                null_mut(),
                &mut item,
                0,
                null_mut(),
            )
        };
        if parsed < 0 || item.is_null() {
            return Err(Failure::Shell);
        }
        let opened = unsafe {
            let status =
                windows_sys::Win32::UI::Shell::SHOpenFolderAndSelectItems(item, 0, null(), 0);
            windows_sys::Win32::System::Com::CoTaskMemFree(item.cast());
            status
        };
        return if opened >= 0 {
            Ok(())
        } else {
            Err(Failure::Shell)
        };
    }
    let (executable, arguments) = (target.to_owned(), Vec::<u16>::new());
    let path = wide(&executable.to_string_lossy()).map_err(|_| Failure::Shell)?;
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
        hwnd: owner as HWND,
        lpFile: path.as_ptr(),
        lpParameters: if arguments.is_empty() {
            null()
        } else {
            arguments.as_ptr()
        },
        nShow: SW_SHOWNORMAL,
        ..Default::default()
    };
    // SAFETY: both owned strings stay alive until ShellExecuteExW returns;
    // SEE_MASK_NOASYNC prevents Windows retaining their buffers.
    if unsafe { ShellExecuteExW(&mut info) } == 0 {
        Err(Failure::Shell)
    } else {
        Ok(())
    }
}

pub fn open_url(url: &str) -> Result<(), Failure> {
    let url = wide(url).map_err(|_| Failure::Shell)?;
    // SAFETY: the read-only URL and verb remain live through the synchronous Shell call.
    let verb = wide("open").map_err(|_| Failure::Shell)?;
    let result = unsafe {
        ShellExecuteW(
            null_mut(),
            verb.as_ptr(),
            url.as_ptr(),
            null(),
            null(),
            SW_SHOWNORMAL,
        )
    };
    if result as isize <= 32 {
        Err(Failure::Shell)
    } else {
        Ok(())
    }
}
