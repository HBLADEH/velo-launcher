//! COM objects remain on their creating worker. Native interfaces provide
//! shortcut metadata, AppsFolder enumeration and icon extraction.

use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use windows::Win32::System::Com::*;
use windows::Win32::UI::Shell::*;
use windows::core::{Interface, PCWSTR, PWSTR};

use crate::core::config::Config;
use crate::core::indexer::{Resolver, Root, Ticket};
use crate::core::model::{AppItem, identity};
use crate::core::{DataError, Result};

pub fn wide(text: &str) -> Result<Vec<u16>> {
    if text.contains('\0') {
        return Err(DataError::new("encode Shell argument", "embedded NUL"));
    }
    Ok(text.encode_utf16().chain([0]).collect())
}

pub struct Apartment(PhantomData<Rc<()>>);
impl Apartment {
    pub fn new() -> Result<Self> {
        // SAFETY: called once per dedicated worker; Rc marker prevents transfer.
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok() }
            .map_err(|e| DataError::new("initialize Shell COM", e))?;
        Ok(Self(PhantomData))
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        // SAFETY: same thread, after its COM interfaces have been released.
        unsafe { CoUninitialize() };
    }
}

fn allocated_string(value: PWSTR) -> Result<String> {
    // SAFETY: Shell returns a NUL-terminated CoTaskMem allocation. Free even
    // if conversion fails, and never expose its contents through diagnostics.
    let result = unsafe { value.to_string() }.map_err(|e| DataError::new("decode Shell string", e));
    unsafe { CoTaskMemFree(Some(value.0.cast())) };
    result
}

pub fn roots(config: &Config) -> Result<Vec<Root>> {
    let mut result = Vec::new();
    let mut add = |id, source: &str, depth| -> Result<()> {
        // SAFETY: known folder GUID is static, returned memory freed below.
        let value = unsafe { SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None) }
            .map_err(|e| DataError::new("find Windows application directory", e))?;
        result.push(Root {
            path: PathBuf::from(allocated_string(value)?),
            source: source.into(),
            max_depth: depth,
        });
        Ok(())
    };
    add(&FOLDERID_Programs, "Start Menu", 0)?;
    add(&FOLDERID_CommonPrograms, "Start Menu", 0)?;
    add(&FOLDERID_Desktop, "Desktop", 0)?;
    add(&FOLDERID_PublicDesktop, "Desktop", 0)?;
    if config.scan_program_files {
        let depth = if config.filter_noise { 2 } else { 0 };
        add(&FOLDERID_ProgramFiles, "Program Files", depth)?;
        add(&FOLDERID_ProgramFilesX86, "Program Files (x86)", depth)?;
    }
    result.extend(config.custom_directories.iter().flatten().map(|path| Root {
        path: path.into(),
        source: "Custom".into(),
        max_depth: 0,
    }));
    Ok(result)
}

pub struct ShortcutResolver<'a>(pub &'a Apartment);
impl Resolver for ShortcutResolver<'_> {
    fn resolve(&mut self, path: &Path) -> Result<AppItem> {
        let path = wide(&path.to_string_lossy())?;
        // SAFETY: apartment outlives the interfaces, buffers are sized and live
        // until each synchronous Shell call returns. Never resolve with UI.
        unsafe {
            let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
                .map_err(|e| DataError::new("create shortcut reader", e))?;
            let file: IPersistFile = link
                .cast()
                .map_err(|e| DataError::new("open shortcut interface", e))?;
            file.Load(PCWSTR(path.as_ptr()), STGM_READ)
                .map_err(|e| DataError::new("load shortcut", e))?;
            let read = |get: &dyn Fn(&mut [u16]) -> windows::core::Result<()>| -> Result<String> {
                let mut buffer = vec![0; 32768];
                get(&mut buffer).map_err(|e| DataError::new("read shortcut field", e))?;
                String::from_utf16(
                    &buffer[..buffer.iter().position(|c| *c == 0).unwrap_or(buffer.len())],
                )
                .map_err(|e| DataError::new("decode shortcut field", e))
            };
            let mut index = 0;
            let mut icon = vec![0; 32768];
            link.GetIconLocation(&mut icon, &mut index)
                .map_err(|e| DataError::new("read shortcut icon", e))?;
            let icon = String::from_utf16_lossy(
                &icon[..icon.iter().position(|c| *c == 0).unwrap_or(icon.len())],
            );
            Ok(AppItem {
                exec_path: read(&|buffer| {
                    link.GetPath(buffer, std::ptr::null_mut(), SLGP_UNCPRIORITY.0 as u32)
                })?,
                arguments: read(&|buffer| link.GetArguments(buffer))?,
                working_directory: read(&|buffer| link.GetWorkingDirectory(buffer))?,
                description: read(&|buffer| link.GetDescription(buffer))?,
                icon_path: if icon.is_empty() {
                    String::new()
                } else {
                    format!("{icon},{index}")
                },
                keywords: Some(Vec::new()),
                ..AppItem::default()
            })
        }
    }
}

pub fn packaged_apps(_apartment: &Apartment, ticket: &Ticket) -> Result<Vec<AppItem>> {
    let path = wide("shell:AppsFolder")?;
    // SAFETY: all interfaces are released before the worker's STA is closed;
    // one-item buffers and fetched counts follow IEnumShellItems' contract.
    unsafe {
        let folder: IShellItem = SHCreateItemFromParsingName(PCWSTR(path.as_ptr()), None)
            .map_err(|e| DataError::new("open AppsFolder", e))?;
        let entries: IEnumShellItems = folder
            .BindToHandler(None, &BHID_EnumItems)
            .map_err(|e| DataError::new("enumerate AppsFolder", e))?;
        let mut apps = Vec::new();
        loop {
            ticket.check()?;
            let mut item = [None];
            let mut fetched = 0;
            entries
                .Next(&mut item, Some(&mut fetched))
                .map_err(|e| DataError::new("read AppsFolder", e))?;
            let Some(item) = item[0].take().filter(|_| fetched > 0) else {
                break;
            };
            let parsing = allocated_string(
                item.GetDisplayName(SIGDN_DESKTOPABSOLUTEPARSING)
                    .map_err(|e| DataError::new("read application identity", e))?,
            )?;
            if !parsing.contains('!') {
                continue;
            }
            let app_id = parsing.rsplit('\\').next().unwrap_or(&parsing);
            let path = format!("shell:AppsFolder\\{app_id}");
            let name = allocated_string(
                item.GetDisplayName(SIGDN_NORMALDISPLAY)
                    .map_err(|e| DataError::new("read application name", e))?,
            )?;
            apps.push(AppItem {
                id: identity(&path, ""),
                name,
                exec_path: path.clone(),
                path,
                source: "Windows Apps".into(),
                keywords: Some(Vec::new()),
                ..AppItem::default()
            });
        }
        Ok(apps)
    }
}
