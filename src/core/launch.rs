//! Shell command construction is independent of HWND and COM ownership.

use super::indexer::extension;
use super::model::AppItem;
use std::io::{self, ErrorKind};
use std::path::Path;

pub const APPS_FOLDER_PREFIX: &str = "shell:AppsFolder\\";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Launch,
    RunAsAdmin,
    OpenDirectory,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandError {
    MissingTarget,
    Unavailable,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Command {
    pub verb: &'static str,
    pub path: String,
    pub arguments: String,
    pub directory: String,
}

fn target(item: &AppItem) -> &str {
    if item.exec_path.is_empty() || extension(&item.path, "url") {
        &item.path
    } else {
        &item.exec_path
    }
}

pub fn capabilities(item: &AppItem) -> (bool, bool) {
    let path = target(item);
    let file = !item.is_folder()
        && !item.is_launch_group()
        && Path::new(path).is_absolute()
        && !extension(path, "lnk");
    (file, file && extension(path, "exe"))
}

fn target_metadata(
    path: &Path,
    metadata: impl FnOnce(&Path) -> io::Result<bool>,
) -> Result<bool, CommandError> {
    metadata(path).map_err(|error| {
        if path.is_absolute() && error.kind() == ErrorKind::NotFound {
            CommandError::MissingTarget
        } else {
            CommandError::Unavailable
        }
    })
}

pub fn command(
    item: &AppItem,
    action: Action,
    windows_directory: &Path,
    metadata: impl Fn(&Path) -> io::Result<bool>,
) -> Result<Command, CommandError> {
    if item.is_launch_group() {
        return Err(CommandError::Unavailable);
    }
    let empty = |path: String| Command {
        verb: "",
        path,
        arguments: String::new(),
        directory: String::new(),
    };
    if item.is_folder() {
        let path = Path::new(&item.path);
        if action != Action::Launch || !path.is_absolute() {
            return Err(CommandError::Unavailable);
        }
        if !target_metadata(path, metadata)? {
            return Err(CommandError::MissingTarget);
        }
        return Ok(empty(item.path.clone()));
    }
    if action == Action::OpenDirectory {
        if !capabilities(item).0 {
            return Err(CommandError::Unavailable);
        }
        let directory = Path::new(target(item))
            .parent()
            .ok_or(CommandError::Unavailable)?;
        if !metadata(directory).map_err(|_| CommandError::Unavailable)? {
            return Err(CommandError::Unavailable);
        }
        return Ok(empty(directory.to_string_lossy().into_owned()));
    }
    if action == Action::RunAsAdmin {
        if !capabilities(item).1 || target_metadata(Path::new(target(item)), metadata)? {
            return Err(CommandError::Unavailable);
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
            Err(CommandError::Unavailable)
        };
    }
    if item.path.starts_with(APPS_FOLDER_PREFIX) {
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
    let directory = target_metadata(Path::new(&item.path), metadata)?;
    if extension(&item.path, "url") && directory {
        return Err(CommandError::MissingTarget);
    }
    if extension(&item.path, "lnk") || extension(&item.path, "url") {
        return Ok(empty(item.path.clone()));
    }
    Ok(Command {
        arguments: item.arguments.clone(),
        directory: item.working_directory.clone(),
        ..empty(item.exec_path.clone())
    })
}
