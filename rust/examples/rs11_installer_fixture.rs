#![windows_subsystem = "windows"]

//! Local installer branch fixture, never a distributable installer. Every write
//! is constrained to its adjacent plan's isolated directory under rust/target.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use velo_slint::core::{config::Config, data::DataStore, storage, update};
use velo_slint::platform::windows::{interaction, updater};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
#[derive(Serialize, Deserialize)]
struct Plan {
    root: PathBuf,
    target: PathBuf,
    payload_sha256: String,
    marker: PathBuf,
    fail: bool,
}

fn digest(path: &Path) -> Result<String> {
    Ok(Sha256::digest(fs::read(path)?)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn main() -> Result {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|s| s == "--prepare") {
        let root = PathBuf::from(args.get(1).ok_or("isolated root required")?);
        let payload = PathBuf::from(args.get(2).ok_or("Rust payload required")?).canonicalize()?;
        if root.exists() || !root.is_absolute() {
            return Err("fixture requires a fresh absolute directory".into());
        }
        let parent = root.parent().ok_or("missing parent")?.canonicalize()?;
        let allowed = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .canonicalize()?;
        if !parent.starts_with(allowed) {
            return Err("fixture must stay under rust/target".into());
        }
        fs::create_dir(&root)?;
        let root = root.canonicalize()?;
        let package = root.join("package");
        fs::create_dir(&package)?;
        let target = root.join("受保护 安装目录");
        fs::create_dir(&target)?;
        fs::copy(&payload, target.join("velo-slint.exe"))?;
        fs::copy(&payload, package.join("payload.exe"))?;
        let installer = package.join("velo-slint-x64-installer.exe");
        fs::copy(std::env::current_exe()?, &installer)?;
        let plan = Plan {
            root: root.clone(),
            target: target.clone(),
            payload_sha256: digest(&payload)?,
            marker: root.join("installer-result.json"),
            fail: false,
        };
        fs::write(package.join("plan.json"), serde_json::to_vec_pretty(&plan)?)?;
        fs::create_dir(root.join("data"))?;
        storage::write(
            &root.join("data/config.json"),
            &Config {
                hotkey: "Ctrl+Alt+L".into(),
                launch_at_startup: false,
                auto_check_updates: false,
                scan_program_files: false,
                space_launch: false,
                ..Config::default()
            },
        )?;
        DataStore::open(&root.join("data"))?
            .config
            .backup(&root.join("data/backups"))?;
        let arguments = vec![
            "--background".into(),
            "--diagnostics".into(),
            "--instance-id=rs11-installer".into(),
            format!("--data-dir=\"{}\"", root.join("data").display()),
        ];
        let script = update::script(
            &target.join("velo-slint.exe"),
            &installer,
            false,
            &digest(&installer)?,
            2147483647,
            &arguments,
        );
        let mut file = fs::File::create(root.join("install-update.ps1"))?;
        file.write_all(b"\xef\xbb\xbf")?;
        file.write_all(script.as_bytes())?;
        file.sync_all()?;
        return Ok(());
    }
    if args.first().is_some_and(|s| s == "--schedule") {
        let root =
            PathBuf::from(args.get(1).ok_or("fixture directory required")?).canonicalize()?;
        updater::schedule(&root.join("install-update.ps1"))?;
        return Ok(());
    }
    let executable = std::env::current_exe()?;
    let package = executable.parent().ok_or("missing installer directory")?;
    let plan: Plan = serde_json::from_slice(&fs::read(package.join("plan.json"))?)?;
    let allowed = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .canonicalize()?;
    if !plan.root.starts_with(&allowed)
        || !plan.target.starts_with(&plan.root)
        || !plan.marker.starts_with(&plan.root)
    {
        return Err("fixture write boundary failed".into());
    }
    let expected = format!("/D={}", plan.target.display());
    // NSIS treats everything after the final /D= as the directory, including
    // spaces. Match its raw command-line convention in this local fixture.
    if args.first().is_none_or(|arg| arg != "/S")
        || args[1..].join(" ") != expected
        || !interaction::elevated()
    {
        return Err("fixture requires exact silent arguments and administrator token".into());
    }
    let payload = package.join("payload.exe");
    if digest(&payload)? != plan.payload_sha256 {
        return Err("fixture payload checksum failed".into());
    }
    if plan.fail {
        fs::write(
            &plan.marker,
            b"{\"elevated\":true,\"injected_failure\":true}",
        )?;
        return Err("injected installer failure".into());
    }
    fs::copy(&payload, plan.target.join("velo-slint.exe"))?;
    fs::OpenOptions::new()
        .write(true)
        .open(plan.target.join("velo-slint.exe"))?
        .sync_all()?;
    fs::write(
        &plan.marker,
        serde_json::to_vec_pretty(
            &serde_json::json!({"elevated":true,"arguments":args,"payload_sha256":plan.payload_sha256,"installed_sha256":digest(&plan.target.join("velo-slint.exe"))?}),
        )?,
    )?;
    Ok(())
}
