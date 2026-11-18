use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

use chrono::{DateTime, Utc};
use tempfile::Builder;

use super::{DataError, JsonDocument, Result};

pub fn read<T: JsonDocument>(path: &Path) -> Result<T> {
    let bytes = fs::read(path).map_err(|e| DataError::new("read data file", e))?;
    T::decode(&bytes)
}

pub fn write<T: JsonDocument>(path: &Path, value: &T) -> Result<()> {
    write_checked(path, value, || Ok(()))
}

pub(crate) fn write_checked<T: JsonDocument>(
    path: &Path,
    value: &T,
    check: impl Fn() -> Result<()>,
) -> Result<()> {
    check()?;
    value.validate()?;
    let bytes =
        serde_json::to_vec_pretty(value).map_err(|e| DataError::new("encode data file", e))?;
    replace_bytes(path, &bytes, |_| check().map_err(io::Error::other))
}

#[derive(Clone, Copy, Debug)]
enum WriteStage {
    BeforeFlush,
    BeforeReplace,
}

fn replace_bytes(
    path: &Path,
    bytes: &[u8],
    mut checkpoint: impl FnMut(WriteStage) -> io::Result<()>,
) -> Result<()> {
    let directory = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(directory).map_err(|e| DataError::new("create data directory", e))?;
    let mut temporary = Builder::new()
        .prefix(".velo-")
        .suffix(".tmp")
        .tempfile_in(directory)
        .map_err(|e| DataError::new("create temporary file", e))?;
    temporary
        .write_all(bytes)
        .map_err(|e| DataError::new("write temporary file", e))?;
    checkpoint(WriteStage::BeforeFlush).map_err(|e| DataError::new("flush temporary file", e))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|e| DataError::new("flush temporary file", e))?;
    // Close before Win32 replacement; TempPath removes failed temporary writes.
    let temporary = temporary.into_temp_path();
    checkpoint(WriteStage::BeforeReplace).map_err(|e| DataError::new("replace data file", e))?;
    crate::atomic_file::move_file(&temporary, path, true)
        .map_err(|e| DataError::new("replace data file", e))
}

pub(crate) fn preserve_bytes(directory: &Path, prefix: &str, bytes: &[u8]) -> Result<PathBuf> {
    let directory = std::path::absolute(directory)
        .map_err(|e| DataError::new("resolve backup directory", e))?;
    fs::create_dir_all(&directory).map_err(|e| DataError::new("create backup directory", e))?;
    let mut temporary = Builder::new()
        .prefix(".config-")
        .suffix(".tmp")
        .tempfile_in(&directory)
        .map_err(|e| DataError::new("create backup", e))?;
    temporary
        .write_all(bytes)
        .map_err(|e| DataError::new("write backup", e))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|e| DataError::new("flush backup", e))?;
    let timestamp: DateTime<Utc> = std::time::SystemTime::now().into();
    let suffix = temporary
        .path()
        .file_stem()
        .and_then(|s| s.to_str())
        .and_then(|s| s.strip_prefix(".config-"))
        .ok_or_else(|| DataError::new("name backup", "invalid temporary file name"))?
        .to_owned();
    let destination = directory.join(format!(
        "{prefix}-{}-{suffix}.json",
        timestamp.format("%Y%m%d-%H%M%S")
    ));
    // Never replace an existing backup, even when several are made in one second.
    let temporary = temporary.into_temp_path();
    crate::atomic_file::move_file(&temporary, &destination, false)
        .map_err(|e| DataError::new("commit backup", e))?;
    Ok(destination)
}

/// Writers serialize the whole read/modify/flush/publish operation. Readers only
/// hold a short lock to acquire the last committed Arc; disk I/O never blocks them.
pub struct Store<T> {
    path: PathBuf,
    commit: Mutex<()>,
    snapshot: RwLock<Arc<T>>,
}

impl<T: JsonDocument> Store<T> {
    pub(crate) fn from_loaded(path: PathBuf, value: T) -> Self {
        Self {
            path,
            commit: Mutex::new(()),
            snapshot: RwLock::new(Arc::new(value)),
        }
    }

    pub fn snapshot(&self) -> Result<Arc<T>> {
        self.snapshot
            .read()
            .map(|value| Arc::clone(&value))
            .map_err(|_| DataError::new("read committed snapshot", "snapshot lock poisoned"))
    }

    pub fn update(&self, change: impl FnOnce(&T) -> Result<T>) -> Result<Arc<T>> {
        let _commit = self
            .commit
            .lock()
            .map_err(|_| DataError::new("commit data", "commit lock poisoned"))?;
        let current = self.snapshot()?;
        let next = change(&current)?;
        write(&self.path, &next)?;
        let next = Arc::new(next);
        *self
            .snapshot
            .write()
            .map_err(|_| DataError::new("publish data", "snapshot lock poisoned"))? =
            Arc::clone(&next);
        Ok(next)
    }

    pub fn backup(&self, directory: &Path) -> Result<PathBuf> {
        let _commit = self
            .commit
            .lock()
            .map_err(|_| DataError::new("backup data", "commit lock poisoned"))?;
        let bytes = fs::read(&self.path).map_err(|e| DataError::new("read backup source", e))?;
        preserve_bytes(directory, "config", &bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::Config;
    use std::process::Command;

    #[test]
    fn write_failures_before_flush_and_replacement_preserve_the_previous_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        write(&path, &Config::default()).unwrap();
        let previous = fs::read(&path).unwrap();
        for fail_at_flush in [true, false] {
            let error = replace_bytes(&path, b"new bytes", |stage| {
                if fail_at_flush == matches!(stage, WriteStage::BeforeFlush) {
                    Err(io::Error::other("injected storage failure"))
                } else {
                    Ok(())
                }
            });
            assert!(error.is_err());
            assert_eq!(fs::read(&path).unwrap(), previous);
            assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
        }
    }

    #[test]
    fn interrupted_writer_child() {
        let Some(path) = std::env::var_os("VELO_RS08_INTERRUPTED_WRITE") else {
            return;
        };
        let _ = replace_bytes(Path::new(&path), b"incomplete new document", |stage| {
            if matches!(stage, WriteStage::BeforeReplace) {
                // Exit without destructors after the new file was flushed.
                std::process::exit(73);
            }
            Ok(())
        });
        panic!("child did not reach the replacement boundary");
    }

    #[test]
    fn process_interruption_after_flush_does_not_truncate_old_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        write(&path, &Config::default()).unwrap();
        let previous = fs::read(&path).unwrap();
        let status = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "core::storage::tests::interrupted_writer_child"])
            .env("VELO_RS08_INTERRUPTED_WRITE", &path)
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(73));
        assert_eq!(fs::read(&path).unwrap(), previous);
        assert_eq!(read::<Config>(&path).unwrap(), Config::default());
        // Orphan temporary files are ignored; a later normal commit still works.
        write(&path, &Config::default()).unwrap();
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }
}
