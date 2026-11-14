//! Emit the production updater for an actual NSIS package in a task directory.
//! This helper never launches the script, requests elevation or changes settings.
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use velo_slint::core::update;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if arguments.len() != 3 {
        return Err("usage: rs14_update_plan <target exe> <actual installer> <script path>".into());
    }
    let target = &arguments[0];
    let installer = &arguments[1];
    let script = &arguments[2];
    let allowed = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .canonicalize()?;
    for path in &arguments {
        if !path.is_absolute()
            || !path
                .parent()
                .ok_or("missing parent")?
                .canonicalize()?
                .starts_with(&allowed)
        {
            return Err("all update test paths must stay inside rust/target".into());
        }
    }
    let fixture = script.parent().ok_or("missing fixture directory")?;
    let hash = Sha256::digest(fs::read(installer)?)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let restart = vec![
        "--background".into(),
        "--diagnostics".into(),
        "--instance-id=rs11-installer".into(),
        format!("--data-dir=\"{}\"", fixture.join("data").display()),
    ];
    let contents = update::script(target, installer, false, &hash, 2_147_483_647, &restart);
    let mut file = fs::File::create(script)?;
    file.write_all(b"\xef\xbb\xbf")?;
    file.write_all(contents.as_bytes())?;
    file.sync_all()?;
    Ok(())
}
