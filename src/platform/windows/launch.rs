use std::ptr::{null, null_mut};
use windows_sys::Win32::Foundation::{ERROR_CANCELLED, HWND};
use windows_sys::Win32::UI::Shell::{
    SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SHELLEXECUTEINFOW, ShellExecuteExW, ShellExecuteW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use super::shell::{Apartment, wide};
use crate::core::launch::{self, Action};
use crate::core::model::AppItem;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    Unavailable,
    Cancelled,
    Shell,
}

impl Failure {
    pub fn message(self) -> &'static str {
        match self {
            Self::Unavailable => "应用已移动、删除或不支持此操作，请刷新后重试",
            Self::Cancelled => "已取消启动",
            Self::Shell => "Windows 无法打开此应用，请检查权限或文件关联后重试",
        }
    }
}

pub fn execute(
    _apartment: &Apartment,
    owner: isize,
    item: &AppItem,
    action: Action,
) -> Result<(), Failure> {
    if action == Action::Launch && item.path.starts_with("velo-action:") {
        return super::system_tools::action(owner, item);
    }
    let windows = std::env::var_os("WINDIR").ok_or(Failure::Unavailable)?;
    let command = launch::command(item, action, std::path::Path::new(&windows), |p| {
        std::fs::metadata(p).map(|m| m.is_dir())
    })
    .map_err(|_| Failure::Unavailable)?;
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
        return Err(
            if std::io::Error::last_os_error().raw_os_error() == Some(ERROR_CANCELLED as i32) {
                Failure::Cancelled
            } else {
                Failure::Shell
            },
        );
    }
    Ok(())
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
