use chrono::{DateTime, Utc};
use std::collections::BTreeMap;
use std::fs;
use std::sync::{Arc, mpsc};
use std::time::Duration;
use velo_slint::core::catalog::Catalog;
use velo_slint::core::data::HistoryEntry;
use velo_slint::core::data::{DataStore, History};
use velo_slint::core::model::AppItem;
use velo_slint::core::{JsonDocument, UnknownFields};

fn now() -> chrono::DateTime<chrono::FixedOffset> {
    DateTime::parse_from_rfc3339("2026-11-03T04:05:06Z").unwrap()
}
fn item(id: &str) -> AppItem {
    AppItem {
        id: id.into(),
        name: id.into(),
        ..AppItem::default()
    }
}

#[test]
fn failed_history_and_pin_commits_keep_files_and_snapshots() {
    let directory = tempfile::tempdir().unwrap();
    let data = DataStore::open(directory.path()).unwrap();
    data.history
        .record_success("used", " CODE ", now())
        .unwrap();
    let apps = [item("used")];
    data.library.set_pinned(&apps, &[], "used", true).unwrap();
    for path in ["history.json", "application-library.json"] {
        let path = directory.path().join(path);
        let bytes = fs::read(&path).unwrap();
        let original_permissions = fs::metadata(&path).unwrap().permissions();
        let mut permissions = original_permissions.clone();
        permissions.set_readonly(true);
        fs::set_permissions(&path, permissions).unwrap();
        let history = data.history.snapshot().unwrap();
        let library = data.library.snapshot().unwrap();
        let failed = if path.ends_with("history.json") {
            data.history.record_success("used", "new", now()).is_err()
        } else {
            data.library.set_pinned(&apps, &[], "used", false).is_err()
        };
        fs::set_permissions(&path, original_permissions).unwrap();
        assert!(failed);
        if path.ends_with("history.json") {
            assert!(Arc::ptr_eq(&history, &data.history.snapshot().unwrap()));
        } else {
            assert!(Arc::ptr_eq(&library, &data.library.snapshot().unwrap()));
        }
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
}

#[test]
fn history_readers_score_last_commit_while_a_writer_is_blocked() {
    let directory = tempfile::tempdir().unwrap();
    let data = Arc::new(DataStore::open(directory.path()).unwrap());
    data.history.record_success("used", "c", now()).unwrap();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let worker_data = Arc::clone(&data);
    let writer = std::thread::spawn(move || {
        worker_data.history.update(|history| {
            entered_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            history.record("used", "c", now())
        })
    });
    entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let (result_tx, result_rx) = mpsc::channel();
    let reader_data = Arc::clone(&data);
    let reader = std::thread::spawn(move || {
        let snapshot = reader_data.history.snapshot().unwrap();
        result_tx
            .send(snapshot.scores("c", now().with_timezone(&Utc)).unwrap())
            .unwrap();
    });
    let result = result_rx.recv_timeout(Duration::from_secs(2));
    release_tx.send(()).unwrap();
    writer.join().unwrap().unwrap();
    reader.join().unwrap();
    assert_eq!(result.unwrap()["used"], 85.0);
    assert_eq!(data.history.snapshot().unwrap().0["used"].launch_count, 2);
}

#[test]
fn concurrent_history_and_pin_writers_preserve_all_updates_after_reopen() {
    let directory = tempfile::tempdir().unwrap();
    let data = Arc::new(DataStore::open(directory.path()).unwrap());
    let apps: Arc<Vec<_>> = Arc::new((0..20).map(|id| item(&format!("item-{id}"))).collect());
    let workers: Vec<_> = (0..20)
        .map(|id| {
            let data = Arc::clone(&data);
            let apps = Arc::clone(&apps);
            std::thread::spawn(move || {
                data.history.record_success("used", "c", now()).unwrap();
                data.library
                    .set_pinned(&apps, &[], &format!("item-{id}"), true)
                    .unwrap();
            })
        })
        .collect();
    for worker in workers {
        worker.join().unwrap();
    }
    let reopened = DataStore::open(directory.path()).unwrap();
    let history = reopened.history.snapshot().unwrap();
    assert_eq!(history.0["used"].launch_count, 20);
    assert_eq!(history.0["used"].query_history.as_ref().unwrap()["c"], 20);
    assert_eq!(
        reopened
            .library
            .snapshot()
            .unwrap()
            .pinned
            .as_ref()
            .unwrap()
            .len(),
        20
    );
}

#[test]
fn pin_limit_idempotence_and_unknown_fields_survive_learning() {
    let directory = tempfile::tempdir().unwrap();
    let data = DataStore::open(directory.path()).unwrap();
    let mut apps: Vec<_> = (0..201).map(|id| item(&format!("item-{id}"))).collect();
    apps[0].unknown = UnknownFields::from([("future".into(), serde_json::json!({"keep":true}))]);
    data.library
        .update(|library| {
            let mut library = library.clone();
            library
                .unknown
                .insert("future".into(), serde_json::json!(42));
            library.pinned = Some(apps[..200].to_vec());
            Ok(library)
        })
        .unwrap();
    assert!(
        data.library
            .set_pinned(&apps, &[], "item-200", true)
            .is_err()
    );
    data.library.set_pinned(&apps, &[], "item-0", true).unwrap();
    data.library
        .set_pinned(&apps, &[], "item-0", false)
        .unwrap();
    let library = data.library.set_pinned(&apps, &[], "item-0", true).unwrap();
    assert_eq!(library.unknown["future"], 42);
    assert_eq!(
        library.pinned.as_ref().unwrap().last().unwrap().unknown,
        apps[0].unknown
    );
    let history =
        History::decode(br#"{"id":{"future":42,"launch_count":1,"query_history":null}}"#).unwrap();
    assert_eq!(
        history.record("id", "C", now()).unwrap().0["id"].unknown["future"],
        42
    );
    assert!(Catalog::new(&apps, &library, &[]).item("missing").is_none());
}

#[test]
fn tied_eviction_keeps_current_query_and_integer_overflow_rolls_back() {
    let history = History(BTreeMap::from([(
        "used".into(),
        HistoryEntry {
            query_history: Some((0..100).map(|n| (format!("q{n:03}"), 1)).collect()),
            ..HistoryEntry::default()
        },
    )]));
    let next = history.record("used", "new", now()).unwrap();
    let queries = next.0["used"].query_history.as_ref().unwrap();
    assert_eq!(queries.len(), 100);
    assert_eq!(queries["new"], 1);
    assert!(!queries.contains_key("q000"));
    assert_eq!(history.0["used"].query_history.as_ref().unwrap().len(), 100);
    let directory = tempfile::tempdir().unwrap();
    let data = DataStore::open(directory.path()).unwrap();
    data.history
        .update(|_| {
            Ok(History(BTreeMap::from([(
                "used".into(),
                HistoryEntry {
                    launch_count: i64::MAX,
                    ..HistoryEntry::default()
                },
            )])))
        })
        .unwrap();
    let before = data.history.snapshot().unwrap();
    assert!(data.history.record_success("used", "c", now()).is_err());
    assert!(Arc::ptr_eq(&before, &data.history.snapshot().unwrap()));
}
