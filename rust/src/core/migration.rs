//! One durable, byte-exact user snapshot per machine-install transaction.
//! Called on the discovery worker, before DataStore can recover or write files.
use super::{DataError, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path};

#[derive(Serialize, Deserialize)]
struct Entry {
    path: String,
    sha256: String,
    bytes: u64,
}
#[derive(Serialize, Deserialize)]
struct Manifest {
    schema: u8,
    migration_id: String,
    files: Vec<Entry>,
}

fn failure() -> DataError {
    DataError::new(
        "preserve pre-migration user data",
        "backup incomplete or unsafe",
    )
}
fn ordinary(path: &Path) -> Result<fs::Metadata> {
    use std::os::windows::fs::MetadataExt;
    let metadata = fs::symlink_metadata(path).map_err(|_| failure())?;
    // Reject junctions as well as symlinks, including cloud reparse points.
    if metadata.file_attributes() & 0x400 != 0 {
        return Err(failure());
    }
    Ok(metadata)
}
fn hash(path: &Path) -> Result<String> {
    let mut file = File::open(path).map_err(|_| failure())?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let read = file.read(&mut buffer).map_err(|_| failure())?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}
fn collect(root: &Path, relative: &Path, files: &mut Vec<Entry>, total: &mut u64) -> Result<()> {
    if relative.components().count() > 32 || files.len() >= 50_000 {
        return Err(failure());
    }
    let path = root.join(relative);
    let metadata = ordinary(&path)?;
    if metadata.is_dir() {
        for entry in fs::read_dir(path).map_err(|_| failure())? {
            let name = entry.map_err(|_| failure())?.file_name();
            collect(root, &relative.join(name), files, total)?;
        }
    } else if metadata.is_file() {
        *total = total.checked_add(metadata.len()).ok_or_else(failure)?;
        if *total > 1 << 30 {
            return Err(failure());
        }
        files.push(Entry {
            path: relative.to_str().ok_or_else(failure)?.replace('\\', "/"),
            sha256: hash(&path)?,
            bytes: metadata.len(),
        });
    } else {
        return Err(failure());
    }
    Ok(())
}

pub fn preserve(data: &Path, id: &str) -> Result<()> {
    if id.len() != 32 || !id.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(failure());
    }
    let parent = data.parent().ok_or_else(failure)?;
    let backups = parent.join("VeloRustMigrationBackups");
    fs::create_dir_all(&backups).map_err(|_| failure())?;
    if !ordinary(&backups)?.is_dir() {
        return Err(failure());
    }
    let accepted = backups.join(id);
    if accepted.exists() {
        verify(&accepted, id)?;
        return Ok(());
    }
    let temporary = tempfile::Builder::new()
        .prefix(".pending-")
        .tempdir_in(&backups)
        .map_err(|_| failure())?;
    let mut manifest = Manifest {
        schema: 1,
        migration_id: id.into(),
        files: Vec::new(),
    };
    if data.exists() {
        collect(data, Path::new(""), &mut manifest.files, &mut 0)?;
    }
    manifest.files.sort_by(|a, b| a.path.cmp(&b.path));
    let copies = temporary.path().join("files");
    fs::create_dir(&copies).map_err(|_| failure())?;
    for entry in &manifest.files {
        let target = copies.join(&entry.path);
        fs::create_dir_all(target.parent().ok_or_else(failure)?).map_err(|_| failure())?;
        let mut source = File::open(data.join(&entry.path)).map_err(|_| failure())?;
        let mut output = File::create(&target).map_err(|_| failure())?;
        let copied = std::io::copy(&mut source, &mut output).map_err(|_| failure())?;
        output.sync_all().map_err(|_| failure())?;
        if copied != entry.bytes || hash(&target)? != entry.sha256 {
            return Err(failure());
        }
    }
    let mut marker = File::create(temporary.path().join("manifest.json")).map_err(|_| failure())?;
    marker
        .write_all(&serde_json::to_vec_pretty(&manifest).map_err(|_| failure())?)
        .map_err(|_| failure())?;
    marker.sync_all().map_err(|_| failure())?;
    drop(marker);
    verify(temporary.path(), id)?;
    // Same-volume rename publishes only a fully flushed, re-read snapshot.
    crate::atomic_file::move_file(temporary.path(), &accepted, false).map_err(|_| failure())?;
    eprintln!(
        "event=product_migration_backup_verified files={}",
        manifest.files.len()
    );
    Ok(())
}

fn verify(directory: &Path, id: &str) -> Result<()> {
    ordinary(directory)?;
    let manifest: Manifest =
        serde_json::from_slice(&fs::read(directory.join("manifest.json")).map_err(|_| failure())?)
            .map_err(|_| failure())?;
    if manifest.schema != 1 || manifest.migration_id != id || manifest.files.len() > 50_000 {
        return Err(failure());
    }
    let mut seen = std::collections::BTreeSet::new();
    for entry in manifest.files {
        let relative = Path::new(&entry.path);
        if entry.path.is_empty()
            || relative
                .components()
                .any(|c| !matches!(c, Component::Normal(_)))
            || !seen.insert(entry.path.to_lowercase())
        {
            return Err(failure());
        }
        let file = directory.join("files").join(relative);
        if ordinary(&file)?.len() != entry.bytes || hash(&file)? != entry.sha256 {
            return Err(failure());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const ID: &str = "abcdef0123456789abcdef0123456789";

    #[test]
    fn snapshot_keeps_unknown_files_and_exact_backup_bytes_and_is_not_replaced() {
        let root = tempfile::tempdir().unwrap();
        let data = root.path().join("Velo");
        fs::create_dir_all(data.join("backups")).unwrap();
        fs::write(data.join("config.json"), b"{ invalid original\r\n").unwrap();
        fs::write(data.join("backups/old.json"), b"original backup").unwrap();
        fs::write(data.join("unknown.bin"), [0, 255, 13, 10]).unwrap();
        preserve(&data, ID).unwrap();
        fs::write(data.join("config.json"), b"new user setting").unwrap();
        preserve(&data, ID).unwrap();
        let saved = root
            .path()
            .join("VeloRustMigrationBackups")
            .join(ID)
            .join("files");
        assert_eq!(
            fs::read(saved.join("config.json")).unwrap(),
            b"{ invalid original\r\n"
        );
        assert_eq!(
            fs::read(saved.join("backups/old.json")).unwrap(),
            b"original backup"
        );
        assert_eq!(
            fs::read(saved.join("unknown.bin")).unwrap(),
            [0, 255, 13, 10]
        );
    }

    #[test]
    fn failed_or_tampered_snapshot_never_allows_data_initialization() {
        let root = tempfile::tempdir().unwrap();
        let data = root.path().join("Velo");
        fs::create_dir(&data).unwrap();
        fs::write(data.join("config.json"), b"original").unwrap();
        preserve(&data, ID).unwrap();
        let saved = root.path().join("VeloRustMigrationBackups").join(ID);
        fs::write(saved.join("files/config.json"), b"tampered").unwrap();
        assert!(preserve(&data, ID).is_err());
        assert_eq!(fs::read(data.join("config.json")).unwrap(), b"original");
        assert!(preserve(&data, "../unsafe").is_err());
    }

    #[test]
    fn unavailable_backup_destination_preserves_source_without_publishing() {
        let root = tempfile::tempdir().unwrap();
        let data = root.path().join("Velo");
        fs::create_dir(&data).unwrap();
        fs::write(data.join("config.json"), b"source bytes").unwrap();
        fs::write(
            root.path().join("VeloRustMigrationBackups"),
            b"not a directory",
        )
        .unwrap();
        assert!(preserve(&data, ID).is_err());
        assert_eq!(fs::read(data.join("config.json")).unwrap(), b"source bytes");
        assert_eq!(fs::read_dir(&data).unwrap().count(), 1);
    }
}
