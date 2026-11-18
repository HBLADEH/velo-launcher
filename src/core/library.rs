//! Imports register files without executing or pinning them. The caller holds
//! the shared commit lock so refresh acceptance and deletion cannot race.
use super::{
    DataError, Result,
    data::ApplicationLibrary,
    icons,
    indexer::{self, Resolver},
    model::{AppItem, identity},
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
pub fn item(path: &Path, resolver: &mut dyn Resolver) -> Result<AppItem> {
    if !path.is_absolute() {
        return Err(DataError::new(
            "import application",
            "absolute path required",
        ));
    }
    let path = clean(path);
    let path_text = path.to_string_lossy().into_owned();
    if !(indexer::extension(&path_text, "exe") || indexer::extension(&path_text, "lnk")) {
        return Err(DataError::new(
            "import application",
            "unsupported extension",
        ));
    }
    let metadata = std::fs::metadata(&path).map_err(|e| DataError::new("read imported file", e))?;
    if !metadata.is_file() {
        return Err(DataError::new("import application", "not a file"));
    }
    let mut item = if indexer::extension(&path_text, "lnk") {
        resolver.resolve(&path)?
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
        return Err(DataError::new(
            "import applications",
            "100 file batch limit",
        ));
    }
    let mut result = Import {
        library: library.clone(),
        added: 0,
        rejected: 0,
    };
    for path in paths {
        let item = match item(path, resolver) {
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
    if !library
        .applications
        .iter()
        .flatten()
        .any(|item| item.id == id)
    {
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
