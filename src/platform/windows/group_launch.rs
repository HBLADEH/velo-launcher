//! A group uses at most three temporary STA workers. The coordinator retains
//! the owner HWND until all synchronous Shell calls and workers have returned.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use super::{launch, processes, shell};
use crate::core::group_run::{Entry, Report, State};
use crate::core::launch::Action;
use crate::core::launch_groups::{Group, Member};
use crate::core::library;
use crate::core::model::{AppItem, identity};

pub(crate) type Pending = Mutex<BTreeMap<processes::FileIdentity, Instant>>;

pub(crate) struct Request {
    pub group: Group,
    pub items: Vec<Option<AppItem>>,
    pub run_id: u64,
    pub owner: isize,
    pub force: Option<String>,
}

pub(crate) fn run(
    request: Request,
    pending: &Pending,
    stopping: &AtomicBool,
    cancelled: &AtomicBool,
    progress: impl Fn(Report) + Sync,
) -> Report {
    coordinate(request, stopping, cancelled, progress, |request, index| {
        let apartment = shell::Apartment::new();
        if let (Ok(apartment), Some(item)) = (&apartment, &request.items[index]) {
            let member = &request.group.members[index];
            execute(
                member,
                item,
                apartment,
                request.owner,
                request.force.as_deref() == Some(&member.app_id),
                pending,
                stopping,
                cancelled,
            )
        } else {
            (State::Failed, "应用入口不可用，请重新选择应用")
        }
    })
}

fn coordinate(
    request: Request,
    stopping: &AtomicBool,
    cancelled: &AtomicBool,
    progress: impl Fn(Report) + Sync,
    execute: impl Fn(&Request, usize) -> (State, &'static str) + Sync,
) -> Report {
    let report = Mutex::new(Report {
        run_id: request.run_id,
        group_id: request.group.id.clone(),
        name: request.group.name.clone(),
        entries: request
            .group
            .members
            .iter()
            .map(|member| Entry {
                app_id: member.app_id.clone(),
                name: member.display_name.clone(),
                state: State::Queued,
                message: "",
            })
            .collect(),
        done: false,
        history_saved: true,
    });
    let next = AtomicUsize::new(0);
    // All progress carries every member; replacing an older progress message
    // never loses a completed item. Only the coordinator sends the final event.
    std::thread::scope(|scope| {
        let mut workers = Vec::with_capacity(3);
        for _ in 0..request.group.members.len().min(3) {
            let request = &request;
            let report = &report;
            let next = &next;
            let progress = &progress;
            let execute = &execute;
            match std::thread::Builder::new()
                .name("velo-group-shell".into())
                .spawn_scoped(scope, move || {
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some(_) = request.group.members.get(index) else {
                            break;
                        };
                        let outcome = if stopping.load(Ordering::Acquire)
                            || cancelled.load(Ordering::Acquire)
                        {
                            (State::Cancelled, "尚未启动的成员已取消")
                        } else {
                            update(report, index, State::Checking, "", progress);
                            execute(request, index)
                        };
                        update(report, index, outcome.0, outcome.1, progress);
                    }
                }) {
                Ok(worker) => workers.push(worker),
                Err(_) => break,
            }
        }
        // Join explicitly so failure to start a worker or a worker panic does
        // not strand the service's busy flag with an incomplete result.
        for worker in workers {
            if worker.join().is_err() {
                eprintln!("event=group_worker_failed");
            }
        }
    });
    let mut final_report = report
        .into_inner()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    for entry in &mut final_report.entries {
        if matches!(entry.state, State::Queued | State::Checking) {
            entry.state = State::Failed;
            entry.message = "启动任务未完成，请重新检查后重试";
        }
    }
    final_report.done = true;
    final_report
}

fn update(
    report: &Mutex<Report>,
    index: usize,
    state: State,
    message: &'static str,
    progress: &impl Fn(Report),
) {
    if let Ok(mut report) = report.lock() {
        report.entries[index].state = state;
        report.entries[index].message = message;
        // Serialize publication with mutation: an older worker cannot enqueue
        // an older complete snapshot after a newer worker has published one.
        progress(report.clone());
    }
}

#[allow(clippy::too_many_arguments)]
fn execute(
    member: &Member,
    item: &AppItem,
    apartment: &shell::Apartment,
    owner: isize,
    force: bool,
    pending: &Pending,
    stopping: &AtomicBool,
    cancelled: &AtomicBool,
) -> (State, &'static str) {
    let Ok(resolved) = library::item(
        std::path::Path::new(&item.path),
        &mut shell::ShortcutResolver(apartment),
    ) else {
        return (State::Failed, "应用已移动或无法读取，请重新选择");
    };
    // A shortcut can change after indexing. Do not check one program and then
    // execute a newly retargeted shortcut without an explicit user repair.
    let target = if item.exec_path.is_empty() {
        &item.path
    } else {
        &item.exec_path
    };
    let resolved_arguments = if crate::core::indexer::extension(&item.path, "lnk") {
        &resolved.arguments
    } else {
        &item.arguments
    };
    if identity(&resolved.exec_path, resolved_arguments) != identity(target, &item.arguments) {
        return (
            State::NeedsConfiguration,
            "快捷方式目标已变化，请重新选择应用",
        );
    }
    let Some(program) = member.running_program(item) else {
        return (State::NeedsConfiguration, "请选择实际运行的主程序");
    };
    let Ok(key) = processes::file_identity(&program) else {
        return (State::NeedsConfiguration, "运行程序不可读取，请重新选择");
    };
    let presence = processes::Snapshot::capture()
        .map_or(processes::Presence::Unknown, |snapshot| {
            snapshot.presence(&program)
        });
    let Ok(mut locks) = pending.lock() else {
        return (State::NeedsReview, "启动状态不可用，请重启 Velo 后重试");
    };
    if presence == processes::Presence::Running {
        locks.remove(&key);
        return (State::AlreadyRunning, "");
    }
    if let Some(started) = locks.get(&key) {
        if started.elapsed() < Duration::from_secs(15) {
            return (State::Pending, "请稍候，避免重复启动");
        }
        if !force {
            return (State::NeedsReview, "启动结果未确认，可重新检查或仍然打开");
        }
    }
    if presence == processes::Presence::Unknown && !force {
        return (State::NeedsReview, "无法确认运行状态，可重新检查或仍然打开");
    }
    if stopping.load(Ordering::Acquire) || cancelled.load(Ordering::Acquire) {
        return (State::Cancelled, "尚未启动的成员已取消");
    }
    if !locks.contains_key(&key) && locks.len() >= 1024 {
        return (State::NeedsReview, "待确认启动记录过多，请重启 Velo 后重试");
    }
    locks.insert(key, Instant::now());
    drop(locks);
    match launch::execute(apartment, owner, item, Action::Launch) {
        Ok(()) => (State::Submitted, ""),
        Err(error) => {
            if let Ok(mut locks) = pending.lock() {
                locks.remove(&key);
            }
            (State::Failed, error.message())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::launch_groups::{Detection, RunningRule};

    fn request(count: usize) -> Request {
        Request {
            group: Group {
                id: "group:test".into(),
                name: "合成组合".into(),
                members: (0..count)
                    .map(|i| Member {
                        app_id: format!("app-{i}"),
                        display_name: format!("应用 {i}"),
                        ..Member::default()
                    })
                    .collect(),
                ..Group::default()
            },
            items: vec![None; count],
            run_id: 7,
            owner: 0,
            force: None,
        }
    }

    #[test]
    fn bounded_parallel_jobs_keep_every_partial_failure_and_a_complete_final_snapshot() {
        let active = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        let seen = Mutex::new(Vec::new());
        let events = Mutex::new(Vec::new());
        let report = coordinate(
            request(20),
            &AtomicBool::new(false),
            &AtomicBool::new(false),
            |report| events.lock().unwrap().push(report),
            |_, index| {
                let current = active.fetch_add(1, Ordering::AcqRel) + 1;
                peak.fetch_max(current, Ordering::Relaxed);
                std::thread::sleep(Duration::from_millis(2));
                seen.lock().unwrap().push(index);
                active.fetch_sub(1, Ordering::AcqRel);
                if index == 1 {
                    (State::Failed, "synthetic failure")
                } else {
                    (State::Submitted, "")
                }
            },
        );
        assert_eq!(report.entries.len(), 20);
        assert!(report.done);
        assert_eq!(report.entries[1].state, State::Failed);
        assert_eq!(
            report
                .entries
                .iter()
                .filter(|entry| entry.state == State::Submitted)
                .count(),
            19
        );
        assert!(peak.load(Ordering::Relaxed) <= 3);
        let mut indices = seen.into_inner().unwrap();
        indices.sort();
        assert_eq!(indices, (0..20).collect::<Vec<_>>());
        let events = events.into_inner().unwrap();
        assert!(
            events
                .iter()
                .all(|report| report.entries.len() == 20 && !report.done)
        );
        assert!(
            events
                .last()
                .unwrap()
                .entries
                .iter()
                .all(|entry| !matches!(entry.state, State::Queued | State::Checking))
        );
        assert_eq!(report.retry_ids(), vec!["app-1"]);
    }

    #[test]
    fn cancellation_skips_queued_jobs_and_does_not_undo_already_dispatched_members() {
        let cancelled = AtomicBool::new(false);
        let report = coordinate(
            request(20),
            &AtomicBool::new(false),
            &cancelled,
            |_| {},
            |_, _| {
                cancelled.store(true, Ordering::Release);
                (State::Submitted, "")
            },
        );
        let submitted = report
            .entries
            .iter()
            .filter(|entry| entry.state == State::Submitted)
            .count();
        assert!((1..=3).contains(&submitted));
        assert_eq!(
            report
                .entries
                .iter()
                .filter(|entry| entry.state == State::Cancelled)
                .count(),
            20 - submitted
        );
        let report = coordinate(
            request(3),
            &AtomicBool::new(true),
            &AtomicBool::new(false),
            |_| {},
            |_, _| panic!("stopping must never dispatch"),
        );
        assert!(
            report
                .entries
                .iter()
                .all(|entry| entry.state == State::Cancelled)
        );
    }

    #[test]
    fn pending_dispatch_does_not_reopen_a_program_even_after_observation_timeout() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("synthetic-not-executable.exe");
        std::fs::write(&path, b"never execute this synthetic file").unwrap();
        let apartment = shell::Apartment::new().unwrap();
        let item = library::item(&path, &mut shell::ShortcutResolver(&apartment)).unwrap();
        let member = Member::from_app(&item).unwrap();
        let key = processes::file_identity(&path).unwrap();
        let pending = Mutex::new(BTreeMap::from([(key, Instant::now())]));
        let stopped = AtomicBool::new(false);
        assert_eq!(
            execute(
                &member, &item, &apartment, 0, false, &pending, &stopped, &stopped
            )
            .0,
            State::Pending
        );
        pending
            .lock()
            .unwrap()
            .insert(key, Instant::now() - Duration::from_secs(20));
        assert_eq!(
            execute(
                &member, &item, &apartment, 0, false, &pending, &stopped, &stopped
            )
            .0,
            State::NeedsReview
        );
    }

    #[test]
    fn a_running_host_does_not_prove_a_parameterized_entry_without_a_rule() {
        let executable = std::env::current_exe().unwrap();
        let apartment = shell::Apartment::new().unwrap();
        let mut item =
            library::item(&executable, &mut shell::ShortcutResolver(&apartment)).unwrap();
        item.arguments = "--synthetic-profile".into();
        item.id = identity(&item.exec_path, &item.arguments);
        let mut member = Member::from_app(&item).unwrap();
        let pending = Mutex::new(BTreeMap::new());
        let stopped = AtomicBool::new(false);
        assert_eq!(
            execute(
                &member, &item, &apartment, 0, false, &pending, &stopped, &stopped
            )
            .0,
            State::NeedsConfiguration
        );
        member.running_rule = RunningRule {
            kind: Detection::Executable,
            path: item.exec_path.clone(),
            ..RunningRule::default()
        };
        assert_eq!(
            execute(
                &member, &item, &apartment, 0, false, &pending, &stopped, &stopped
            )
            .0,
            State::AlreadyRunning
        );
        assert!(pending.lock().unwrap().is_empty());
    }
}
