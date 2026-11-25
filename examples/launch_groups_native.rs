//! Actual Shell group dispatch uses only copies of our private helper. Every
//! write stays in a fresh output directory, independently of installed Velo.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use velo_slint::core::{
    config::Config,
    data::ApplicationLibrary,
    group_run::{Report, State},
    launch_groups::{Group, Member},
    library,
    service::{Event, Manage, Runtime},
    storage,
};
use velo_slint::platform::windows::shell;

fn wait<T>(
    runtime: &Runtime,
    mut find: impl FnMut(Event) -> Option<T>,
) -> Result<T, Box<dyn std::error::Error>> {
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        for event in runtime.drain() {
            if let Some(result) = find(event) {
                return Ok(result);
            }
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    Err("native group operation timed out".into())
}

fn completion(runtime: &Runtime) -> Result<Report, Box<dyn std::error::Error>> {
    wait(runtime, |event| match event {
        Event::GroupRun(report) if report.done => Some(*report),
        _ => None,
    })
}

fn marker(path: &Path) -> usize {
    std::fs::read_to_string(path.with_extension("started")).map_or(0, |text| text.lines().count())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let helper = PathBuf::from(args.next().ok_or("provide the private helper executable")?);
    if helper.file_name().and_then(|name| name.to_str()) != Some("launch_groups_helper.exe") {
        return Err("only the private helper is permitted".into());
    }
    let directory = PathBuf::from(args.next().ok_or("provide a fresh output directory")?);
    if args.next().is_some() {
        return Err("unexpected argument".into());
    }
    std::fs::create_dir(&directory)?;
    let directory = std::fs::canonicalize(directory)?;
    let data = directory.join("data");
    std::fs::create_dir(&data)?;
    let config = Config {
        scan_program_files: false,
        auto_check_updates: false,
        ..Config::default()
    };
    storage::write(&data.join("config.json"), &config)?;
    let paths: Vec<_> = ["game", "voice", "accelerator"]
        .map(|name| {
            directory.join(format!(
                "velo-group-native-{}-{name}.exe",
                std::process::id()
            ))
        })
        .into();
    for path in &paths {
        std::fs::copy(&helper, path)?;
    }
    let mut voice = std::process::Command::new(&paths[1]).spawn()?;
    let deadline = Instant::now() + Duration::from_secs(10);
    while marker(&paths[1]) != 1 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(15));
    }
    assert_eq!(marker(&paths[1]), 1);
    let apartment = shell::Apartment::new()?;
    let applications: Vec<_> = paths
        .iter()
        .map(|path| library::item(path, &mut shell::ShortcutResolver(&apartment)))
        .collect::<Result<_, _>>()?;
    let members = applications
        .iter()
        .map(Member::from_app)
        .collect::<Result<_, _>>()?;
    storage::write(
        &data.join("application-library.json"),
        &ApplicationLibrary {
            applications: Some(applications),
            ..ApplicationLibrary::empty()
        },
    )?;
    let group = Group {
        id: "group:native-test".into(),
        name: "合成开黑".into(),
        members,
        ..Group::default()
    };
    let mut runtime = Runtime::start(&data, || {})?;
    wait(&runtime, |event| match event {
        Event::Snapshot(snapshot) => Some(snapshot),
        _ => None,
    })?;
    assert!(runtime.manage(Manage::SaveGroup(group.clone())));
    let saved = wait(&runtime, |event| match event {
        Event::Managed {
            kind: "group_save",
            result,
        } => Some(result),
        _ => None,
    })?;
    saved?;
    assert!(runtime.launch_group(group.id.clone(), Vec::new(), None, 0));
    assert!(
        !runtime.launch_group(group.id.clone(), Vec::new(), None, 0),
        "a repeated trigger must not queue another batch"
    );
    let first = completion(&runtime)?;
    assert_eq!(
        first
            .entries
            .iter()
            .filter(|entry| entry.state == State::Submitted)
            .count(),
        2
    );
    assert_eq!(first.entries[1].state, State::AlreadyRunning);
    assert!(first.history_saved);
    let history_path = data.join("history.json");
    let after_first = storage::read::<velo_slint::core::data::History>(&history_path)?;
    assert_eq!(after_first.0[&group.id].launch_count, 1);
    assert!(runtime.launch_group(group.id.clone(), Vec::new(), None, 0));
    let second = completion(&runtime)?;
    assert!(
        second
            .entries
            .iter()
            .all(|entry| matches!(entry.state, State::AlreadyRunning | State::Pending))
    );
    assert!(runtime.launch_group(
        group.id.clone(),
        vec![group.members[1].app_id.clone()],
        None,
        0,
    ));
    assert_eq!(
        completion(&runtime)?.entries[0].state,
        State::AlreadyRunning
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    while paths.iter().any(|path| marker(path) == 0) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(15));
    }
    assert_eq!(
        paths.iter().map(|path| marker(path)).collect::<Vec<_>>(),
        vec![1, 1, 1]
    );
    let history = storage::read::<velo_slint::core::data::History>(&history_path)?;
    assert_eq!(history.0.len(), 3);
    assert_eq!(history.0[&group.id].launch_count, 2);
    for member in [&group.members[0], &group.members[2]] {
        assert_eq!(history.0[&member.app_id].launch_count, 1);
    }
    assert!(!history.0.contains_key(&group.members[1].app_id));
    assert!(history.0.values().all(|entry| {
        entry
            .query_history
            .as_ref()
            .is_none_or(|queries| queries.is_empty())
    }));
    runtime.stop();
    voice.wait()?;
    // The other two helpers exit themselves. Await that finite lifetime before
    // reporting cleanup; do not kill unrelated or installed applications.
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let snapshot = velo_slint::platform::windows::processes::Snapshot::capture()?;
        if paths.iter().all(|path| {
            snapshot.presence(path)
                == velo_slint::platform::windows::processes::Presence::NotRunning
        }) {
            break;
        }
        if Instant::now() > deadline {
            return Err("private helpers did not exit".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let evidence = serde_json::json!({
        "synthetic": true,
        "first_submitted": 2,
        "first_skipped": 1,
        "second_submitted": 0,
        "marker_counts": [1, 1, 1],
        "history_counts": [1, 1],
        "group_history_count": 2,
        "member_retry_counted_as_group": false,
        "query_learning_empty": true,
        "helpers_exited": true
    });
    std::fs::write(
        directory.join("result.json"),
        serde_json::to_vec_pretty(&evidence)?,
    )?;
    println!(
        "event=launch_group_native passed=true synthetic=true submitted=2 skipped=1 duplicate_dispatch=0 helpers_exited=true"
    );
    Ok(())
}
