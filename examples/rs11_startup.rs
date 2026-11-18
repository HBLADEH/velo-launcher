//! Current-session background startup checks. No login item, keyboard injection,
//! logout or security-policy change. This is not a real login-session test.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use velo_slint::core::{config::Config, data::IndexCache, model::AppItem, storage};
use velo_slint::platform::windows::{
    hotkey::{Key, Service},
    instance::{self, Identity},
};
use windows_sys::Win32::Foundation::{HWND, LPARAM};
use windows_sys::Win32::UI::Shell::{NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

struct Running {
    child: Child,
    identity: Identity,
}
impl Drop for Running {
    fn drop(&mut self) {
        // SAFETY: remove only the notification icon owned by this child PID.
        // Then terminate the disposable test process; this is cleanup, not a
        // claim that the application followed its normal user-quit path.
        unsafe {
            let hwnd = FindWindowW(
                instance::wide(instance::WINDOW_CLASS).as_ptr(),
                instance::wide(&self.identity.window_title()).as_ptr(),
            );
            let mut pid = 0;
            if !hwnd.is_null()
                && GetWindowThreadProcessId(hwnd, &mut pid) != 0
                && pid == self.child.id()
            {
                let data = NOTIFYICONDATAW {
                    cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                    hWnd: hwnd,
                    uID: 1,
                    ..Default::default()
                };
                Shell_NotifyIconW(NIM_DELETE, &data);
            }
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct Visibility {
    pid: u32,
    visible: bool,
}
unsafe extern "system" fn window(hwnd: HWND, state: LPARAM) -> i32 {
    // SAFETY: EnumWindows is synchronous; state is a live exclusive stack value.
    let state = unsafe { &mut *(state as *mut Visibility) };
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(hwnd, &mut pid);
    }
    if pid == state.pid {
        let mut class = [0u16; 64];
        let length = unsafe { GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32) };
        if String::from_utf16_lossy(&class[..length.max(0) as usize]) == "VeloSlintLauncherWindow" {
            state.visible |= unsafe { IsWindowVisible(hwnd) } != 0;
        }
    }
    1
}
fn visible(pid: u32) -> bool {
    let mut state = Visibility {
        pid,
        visible: false,
    };
    // SAFETY: callback accesses state only during this synchronous enumeration.
    unsafe {
        EnumWindows(Some(window), (&mut state as *mut Visibility) as isize);
    }
    state.visible
}

fn run(
    executable: &Path,
    directory: &Path,
    saved: Key,
    expect_visible: bool,
    name: &str,
    invalid_config: bool,
) -> Result<serde_json::Value> {
    fs::create_dir(directory)?;
    let program = directory.join("程序 中文");
    fs::create_dir(&program)?;
    let copy = program.join("velo-slint.exe");
    fs::copy(executable, &copy)?;
    let data = directory.join("数据 空格");
    fs::create_dir(&data)?;
    storage::write(
        &data.join("config.json"),
        &Config {
            hotkey: saved.to_string(),
            auto_check_updates: false,
            scan_program_files: false,
            space_launch: false,
            ..Config::default()
        },
    )?;
    if invalid_config {
        fs::write(
            data.join("config.json"),
            br#"{"version":999,"preserve":"original future config"}"#,
        )?;
    }
    let original_config = fs::read(data.join("config.json"))?;
    // A realistic large cached index keeps business loading asynchronous long
    // enough to expose a provisional default chord being treated as final.
    let cache = IndexCache {
        version: 1,
        apps: Some(
            (0..10000)
                .map(|n| AppItem {
                    id: format!("startup-{n}"),
                    name: format!("Startup fixture {n}"),
                    path: format!(r"D:\Synthetic\Application{n}.exe"),
                    source: "Custom".into(),
                    ..AppItem::default()
                })
                .collect(),
        ),
        ..IndexCache::default()
    };
    storage::write(&data.join("index.json"), &cache)?;
    let identity = Identity::resolve(
        Some(format!("rs11-startup-{}-{name}", std::process::id())),
        Some(data),
    )?;
    let log = directory.join("startup.stderr.log");
    let child = Command::new(&copy)
        .arg("--background")
        .arg("--diagnostics")
        .arg(format!("--instance-id={}", identity.id))
        .arg(format!("--data-dir={}", identity.data_dir.display()))
        .stdout(Stdio::null())
        .stderr(fs::File::create(&log)?)
        .spawn()?;
    let mut running = Running { child, identity };
    let started = Instant::now();
    let mut ever_visible = false;
    loop {
        ever_visible |= visible(running.child.id());
        let text = fs::read_to_string(&log)?;
        let configured = text.contains("event=startup_configuration_ready")
            && text.contains("event=hotkey_bind")
            && text.contains("event=lifecycle_ready")
            && (text.contains("event=lifecycle_shown reason=startup")
                || text.contains("event=lifecycle_hidden reason=startup"));
        if configured || started.elapsed() > Duration::from_secs(20) {
            let final_visible = visible(running.child.id());
            let original_preserved =
                fs::read(running.identity.data_dir.join("config.json"))? == original_config;
            let registration = Service::start(|_| {})?;
            let saved_registered = registration.bind(saved).is_err();
            let passed = original_preserved
                && configured
                && final_visible == expect_visible
                && (expect_visible || (!ever_visible && saved_registered));
            let result = serde_json::json!({"case":name,"passed":passed,"configured":configured,"expected_visible":expect_visible,"visible":final_visible,"ever_visible":ever_visible,"saved_chord_registered_or_blocked":saved_registered,"startup_shown":text.contains("event=lifecycle_shown reason=startup"),"registered_chord_failures":text.matches("event=hotkey_bind success=false").count(),"original_config_preserved":original_preserved,"cached_items":10000,"scope":"current-session command startup; no real login or keyboard evidence"});
            drop(running);
            return Ok(result);
        }
        if running.child.try_wait()?.is_some() {
            return Err("background test process exited before readiness".into());
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn main() -> Result {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let executable = PathBuf::from(args.first().ok_or("executable required")?).canonicalize()?;
    let output = PathBuf::from(args.get(1).ok_or("fresh report directory required")?);
    if output.exists() || !output.is_absolute() {
        return Err("fresh absolute output directory required".into());
    }
    let allowed = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .canonicalize()?;
    if !output
        .parent()
        .ok_or("missing parent")?
        .canonicalize()?
        .starts_with(allowed)
    {
        return Err("report must stay inside target".into());
    }
    fs::create_dir(&output)?;
    let default = Key::parse("Alt+Space")?;
    let saved = Key::parse("Ctrl+Alt+Shift+F17")?;
    let default_blocker = Service::start(|_| {})?;
    let default_already_busy = default_blocker.bind(default).is_err();
    let first = run(
        &executable,
        &output.join("default-conflict"),
        saved,
        false,
        "default",
        false,
    )?;
    drop(default_blocker);
    let saved_blocker = Service::start(|_| {})?;
    saved_blocker.bind(saved)?;
    let second = run(
        &executable,
        &output.join("saved-conflict"),
        saved,
        true,
        "saved",
        false,
    )?;
    drop(saved_blocker);
    let third = run(
        &executable,
        &output.join("invalid-config"),
        saved,
        true,
        "invalid",
        true,
    )?;
    let passed = first["passed"] == true && second["passed"] == true && third["passed"] == true;
    fs::write(
        output.join("results.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"passed":passed,"default_already_busy":default_already_busy,"cases":[first,second,third],"login_values_created":false,"process_cleanup":"isolated PID terminated after removing owned tray icon"}),
        )?,
    )?;
    if !passed {
        return Err("background startup contract failed; see preserved report".into());
    }
    Ok(())
}
