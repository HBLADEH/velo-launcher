//! Offline cross-language check. Explicit fixture directories only; no UI,
//! scan, Shell actions, hotkeys, or product data access.

use velo_slint::platform::windows::instance;
#[path = "../tests/support/mod.rs"]
mod support;

fn main() -> support::TestResult {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        return Err("Expected check|emit|verify|verify-edited|rewrite FIXTURE [DIRECTORY]".into());
    }
    let fixture = support::Fixture::load(std::path::Path::new(&args[1]))?;
    fixture.check()?;
    if args[0] != "check" {
        if args.len() != 3 {
            return Err("explicit fixture directory required".into());
        }
        let identity = instance::Identity::resolve(
            Some(format!("rs08-contract-{}", std::process::id())),
            Some((&args[2]).into()),
        )?;
        let instance::Acquisition::Primary(_guard) = identity.acquire()? else {
            return Err("fixture directory already in use".into());
        };
        match args[0].as_str() {
            "emit" => fixture.emit(&identity.data_dir)?,
            "verify" => fixture.verify(&identity.data_dir, false)?,
            "verify-edited" => fixture.verify(&identity.data_dir, true)?,
            "rewrite" => fixture.rewrite(&identity.data_dir)?,
            _ => return Err("unknown command".into()),
        }
    }
    println!(
        "RS-08 Rust data contract passed ({} ID / {} config cases)",
        fixture.identities.len(),
        fixture.configs.len()
    );
    Ok(())
}
