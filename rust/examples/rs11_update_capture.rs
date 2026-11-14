#![windows_subsystem = "windows"]

//! Minimal restart receipt for the native update test; never distributed.
use std::{env, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = env::args_os().skip(1);
    if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--capture")) {
        return Err("capture mode required".into());
    }
    let output = PathBuf::from(arguments.next().ok_or("capture path required")?);
    let executable = env::current_exe()?;
    if output
        .parent()
        .ok_or("capture parent required")?
        .canonicalize()?
        != executable
            .parent()
            .ok_or("executable parent required")?
            .canonicalize()?
    {
        return Err("capture must stay beside the copied test executable".into());
    }
    let arguments: Vec<_> = arguments
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    fs::write(
        output,
        serde_json::to_vec(&serde_json::json!({"arguments": arguments}))?,
    )?;
    Ok(())
}
