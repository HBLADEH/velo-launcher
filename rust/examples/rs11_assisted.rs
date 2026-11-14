//! Read-only native identity evidence for user-assisted UAC checks. The helper
//! writes only its explicit synthetic marker and never executes another file.
use std::path::PathBuf;
use velo_slint::platform::windows::interaction;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if arguments.first().is_none_or(|arg| arg != "--capture") {
        return Err("usage: rs11_assisted --capture <synthetic marker> [profile arguments]".into());
    }
    let marker = PathBuf::from(arguments.get(1).ok_or("missing capture marker")?);
    if !marker.is_absolute() || marker.exists() {
        return Err("capture marker must be a fresh absolute path".into());
    }
    std::fs::write(
        marker,
        serde_json::to_vec_pretty(&serde_json::json!({
            "elevated": interaction::elevated(),
            "arguments": arguments[2..],
            "working_directory": std::env::current_dir()?,
            "executable": std::env::current_exe()?,
        }))?,
    )?;
    Ok(())
}
