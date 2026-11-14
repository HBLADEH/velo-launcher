//! Executes update replacement/recovery only in a disposable directory, using
//! a minimal argument-capture helper. No installer or system action runs.
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use velo_slint::core::update;
use velo_slint::core::{config::Config, storage};
use velo_slint::platform::windows::{preferences, system_tools, updater};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn main() -> Result {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|a| a == "--prepare-copy") {
        let source = PathBuf::from(args.get(2).ok_or("source required")?);
        let data = PathBuf::from(args.get(3).ok_or("test data directory required")?);
        let apps = PathBuf::from(args.get(4).ok_or("test applications required")?);
        for name in [
            "config.json",
            "history.json",
            "application-library.json",
            "quick-launch.json",
            "index.json",
        ] {
            if source.join(name).is_file() {
                fs::copy(source.join(name), data.join(name))?;
            }
        }
        let mut config: Config = storage::read(&data.join("config.json"))?;
        config
            .custom_directories
            .get_or_insert_default()
            .push(apps.to_string_lossy().into_owned());
        config.scan_program_files = false;
        config.auto_check_updates = false;
        config.space_launch = false;
        storage::write(&data.join("config.json"), &config)?;
        return Ok(());
    }
    let helper = PathBuf::from(args.get(1).ok_or("capture helper required")?);
    let output = PathBuf::from(args.get(2).ok_or("new report path required")?);
    if output.exists() {
        return Err("report already exists".into());
    }
    let directory = tempfile::Builder::new()
        .prefix("velo-rs11-native-")
        .tempdir()?;
    let tools = system_tools::discover()?;
    assert!(tools.iter().all(|i| i.source == "System"));
    let mut windows = Vec::new();
    for (mode, correct) in [("replace", true), ("hash-failure-recovery", false)] {
        let root = directory.path().join(mode);
        fs::create_dir(&root)?;
        let target = root.join("中文 O'Brien target.exe");
        let source = root.join("downloaded.exe");
        let marker = root.join("capture.json");
        fs::copy(&helper, &source)?;
        if correct {
            fs::write(&target, b"old placeholder")?;
        } else {
            fs::copy(&helper, &target)?;
        }
        let before = fs::read(&target)?;
        let hash: String = if correct {
            Sha256::digest(fs::read(&source)?)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect()
        } else {
            "0".repeat(64)
        };
        let script = root.join("update.ps1");
        let arguments = vec![
            "--capture".into(),
            format!("\"{}\"", marker.display()),
            "--profile".into(),
            "RS11".into(),
        ];
        let content = update::script(&target, &source, true, &hash, 2147483647, &arguments);
        let mut file = fs::File::create(&script)?;
        file.write_all(b"\xef\xbb\xbf")?;
        file.write_all(content.as_bytes())?;
        file.sync_all()?;
        drop(file);
        // Normal business data directories are canonical verbatim Win32 paths.
        // Exercise that exact boundary, not just tempfile's DOS spelling.
        updater::schedule(&script.canonicalize()?)?;
        if let Err(error) = wait_for(&marker, Duration::from_secs(30)) {
            fs::write(
                output.with_extension("failure.txt"),
                fs::read(root.join("update.ps1.log")).unwrap_or_default(),
            )?;
            return Err(error);
        }
        let capture: serde_json::Value = serde_json::from_slice(&fs::read(&marker)?)?;
        assert_eq!(
            capture["arguments"],
            serde_json::json!(["--profile", "RS11"])
        );
        if correct {
            assert_eq!(fs::read(&target)?, fs::read(&helper)?);
            let deadline = Instant::now() + Duration::from_secs(5);
            while script.exists() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(20));
            }
            assert!(!source.exists() && !script.exists());
        } else {
            assert_eq!(fs::read(&target)?, before);
            assert!(source.exists());
            wait_for(&root.join("update.ps1.log"), Duration::from_secs(5))?;
        }
        windows.push(mode);
    }
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        output,
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":true,"system_tools":tools.len(),"portable_update":windows,"windows_directory_available":preferences::windows_directory().is_ok(),"installer_uac":"not executed","destructive_actions":"not executed"}),
        )?,
    )?;
    Ok(())
}
fn wait_for(path: &Path, timeout: Duration) -> Result {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if path.is_file() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    Err("update capture timed out".into())
}
