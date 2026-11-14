mod support;

use std::fs::{self, OpenOptions};
use std::os::windows::fs::OpenOptionsExt;
use std::path::Path;
use std::sync::{Arc, mpsc};
use std::time::Duration;

use serde_json::{Value, json};
use tempfile::TempDir;
use velo_slint::core::JsonDocument;
use velo_slint::core::config::Config;
use velo_slint::core::data::{
    ApplicationLibrary, ConfigLoad, DataStore, History, HistoryEntry, IndexCache, IndexLoad,
};
use velo_slint::core::model::cached_icon_path;

fn fixture() -> support::Fixture {
    support::Fixture::load(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/compat/rs08.json"
    )))
    .unwrap()
}

fn seeded() -> (TempDir, std::path::PathBuf) {
    let parent = tempfile::tempdir().unwrap();
    let directory = parent.path().join("合成 数据");
    fixture().emit(&directory).unwrap();
    (parent, directory)
}

#[test]
fn shared_go_identity_defaults_validation_and_bidirectional_shapes() {
    let fixture = fixture();
    fixture.check().unwrap();
    let (_parent, directory) = seeded();
    fixture.verify(&directory, false).unwrap();
    fixture.rewrite(&directory).unwrap();
    fixture.verify(&directory, true).unwrap();
}

#[test]
fn reading_valid_old_config_never_rewrites_it_and_saving_preserves_unknown_fields() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.json");
    let original = "{\r\n  \"hotkey\": \"Ctrl+Alt+K\", \"space_launch\": false,\r\n  \"unknown_option\": {\"integer\":18446744073709551617,\"text\":\"保留\"},\r\n  \"search\": {\"fuzzy\":false,\"future_mode\":[null,1,\"x\"]}\r\n}\r\n";
    fs::write(&path, original).unwrap();
    Config::decode(original.as_bytes()).expect("synthetic old config must decode");
    for _ in 0..2 {
        let data = DataStore::open(directory.path()).unwrap();
        assert!(matches!(data.config_load, ConfigLoad::Existing));
        assert_eq!(fs::read(&path).unwrap(), original.as_bytes());
        assert!(!data.config.snapshot().unwrap().space_launch);
        assert_eq!(data.config.snapshot().unwrap().search.history_weight, 1.0);
    }
    let data = DataStore::open(directory.path()).unwrap();
    data.config
        .update(|current| {
            let mut next = current.clone();
            next.theme = "dark".into();
            Ok(next)
        })
        .unwrap();
    let saved: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let previous: Value = serde_json::from_str(original).unwrap();
    assert_eq!(saved["unknown_option"], previous["unknown_option"]);
    assert_eq!(
        saved["search"]["future_mode"],
        previous["search"]["future_mode"]
    );
    assert_eq!(
        DataStore::open(directory.path())
            .unwrap()
            .config
            .snapshot()
            .unwrap()
            .theme,
        "dark"
    );
}

#[test]
fn corrupted_config_is_preserved_before_recovery_but_future_config_is_untouched() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.json");
    fs::write(&path, b"{broken\r\n").unwrap();
    let data = DataStore::open(directory.path()).unwrap();
    let ConfigLoad::Recovered { original } = data.config_load else {
        panic!("not recovered");
    };
    assert_eq!(fs::read(original).unwrap(), b"{broken\r\n");
    assert_eq!(*data.config.snapshot().unwrap(), Config::default());
    let entries = fs::read_dir(directory.path()).unwrap().count();
    for bytes in [
        br#"{"version":99,"unknown":true}"#.as_slice(),
        br#"{"version":99,"hotkey":3}"#,
        br#"{"version":18446744073709551617}"#,
    ] {
        fs::write(&path, bytes).unwrap();
        assert!(DataStore::open(directory.path()).is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), entries);
    }
}

#[test]
fn failed_recovery_keeps_the_original_path_and_its_preserved_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.json");
    fs::write(&path, b"{broken").unwrap();
    // Deny replacement while still allowing reads, as another Windows process
    // or a filesystem policy can do. The original file must remain usable.
    let _held = OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(&path)
        .unwrap();
    assert!(DataStore::open(directory.path()).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"{broken");
    let backups: Vec<_> = fs::read_dir(directory.path())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("config.json.invalid-")
        })
        .collect();
    assert_eq!(backups.len(), 1);
    assert_eq!(fs::read(&backups[0]).unwrap(), b"{broken");
}

#[test]
fn backups_copy_disk_bytes_are_unique_and_fail_without_changing_the_source() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.json");
    let original = b"{\r\n \"theme\":\"dark\", \"unknown\": null\r\n}\r\n";
    fs::write(&path, original).unwrap();
    let data = DataStore::open(directory.path()).unwrap();
    let backups = directory.path().join("备份");
    let mut names = std::collections::BTreeSet::new();
    for _ in 0..10 {
        let name = data.config.backup(&backups).unwrap();
        assert!(name.is_absolute());
        assert_eq!(name.extension().unwrap(), "json");
        assert_eq!(fs::read(&name).unwrap(), original);
        assert!(names.insert(name));
    }
    assert_eq!(fs::read_dir(&backups).unwrap().count(), 10);
    let blocked = directory.path().join("blocked");
    fs::write(&blocked, b"file, not a directory").unwrap();
    assert!(data.config.backup(&blocked).is_err());
    assert_eq!(fs::read(&path).unwrap(), original);
    fs::remove_file(&path).unwrap();
    assert!(
        data.config
            .backup(&directory.path().join("not-created"))
            .is_err()
    );
    assert!(!directory.path().join("not-created").exists());
}

#[test]
fn replacement_failure_keeps_config_library_and_history_snapshots() {
    let (_parent, directory) = seeded();
    let data = DataStore::open(&directory).unwrap();
    let config = data.config.snapshot().unwrap();
    let library = data.library.snapshot().unwrap();
    let history = data.history.snapshot().unwrap();
    for name in ["config.json", "application-library.json", "history.json"] {
        let path = directory.join(name);
        let bytes = fs::read(&path).unwrap();
        let held = OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&path)
            .unwrap();
        let failed = match name {
            "config.json" => data
                .config
                .update(|current| {
                    let mut next = current.clone();
                    next.theme = "light".into();
                    Ok(next)
                })
                .is_err(),
            "application-library.json" => data
                .library
                .update(|current| {
                    let mut next = current.clone();
                    next.applications = Some(vec![]);
                    next.pinned = Some(vec![]);
                    Ok(next)
                })
                .is_err(),
            _ => data.history.update(|_| Ok(History::default())).is_err(),
        };
        assert!(failed);
        assert_eq!(fs::read(&path).unwrap(), bytes);
        drop(held);
    }
    assert!(Arc::ptr_eq(&data.config.snapshot().unwrap(), &config));
    assert!(Arc::ptr_eq(&data.library.snapshot().unwrap(), &library));
    assert!(Arc::ptr_eq(&data.history.snapshot().unwrap(), &history));
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 4);
}

#[test]
fn read_only_config_can_be_read_and_backed_up_but_not_committed() {
    let (_parent, directory) = seeded();
    let path = directory.join("config.json");
    let original = fs::read(&path).unwrap();
    let permissions = fs::metadata(&path).unwrap().permissions();
    let mut read_only = permissions.clone();
    read_only.set_readonly(true);
    fs::set_permissions(&path, read_only).unwrap();
    let data = DataStore::open(&directory).unwrap();
    let result = data.config.update(|current| {
        let mut next = current.clone();
        next.theme = "light".into();
        Ok(next)
    });
    let backup = data.config.backup(&directory.join("backups"));
    fs::set_permissions(&path, permissions).unwrap();
    assert!(result.is_err());
    assert_eq!(fs::read(&path).unwrap(), original);
    assert_eq!(data.config.snapshot().unwrap().theme, "dark");
    assert_eq!(fs::read(backup.unwrap()).unwrap(), original);
}

#[test]
fn serialized_updates_do_not_lose_changes_or_block_snapshot_readers() {
    let directory = tempfile::tempdir().unwrap();
    let data = Arc::new(DataStore::open(directory.path()).unwrap());
    assert!(matches!(data.config_load, ConfigLoad::Created));
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let writer_data = Arc::clone(&data);
    let writer = std::thread::spawn(move || {
        writer_data
            .history
            .update(|current| {
                entered_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                let mut next = current.clone();
                next.0.entry("synthetic".into()).or_default().launch_count += 1;
                Ok(next)
            })
            .unwrap()
    });
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let reader_data = Arc::clone(&data);
    let (read_tx, read_rx) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        read_tx
            .send(reader_data.history.snapshot().unwrap())
            .unwrap()
    });
    let read_result = read_rx.recv_timeout(Duration::from_secs(2));
    release_tx.send(()).unwrap();
    writer.join().unwrap();
    reader.join().unwrap();
    assert!(
        read_result.unwrap().0.is_empty(),
        "uncommitted state became visible"
    );
    let writers: Vec<_> = (0..20)
        .map(|_| {
            let data = Arc::clone(&data);
            std::thread::spawn(move || {
                data.history
                    .update(|current| {
                        let mut next = current.clone();
                        next.0.entry("synthetic".into()).or_default().launch_count += 1;
                        Ok(next)
                    })
                    .unwrap()
            })
        })
        .collect();
    for writer in writers {
        writer.join().unwrap();
    }
    assert_eq!(
        data.history.snapshot().unwrap().0["synthetic"].launch_count,
        21
    );
    assert_eq!(
        DataStore::open(directory.path())
            .unwrap()
            .history
            .snapshot()
            .unwrap()
            .0["synthetic"]
            .launch_count,
        21
    );
}

#[test]
fn invalid_config_updates_never_reach_disk_or_replace_the_committed_snapshot() {
    let (_parent, directory) = seeded();
    let data = DataStore::open(&directory).unwrap();
    let before = data.config.snapshot().unwrap();
    let bytes = fs::read(directory.join("config.json")).unwrap();
    for case in 0..3 {
        assert!(
            data.config
                .update(|current| {
                    let mut next = current.clone();
                    match case {
                        0 => next.version = 99,
                        1 => next.max_results = 0,
                        _ => next.search.history_weight = f64::NAN,
                    }
                    Ok(next)
                })
                .is_err()
        );
        assert_eq!(fs::read(directory.join("config.json")).unwrap(), bytes);
        assert!(Arc::ptr_eq(&before, &data.config.snapshot().unwrap()));
    }
}

#[test]
fn legacy_migrates_once_and_deleting_from_the_current_library_never_resurrects_it() {
    let fixture = fixture();
    let directory = tempfile::tempdir().unwrap();
    let legacy_path = directory.path().join("quick-launch.json");
    let original = serde_json::to_vec(&fixture.legacy).unwrap();
    fs::write(&legacy_path, &original).unwrap();
    let data = DataStore::open(directory.path()).unwrap();
    assert_eq!(
        *data.library.snapshot().unwrap(),
        support::decode::<ApplicationLibrary>(&fixture.expected_legacy).unwrap()
    );
    data.library
        .update(|_| Ok(ApplicationLibrary::empty()))
        .unwrap();
    let reopened = DataStore::open(directory.path()).unwrap();
    assert_eq!(
        *reopened.library.snapshot().unwrap(),
        ApplicationLibrary::empty()
    );
    assert_eq!(fs::read(&legacy_path).unwrap(), original);
}

#[test]
fn corrupt_or_future_user_documents_are_retained_and_never_replaced_with_empty_data() {
    for (name, bytes) in [
        ("history.json", "{broken"),
        ("history.json", r#"{"id":{"last_launch_time":"invalid"}}"#),
        ("application-library.json", "{broken"),
        ("application-library.json", r#"{"version":99}"#),
        ("quick-launch.json", "{broken"),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(name);
        fs::write(&path, bytes).unwrap();
        assert!(DataStore::open(directory.path()).is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes.as_bytes());
        if name == "quick-launch.json" {
            assert!(!directory.path().join("application-library.json").exists());
        }
    }
}

#[test]
fn invalid_index_is_retained_as_a_rebuild_request_without_replacing_user_documents() {
    let (_parent, directory) = seeded();
    for bytes in ["{broken", r#"{"version":99,"apps":[]}"#] {
        let path = directory.join("index.json");
        fs::write(&path, bytes).unwrap();
        let data = DataStore::open(&directory).unwrap();
        assert!(matches!(data.index, IndexLoad::RebuildRequired(_)));
        assert!(
            !data
                .library
                .snapshot()
                .unwrap()
                .applications
                .as_ref()
                .unwrap()
                .is_empty()
        );
        assert_eq!(fs::read(&path).unwrap(), bytes.as_bytes());
    }
}

#[test]
fn null_empty_nanosecond_timestamps_and_unknown_fields_survive_model_round_trips() {
    for applications in [Value::Null, json!([])] {
        let input =
            json!({"version":1,"applications":applications,"pinned":[],"future":{"keep":true}});
        let library: ApplicationLibrary = support::decode(&input).unwrap();
        assert_eq!(serde_json::to_value(library).unwrap(), input);
    }
    let input = json!({"id":{"launch_count":7,"last_launch_time":"2026-11-03T04:05:06.123456789+08:00","query_history":null,"future":[1,null]}});
    let history: History = support::decode(&input).unwrap();
    assert_eq!(serde_json::to_value(history).unwrap(), input);
    assert!(History::decode(b"null").unwrap().0.is_empty());
    assert_eq!(
        History::decode(br#"{"id":null}"#).unwrap().0["id"],
        HistoryEntry::default()
    );
    let cache: IndexCache =
        support::decode(&json!({"version":1,"files":null,"apps":null})).unwrap();
    assert!(cache.files.is_none());
    assert!(cache.apps.is_none());
}

#[test]
fn nested_app_and_index_unknown_fields_and_original_ids_are_retained() {
    let mut input = fixture().bundle["library"].clone();
    input["applications"][0]["future"] = json!({"keep":true});
    input["pinned"][0]["future"] = json!([null, "keep"]);
    let library: ApplicationLibrary = support::decode(&input).unwrap();
    assert_eq!(serde_json::to_value(library).unwrap(), input);
    let mut input = fixture().bundle["index"].clone();
    let file = input["files"]
        .as_object_mut()
        .unwrap()
        .values_mut()
        .next()
        .unwrap();
    file["future_stamp"] = json!(18446744073709551615u64);
    file["item"]["future"] = json!(true);
    let cache: IndexCache = support::decode(&input).unwrap();
    assert_eq!(serde_json::to_value(cache).unwrap(), input);
}

#[test]
fn cached_icon_urls_map_to_existing_layout_and_reject_path_traversal() {
    let directory = Path::new(r"C:\synthetic-data");
    assert_eq!(
        cached_icon_path(directory, "/icons/0123456789abcdef0123456789abcdef.png"),
        Some(directory.join("cache/icons/0123456789abcdef0123456789abcdef.png"))
    );
    for url in [
        "/icons/../config.json",
        "/icons/../../a.png",
        "/icons/C:\\a.png",
        "https://fixture/a.png",
        "/icons/ABCDEF0123456789abcdef0123456789.png",
        "/icons/a.png",
    ] {
        assert!(cached_icon_path(directory, url).is_none());
    }
}
