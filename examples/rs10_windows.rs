#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Local native integration evidence; launches only an argument-capture fixture in a
//! disposable directory and records aggregate results without personal paths.
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use velo_slint::core::{
    config::Config,
    data::{DataStore, History, IndexCache},
    icons, indexer,
    launch::Action,
    model::AppItem,
    service::{Event, Runtime, Snapshot},
    storage,
};
use velo_slint::platform::windows::{
    icons as native_icons,
    instance::{Acquisition, Identity},
    shell,
};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, IPersistFile};
use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
use windows::core::{Interface, PCWSTR};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> Result {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).is_some_and(|arg| arg == "--prepare-ui") {
        return prepare_ui(Path::new(
            args.get(2).ok_or("missing UI fixture directory")?,
        ));
    }
    let output = PathBuf::from(args.get(1).ok_or("usage: rs10_windows <new report path>")?);
    if output.exists() {
        return Err("report already exists".into());
    }
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let directory = tempfile::Builder::new()
        .prefix("velo-rs10-native-")
        .tempdir()?;
    let apps = directory.path().join("applications");
    let data = directory.path().join("data");
    let workspace = directory.path().join("工作 空格");
    fs::create_dir_all(&apps)?;
    fs::create_dir_all(&workspace)?;
    let executable = apps.join("RS10 Helper.exe");
    fs::copy(capture_helper()?, &executable)?;
    let marker = directory.path().join("launch.json");
    let arguments = format!("--capture \"{}\" --profile 中文", marker.display());
    let shortcut = apps.join("中文 启动.lnk");
    let apartment = shell::Apartment::new()?;
    // Imports canonicalize their paths before persistence. Both spellings must
    // yield the same actual Shell icon while their stored strings stay intact.
    let icon_item = AppItem {
        path: executable.to_string_lossy().into_owned(),
        exec_path: executable.to_string_lossy().into_owned(),
        ..Default::default()
    };
    let ordinary_icon = native_icons::extract(&apartment, &icon_item)?;
    let canonical = executable.canonicalize()?.to_string_lossy().into_owned();
    let extended_item = AppItem {
        path: canonical.clone(),
        exec_path: canonical,
        ..Default::default()
    };
    let extended_icon = native_icons::extract(&apartment, &extended_item)?;
    assert_eq!(
        ordinary_icon.rgba, extended_icon.rgba,
        "canonical import lost its Shell icon"
    );
    shortcut_file(&apartment, &shortcut, &executable, &arguments, &workspace)?;
    let ticket = indexer::Generation::default().ticket();
    let (mut cache, _) = indexer::scan(
        &[indexer::Root {
            path: apps.clone(),
            source: "Custom".into(),
            max_depth: 0,
        }],
        &IndexCache::default(),
        &mut shell::ShortcutResolver(&apartment),
        &ticket,
    )?;
    icons::assign(cache.apps.as_mut().ok_or("no applications")?, &ticket)?;
    let items = cache.apps.as_ref().ok_or("missing applications")?;
    assert_eq!(
        items.len(),
        2,
        "synthetic executable and shortcut must be indexed"
    );
    let link = items
        .iter()
        .find(|item| item.path == shortcut.to_string_lossy())
        .ok_or("missing synthetic shortcut")?;
    assert_eq!(link.name, "中文 启动");
    assert_eq!(link.exec_path, executable.to_string_lossy());
    assert_eq!(link.arguments, arguments);
    assert_eq!(link.working_directory, workspace.to_string_lossy());
    assert_eq!(link.description, "RS-10 合成快捷方式");
    assert_eq!(link.source, "Custom");
    let (mut unchanged, stats) = indexer::scan(
        &[indexer::Root {
            path: apps.clone(),
            source: "Custom".into(),
            max_depth: 0,
        }],
        &cache,
        &mut shell::ShortcutResolver(&apartment),
        &ticket,
    )?;
    icons::assign(
        unchanged
            .apps
            .as_mut()
            .ok_or("missing cached applications")?,
        &ticket,
    )?;
    assert_eq!(unchanged, cache);
    assert_eq!(stats.reused, 2);
    assert_eq!(stats.resolved, 0);
    let roots = shell::roots(&Config {
        scan_program_files: false,
        ..Config::default()
    })?;
    assert_eq!(roots.len(), 4);
    assert_eq!(
        roots
            .iter()
            .map(|root| root.source.as_str())
            .collect::<Vec<_>>(),
        ["Start Menu", "Start Menu", "Desktop", "Desktop"]
    );
    assert!(
        roots
            .iter()
            .all(|root| root.path.is_absolute() && root.max_depth == 0)
    );
    let packaged = shell::packaged_apps(&apartment, &ticket)?;
    let mut ids = std::collections::BTreeSet::new();
    for app in &packaged {
        assert_eq!(app.source, "Windows Apps");
        assert!(app.path.starts_with("shell:AppsFolder\\") && app.path.contains('!'));
        assert!(!app.name.is_empty() && ids.insert(&app.id));
        assert_eq!(app.exec_path, app.path);
        assert_eq!(app.id, velo_slint::core::model::identity(&app.path, ""));
    }
    let mut packaged_icons = 0;
    for app in packaged.iter().take(3) {
        if native_icons::extract(&apartment, app).is_ok() {
            packaged_icons += 1;
        }
    }
    let cached_app = cache
        .apps
        .as_ref()
        .ok_or("missing cache")?
        .iter()
        .find(|item| item.path == shortcut.to_string_lossy())
        .ok_or("shortcut not indexed")?
        .clone();
    icons::load(&apps, &cached_app, &ticket, |item| {
        native_icons::extract(&apartment, item)
    })?;
    icons::load(&apps, &cached_app, &ticket, |_| {
        panic!("existing PNG should be reused")
    })?;
    let app_id = cached_app.id.clone();
    storage::write(
        &data.join("config.json"),
        &Config {
            scan_program_files: false,
            custom_directories: Some(vec![apps.to_string_lossy().into_owned()]),
            ..Config::default()
        },
    )?;
    let identity = Identity::resolve(
        Some(format!("rs10-native-{}", std::process::id())),
        Some(data.clone()),
    )?;
    let Acquisition::Primary(_guard) = identity.acquire()? else {
        return Err("integration identity already running".into());
    };
    let mut runtime = Runtime::start(&identity.data_dir, || {})?;
    let snapshot = await_snapshot(&runtime, &app_id)?;
    runtime.icons(vec![
        snapshot
            .catalog
            .item(&app_id)
            .ok_or("lost shortcut")?
            .as_ref()
            .clone(),
    ]);
    await_event(
        &runtime,
        |event| matches!(event, Event::Icons { items, .. } if !items.is_empty()),
    )?;
    assert!(runtime.launch(app_id.clone(), "中文".into(), Action::Launch, 0));
    assert!(!runtime.launch(app_id.clone(), "duplicate".into(), Action::Launch, 0));
    await_event(&runtime, |event| {
        matches!(
            event,
            Event::Launched {
                result: Ok(()),
                history_saved: true
            }
        )
    })?;
    let started = Instant::now();
    while !marker.exists() && started.elapsed() < Duration::from_secs(10) {
        thread::sleep(Duration::from_millis(10));
    }
    let captured: serde_json::Value = serde_json::from_slice(&fs::read(&marker)?)?;
    assert_eq!(
        captured["arguments"],
        serde_json::json!(["--profile", "中文"])
    );
    assert_eq!(
        PathBuf::from(
            captured["directory"]
                .as_str()
                .ok_or("missing working directory")?
        ),
        workspace
    );
    let history: History = storage::read(&identity.data_dir.join("history.json"))?;
    assert_eq!(history.0[&app_id].launch_count, 1);
    // A removed .exe fails before Shell dispatch and does not learn history.
    let raw_id = snapshot
        .catalog
        .search(
            "RS10 Helper",
            &snapshot.config,
            &snapshot.history,
            velo_slint::core::service::now(),
        )?
        .iter()
        .find(|result| result.item.path == executable.to_string_lossy())
        .ok_or("raw executable missing")?
        .item
        .id
        .clone();
    fs::rename(&executable, apps.join("helper.saved"))?;
    assert!(runtime.launch(raw_id.clone(), "failed".into(), Action::Launch, 0));
    await_event(&runtime, |event| {
        matches!(event, Event::Launched { result: Err(_), .. })
    })?;
    let history_after: History = storage::read(&identity.data_dir.join("history.json"))?;
    assert_eq!(history_after, history);
    fs::rename(apps.join("helper.saved"), &executable)?;
    for _ in 0..20 {
        runtime.refresh();
    }
    await_snapshot(&runtime, &app_id)?;
    runtime.stop();
    let reopened = DataStore::open(&identity.data_dir)?;
    assert_eq!(*reopened.history.snapshot()?, history);
    let mut restarted = Runtime::start(&identity.data_dir, || {})?;
    await_snapshot(&restarted, &app_id)?;
    restarted.stop();
    let report = serde_json::json!({
        "schema_version": 2, "synthetic_shortcut_fields": true, "incremental_scan_reused": true, "known_folder_roots_valid": true,
        "packaged_apps_valid": true, "packaged_count": packaged.len(), "packaged_icons_tested": packaged.iter().take(3).count(), "packaged_icons_loaded": packaged_icons,
        "png_cache_reused": true, "visible_icon_loaded": true, "shortcut_arguments_and_working_directory": true,
        "duplicate_launch_rejected": true, "success_history_persisted": true, "failed_launch_did_not_learn": true,
        "coalesced_refresh_requests": 20, "restart_cache_and_history": true, "isolated_data_and_instance": true,
        "scope": "Windows 11 x64 local native APIs; no UAC desktop, IME or performance claim"
    });
    fs::write(&output, serde_json::to_vec_pretty(&report)?)?;
    Ok(())
}

fn shortcut_file(
    _apartment: &shell::Apartment,
    path: &Path,
    target: &Path,
    arguments: &str,
    workspace: &Path,
) -> Result {
    // SAFETY: STA is on this thread; buffers and interfaces stay live until
    // Save returns. Both callers point only to their synthetic helper.
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        link.SetPath(PCWSTR(shell::wide(&target.to_string_lossy())?.as_ptr()))?;
        link.SetArguments(PCWSTR(shell::wide(arguments)?.as_ptr()))?;
        link.SetWorkingDirectory(PCWSTR(shell::wide(&workspace.to_string_lossy())?.as_ptr()))?;
        link.SetDescription(PCWSTR(shell::wide("RS-10 合成快捷方式")?.as_ptr()))?;
        link.SetIconLocation(PCWSTR(shell::wide(&target.to_string_lossy())?.as_ptr()), 0)?;
        let file: IPersistFile = link.cast()?;
        file.Save(PCWSTR(shell::wide(&path.to_string_lossy())?.as_ptr()), true)?;
    }
    Ok(())
}

fn prepare_ui(directory: &Path) -> Result {
    fs::create_dir(directory)?;
    let apps = directory.join("apps");
    let data = directory.join("data");
    fs::create_dir(&apps)?;
    fs::create_dir(&data)?;
    let executable = apps.join("RS10 Helper.exe");
    fs::copy(capture_helper()?, &executable)?;
    let apartment = shell::Apartment::new()?;
    let arguments = format!(
        "--capture \"{}\" --profile UI",
        directory.join("capture.json").display()
    );
    shortcut_file(
        &apartment,
        &apps.join("RS10 启动验证.lnk"),
        &executable,
        &arguments,
        directory,
    )?;
    storage::write(
        &data.join("config.json"),
        &Config {
            scan_program_files: false,
            custom_directories: Some(vec![apps.to_string_lossy().into_owned()]),
            space_launch: false,
            auto_check_updates: false,
            ..Config::default()
        },
    )?;
    Ok(())
}

fn capture_helper() -> Result<PathBuf> {
    let current = std::env::current_exe()?;
    let helper = current
        .parent()
        .ok_or("missing native example directory")?
        .join("rs11_update_capture.exe");
    if !helper.is_file() {
        return Err("build the rs11_update_capture example before native integration tests".into());
    }
    Ok(helper)
}

fn await_snapshot(runtime: &Runtime, id: &str) -> Result<Arc<Snapshot>> {
    let started = Instant::now();
    let mut snapshot = None;
    let mut finished = false;
    loop {
        for event in runtime.drain() {
            match event {
                Event::Snapshot(current) => snapshot = Some(current),
                Event::Status { scanning, .. } => finished = !scanning,
                _ => {}
            }
        }
        if finished
            && let Some(snapshot) = &snapshot
            && snapshot.catalog.item(id).is_some()
        {
            return Ok(snapshot.clone());
        }
        if started.elapsed() > Duration::from_secs(90) {
            return Err("native scan timeout".into());
        }
        thread::sleep(Duration::from_millis(10));
    }
}
fn await_event(runtime: &Runtime, predicate: impl Fn(&Event) -> bool) -> Result {
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(30) {
        if runtime.drain().iter().any(&predicate) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(10));
    }
    Err("native integration event timeout".into())
}
