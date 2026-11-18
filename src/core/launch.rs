//! Shell command construction is independent of HWND and COM ownership.

use super::indexer::extension;
use super::model::AppItem;
use super::{DataError, Result};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Launch,
    RunAsAdmin,
    OpenDirectory,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Command {
    pub verb: &'static str,
    pub path: String,
    pub arguments: String,
    pub directory: String,
}

fn target(item: &AppItem) -> &str {
    if item.exec_path.is_empty() {
        &item.path
    } else {
        &item.exec_path
    }
}

pub fn capabilities(item: &AppItem) -> (bool, bool) {
    let path = target(item);
    let file = Path::new(path).is_absolute() && !extension(path, "lnk");
    (file, file && extension(path, "exe"))
}

pub fn command(
    item: &AppItem,
    action: Action,
    windows_directory: &Path,
    metadata: impl Fn(&Path) -> std::io::Result<bool>,
) -> Result<Command> {
    let empty = |path: String| Command {
        verb: "",
        path,
        arguments: String::new(),
        directory: String::new(),
    };
    let fail = || DataError::new("prepare launch", "unsupported or unavailable target");
    if action == Action::OpenDirectory {
        if !capabilities(item).0 {
            return Err(fail());
        }
        let directory = Path::new(target(item)).parent().ok_or_else(fail)?;
        if !metadata(directory).map_err(|e| DataError::new("open install directory", e))? {
            return Err(fail());
        }
        return Ok(empty(directory.to_string_lossy().into_owned()));
    }
    if action == Action::RunAsAdmin {
        if !capabilities(item).1
            || metadata(Path::new(target(item)))
                .map_err(|e| DataError::new("find executable", e))?
        {
            return Err(fail());
        }
        return Ok(Command {
            verb: "runas",
            path: target(item).into(),
            arguments: item.arguments.clone(),
            directory: item.working_directory.clone(),
        });
    }
    if item.path.starts_with("velo-action:") || item.path.starts_with("ms-settings:") {
        return if item.path.starts_with("ms-settings:") && super::system_tools::special(item) {
            Ok(empty(item.path.clone()))
        } else {
            Err(fail())
        };
    }
    if item.path.starts_with("shell:AppsFolder\\") {
        return Ok(Command {
            arguments: item.path.clone(),
            ..empty(
                windows_directory
                    .join("explorer.exe")
                    .to_string_lossy()
                    .into_owned(),
            )
        });
    }
    metadata(Path::new(&item.path)).map_err(|e| DataError::new("find application", e))?;
    if extension(&item.path, "lnk") {
        return Ok(empty(item.path.clone()));
    }
    Ok(Command {
        arguments: item.arguments.clone(),
        directory: item.working_directory.clone(),
        ..empty(item.exec_path.clone())
    })
}
