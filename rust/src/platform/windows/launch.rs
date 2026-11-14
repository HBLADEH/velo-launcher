use std::ptr::{null, null_mut};
use windows_sys::Win32::Foundation::{ERROR_CANCELLED, HWND};
use windows_sys::Win32::UI::Shell::{
    SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SHELLEXECUTEINFOW, ShellExecuteExW,
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
