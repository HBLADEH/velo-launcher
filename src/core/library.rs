//! Build explicit library entries for the runtime's serialized updates.
use super::{
    DataError, Result,
    data::ApplicationLibrary,
    icons,
    indexer::{self, Resolver},
    model::{AppItem, FOLDER_SOURCE, identity},
};
use std::path::{Component, Path, PathBuf};

fn clean(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        if component == Component::ParentDir {
            result.pop();
        } else if component != Component::CurDir {
            result.push(component);
        }
    }
    result
}

fn folder_path(path: &Path) -> Result<PathBuf> {
    if !path.is_absolute() || path.to_str().is_none_or(|path| path.contains('\0')) {
        return Err(DataError::new(
            "pin folder path",
            "valid absolute path required",
        ));
    }
    Ok(clean(path))
}

pub fn folder_id(path: &Path) -> Result<String> {
    Ok(folder_identity(&folder_path(path)?))
}

fn folder_identity(path: &Path) -> String {
    format!("folder:{}", identity(&path.to_string_lossy(), ""))
}

pub fn folder(path: &Path) -> Result<AppItem> {
    let path = folder_path(path)?;
    let metadata = std::fs::metadata(&path).map_err(|e| DataError::new("read pinned folder", e))?;
    if !metadata.is_dir() {
        return Err(DataError::new("read pinned folder", "not a directory"));
    }
    Ok(folder_item(&path))
}

fn folder_item(path: &Path) -> AppItem {
    let name = path.file_name().unwrap_or(path.as_os_str());
    AppItem {
        id: folder_identity(path),
        name: name.to_string_lossy().into_owned(),
        path: path.to_string_lossy().into_owned(),
        source: FOLDER_SOURCE.into(),
        ..AppItem::default()
    }
}

#[derive(Clone, Copy)]
pub enum ImportKind {
    Executable,
    Shortcut,
    InternetShortcut,
    Folder,
}

pub fn inspect_import(path: &Path) -> Result<ImportKind> {
    if !path.is_absolute() || path.to_str().is_none_or(|path| path.contains('\0')) {
        return Err(DataError::new(
            "validate import path",
            "valid absolute path required",
        ));
    }
    let metadata = std::fs::metadata(path).map_err(|e| DataError::new("read import target", e))?;
    if metadata.is_dir() {
        return Ok(ImportKind::Folder);
    }
    if metadata.is_file() {
        let path = path.to_string_lossy();
        if indexer::extension(&path, "exe") {
            return Ok(ImportKind::Executable);
        }
        if indexer::extension(&path, "lnk") {
            return Ok(ImportKind::Shortcut);
        }
        if indexer::extension(&path, "url") {
            return Ok(ImportKind::InternetShortcut);
        }
    }
    Err(DataError::new(
        "validate import type",
        "unsupported entry type",
    ))
}

pub fn item(path: &Path, resolver: &mut dyn Resolver) -> Result<AppItem> {
    let path = clean(path);
    match inspect_import(&path)? {
        ImportKind::Folder => Err(DataError::new(
            "import application",
            "not an application file",
        )),
        kind => application_item(&path, kind, resolver),
    }
}

fn application_item(path: &Path, kind: ImportKind, resolver: &mut dyn Resolver) -> Result<AppItem> {
    let path_text = path.to_string_lossy().into_owned();
    let mut item = if matches!(kind, ImportKind::Shortcut) {
        resolver.resolve(path)?
    } else {
        AppItem {
            exec_path: path_text.clone(),
            working_directory: path
                .parent()
                .unwrap_or(Path::new(""))
                .to_string_lossy()
                .into_owned(),
            ..AppItem::default()
        }
    };
    item.path = path_text;
    item.name = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    item.source = "Manual".into();
    item.pinned = false;
    item.id = identity(
        if item.exec_path.is_empty() {
            &item.path
        } else {
            &item.exec_path
        },
        &item.arguments,
    );
    item.icon_url = icons::signature(&item);
    Ok(item)
}

pub struct Import {
    pub library: ApplicationLibrary,
    pub added: usize,
    pub rejected: usize,
}
pub fn import(
    library: &ApplicationLibrary,
    paths: &[PathBuf],
    resolver: &mut dyn Resolver,
) -> Result<Import> {
    if paths.len() > 100 {
        return Err(DataError::new("import entries", "100 entry batch limit"));
    }
    let mut result = Import {
        library: library.clone(),
        added: 0,
        rejected: 0,
    };
    for path in paths {
        let path = clean(path);
        let item = match inspect_import(&path).and_then(|kind| match kind {
            ImportKind::Folder => Ok(folder_item(&path)),
            _ => application_item(&path, kind, resolver),
        }) {
            Ok(item) => item,
            Err(_) => {
                result.rejected += 1;
                continue;
            }
        };
        let apps = result.library.applications.get_or_insert_default();
        if apps.iter().any(|old| old.id == item.id) {
            continue;
        }
        if apps.len() == 1000 {
            result.rejected += 1;
            continue;
        }
        apps.push(item);
        result.added += 1;
    }
    Ok(result)
}
pub fn delete(library: &ApplicationLibrary, id: &str) -> Result<ApplicationLibrary> {
    if !library.has_custom_app(id) {
        return Err(DataError::new(
            "delete custom application",
            "missing application",
        ));
    }
    let mut next = library.clone();
    next.applications
        .iter_mut()
        .for_each(|items| items.retain(|item| item.id != id));
    next.pinned
        .iter_mut()
        .for_each(|items| items.retain(|item| item.id != id));
    Ok(next)
}
