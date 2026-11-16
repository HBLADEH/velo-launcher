//! Owned native interactions run on one temporary STA thread at a time. The
//! Slint thread stays responsive; neither menu nor dialog executes a file.

use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::ptr::{null, null_mut};

use crate::core::{launch, model::AppItem};
use windows_sys::Win32::Foundation::{CloseHandle, HWND, POINT};
use windows_sys::Win32::Security::{
    GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation,
};
use windows_sys::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows_sys::Win32::UI::Controls::Dialogs::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

pub const BROWSE: i32 = 1;
pub const CLEAR: i32 = 2;
pub const BACK: i32 = 3;
pub const MENU: i32 = 4;
pub const BROWSE_EXECUTABLE: i32 = 5;

pub fn new_group_id() -> Option<String> {
    let mut id = windows_sys::core::GUID::from_u128(0);
    // SAFETY: id is a live writable GUID; CoCreateGuid needs no COM apartment.
    if unsafe { windows_sys::Win32::System::Com::CoCreateGuid(&mut id) } < 0 {
        return None;
    }
    Some(format!(
        "group:{:08x}{:04x}{:04x}{}",
        id.data1,
        id.data2,
        id.data3,
        id.data4
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    ))
}

#[derive(Default)]
pub struct Completion {
    pub action: i32,
    pub paths: Vec<PathBuf>,
    pub error: String,
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}

pub fn elevated() -> bool {
    unsafe {
        let mut token = null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION::default();
        let mut length = 0;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut _ as _,
            std::mem::size_of_val(&elevation) as u32,
            &mut length,
        ) != 0;
        CloseHandle(token);
        ok && elevation.TokenIsElevated != 0
    }
}

pub fn run(owner: isize, request: i32) -> Completion {
    // SAFETY: owner is the live Slint HWND. The caller prevents hide/quit and
    // concurrent interactions until the completion returns to its UI thread.
    unsafe {
        let initialized = CoInitializeEx(null(), COINIT_APARTMENTTHREADED as u32) >= 0;
        let owner = owner as HWND;
        let result = match request {
            BROWSE | BROWSE_EXECUTABLE => browse(owner, request == BROWSE_EXECUTABLE),
            CLEAR => Completion {
                action: if MessageBoxW(
                    owner,
                    wide("清空当前导入预览？原文件不会被修改。").as_ptr(),
                    wide("Velo · 清空预览").as_ptr(),
                    MB_OKCANCEL | MB_ICONQUESTION | MB_DEFBUTTON2,
                ) == IDOK
                {
                    CLEAR
                } else {
                    0
                },
                ..Default::default()
            },
            MENU => menu(owner),
            _ => Completion::default(),
        };
        if initialized {
            CoUninitialize();
        }
        result
    }
}

unsafe fn browse(owner: HWND, executable_only: bool) -> Completion {
    let mut buffer = vec![0u16; 65536];
    let title = wide(if executable_only {
        "Velo · 选择实际运行的主程序"
    } else {
        "Velo · 选择应用文件"
    });
    let filter = wide(if executable_only {
        "应用程序 (*.exe)\0*.exe\0"
    } else {
        "应用与快捷方式 (*.exe;*.lnk)\0*.exe;*.lnk\0所有文件 (*.*)\0*.*\0"
    });
    let mut options = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: owner,
        lpstrTitle: title.as_ptr(),
        lpstrFilter: filter.as_ptr(),
        nFilterIndex: 1,
        lpstrFile: buffer.as_mut_ptr(),
        nMaxFile: buffer.len() as u32,
        Flags: OFN_EXPLORER
            | if executable_only {
                0
            } else {
                OFN_ALLOWMULTISELECT
            }
            | OFN_FILEMUSTEXIST
            | OFN_PATHMUSTEXIST
            | OFN_NOCHANGEDIR
            | OFN_NODEREFERENCELINKS,
        ..Default::default()
    };
    if unsafe { GetOpenFileNameW(&mut options) } != 0 {
        match parse_selection(&buffer) {
            Ok(paths) => Completion {
                action: BROWSE,
                paths,
                ..Default::default()
            },
            Err(error) => Completion {
                error: error.into(),
                ..Default::default()
            },
        }
    } else {
        let error = unsafe { CommDlgExtendedError() };
        Completion {
            error: if error == 0 {
                String::new()
            } else {
                format!("文件选择失败（{error:#x}），请减少选择数量后重试。")
            },
            ..Default::default()
        }
    }
}

fn parse_selection(buffer: &[u16]) -> Result<Vec<PathBuf>, &'static str> {
    let end = buffer
        .windows(2)
        .position(|pair| pair == [0, 0])
        .ok_or("文件选择结果不完整。")?;
    let parts: Vec<_> = buffer[..end + 1]
        .split(|&unit| unit == 0)
        .filter(|part| !part.is_empty())
        .map(|part| PathBuf::from(std::ffi::OsString::from_wide(part)))
        .collect();
    let Some(first) = parts.first() else {
        return Ok(Vec::new());
    };
    if !first.is_absolute() {
        return Err("请选择完整文件路径。");
    }
    if parts.len() == 1 {
        return Ok(parts);
    }
    parts[1..]
        .iter()
        .map(|name| {
            if name.file_name() != Some(name.as_os_str()) {
                Err("文件选择结果包含无效文件名。")
            } else {
                Ok(first.join(name))
            }
        })
        .collect()
}

unsafe fn menu(owner: HWND) -> Completion {
    unsafe {
        // A same-thread menu host with a real owner chain. This also lets the
        // UI-thread foreground check recognize the popup without process-wide
        // or IME-wide blur exemptions.
        let host = CreateWindowExW(
            0,
            wide("STATIC").as_ptr(),
            wide("Velo Import Menu").as_ptr(),
            WS_POPUP,
            0,
            0,
            0,
            0,
            owner,
            null_mut(),
            null_mut(),
            null(),
        );
        let menu = CreatePopupMenu();
        if host.is_null() || menu.is_null() {
            if !host.is_null() {
                DestroyWindow(host);
            }
            if !menu.is_null() {
                DestroyMenu(menu);
            }
            return Completion {
                error: "无法打开菜单。".into(),
                ..Default::default()
            };
        }
        AppendMenuW(menu, MF_STRING, BROWSE as usize, wide("选择文件…").as_ptr());
        AppendMenuW(menu, MF_STRING, CLEAR as usize, wide("清空预览…").as_ptr());
        AppendMenuW(menu, MF_SEPARATOR, 0, null());
        AppendMenuW(menu, MF_STRING, BACK as usize, wide("返回搜索").as_ptr());
        let mut point = POINT::default();
        GetCursorPos(&mut point);
        SetForegroundWindow(host);
        let selected = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON,
            point.x,
            point.y,
            0,
            host,
            null(),
        );
        if GetForegroundWindow() == host {
            SetForegroundWindow(owner);
        }
        DestroyMenu(menu);
        DestroyWindow(host);
        Completion {
            action: selected,
            ..Default::default()
        }
    }
}

/// Only an existing catalog item can reach this worker. Return a command; no
/// menu selection performs disk writes or launches before the UI accepts it.
pub fn application(owner: isize, item: Option<&AppItem>, request: i32) -> i32 {
    let owner = owner as HWND;
    // SAFETY: UI retains owner and suppresses hide/quit for this interaction.
    // All Win32 objects are created/destroyed on this temporary worker thread.
    unsafe {
        if request == 5 || request == 6 {
            let message = if request == 5 {
                "移除此自定义应用及其固定项？原文件不会被删除。"
            } else {
                "下载并安装更新？Velo 将备份已保存的配置，退出并重新启动。"
            };
            return if MessageBoxW(
                owner,
                wide(message).as_ptr(),
                wide("Velo · 确认操作").as_ptr(),
                MB_OKCANCEL | MB_DEFBUTTON2 | MB_ICONQUESTION | MB_SETFOREGROUND,
            ) == IDOK
            {
                request
            } else {
                0
            };
        }
        let Some(item) = item else {
            return 0;
        };
        let host = CreateWindowExW(
            0,
            wide("STATIC").as_ptr(),
            wide("Velo Application Menu").as_ptr(),
            WS_POPUP,
            0,
            0,
            0,
            0,
            owner,
            null_mut(),
            null_mut(),
            null(),
        );
        let menu = CreatePopupMenu();
        if host.is_null() || menu.is_null() {
            if !host.is_null() {
                DestroyWindow(host);
            }
            if !menu.is_null() {
                DestroyMenu(menu);
            }
            return -1;
        }
        let (directory, admin) = launch::capabilities(item);
        let target = if item.exec_path.is_empty() {
            &item.path
        } else {
            &item.exec_path
        };
        let directory = directory
            && std::path::Path::new(target)
                .parent()
                .is_some_and(|p| p.is_dir());
        let admin = admin && std::path::Path::new(target).is_file();
        AppendMenuW(menu, MF_STRING | MF_DISABLED, 99, wide(&item.name).as_ptr());
        AppendMenuW(menu, MF_SEPARATOR, 0, null());
        for (id, label, enabled) in [
            (1, "打开", true),
            (2, "打开安装目录", directory),
            (3, "使用管理员权限打开", admin),
            (
                4,
                if item.pinned {
                    "取消置顶"
                } else {
                    "置顶 · 固定到首页"
                },
                true,
            ),
        ] {
            AppendMenuW(
                menu,
                MF_STRING | if enabled { 0 } else { MF_GRAYED },
                id,
                wide(label).as_ptr(),
            );
        }
        let mut point = POINT::default();
        GetCursorPos(&mut point);
        SetForegroundWindow(host);
        let selected = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON,
            point.x,
            point.y,
            0,
            host,
            null(),
        );
        if GetForegroundWindow() == host {
            SetForegroundWindow(owner);
        }
        DestroyMenu(menu);
        DestroyWindow(host);
        selected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picker_keeps_unicode_spaces_and_shortcut_names_for_multi_and_single_selection() {
        let multi = wide("D:\\中文 目录\0程序 一.EXE\0应用.lnk\0");
        assert_eq!(
            parse_selection(&multi).unwrap(),
            vec![
                PathBuf::from("D:\\中文 目录\\程序 一.EXE"),
                PathBuf::from("D:\\中文 目录\\应用.lnk")
            ]
        );
        assert_eq!(
            parse_selection(&wide("D:\\中文 目录\\应用.lnk\0")).unwrap(),
            vec![PathBuf::from("D:\\中文 目录\\应用.lnk")]
        );
        assert!(parse_selection(&wide("relative.exe\0")).is_err());
        assert!(parse_selection(&wide("D:\\目录\0..\\escape.exe\0")).is_err());
        assert!(parse_selection(&[65, 0]).is_err());
        assert!(parse_selection(&[0, 0]).unwrap().is_empty());
    }
}
