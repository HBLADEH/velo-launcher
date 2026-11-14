#![windows_subsystem = "windows"]

//! Minimal launch/restart receipt for native integration tests; never distributed.
use std::{env, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = env::args_os().skip(1);
    if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--capture")) {
        return Err("capture mode required".into());
    }
    let output = PathBuf::from(arguments.next().ok_or("capture path required")?);
    let executable = env::current_exe()?;
    let parent = output
        .parent()
        .ok_or("capture parent required")?
        .canonicalize()?;
    let executable_parent = executable
        .parent()
        .ok_or("executable parent required")?
        .canonicalize()?;
    // Native Shell tests keep the executable in apps/ and its receipt one
    // directory above; update tests keep both beside each other.
    if parent != executable_parent && Some(parent.as_path()) != executable_parent.parent() {
        return Err("capture must stay inside the synthetic test directory".into());
    }
    let arguments: Vec<_> = arguments
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    let staged = output.with_extension("partial");
    fs::write(
        &staged,
        serde_json::to_vec(
            &serde_json::json!({"arguments": arguments, "directory": env::current_dir()?}),
        )?,
    )?;
    fs::rename(staged, output)?;
    Ok(())
}
