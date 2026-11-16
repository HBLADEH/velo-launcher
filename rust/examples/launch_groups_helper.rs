//! Private native-test helper: no window, network, registry or user data.
#![windows_subsystem = "windows"]

use std::io::Write;

fn main() -> std::io::Result<()> {
    let marker = std::env::current_exe()?.with_extension("started");
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(marker)?;
    writeln!(file, "{}", std::process::id())?;
    file.sync_all()?;
    std::thread::sleep(std::time::Duration::from_secs(8));
    Ok(())
}
