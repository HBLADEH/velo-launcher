//! Bounded workers: discovery, Shell actions/writes, visible icons and updates.
//! Notifications are coalesced before crossing into the application's UI loop.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{SyncSender, sync_channel};
use std::sync::{Arc, Condvar, Mutex, OnceLock, RwLock};
use std::thread::JoinHandle;
use std::time::Duration;

use chrono::{DateTime, Utc};

use super::catalog::Catalog;
use super::config::Config;
use super::data::{ApplicationLibrary, DataStore, History, IndexCache, IndexLoad};
use super::group_run::{Report, State};
use super::icons::{self, Pixels};
use super::indexer::{self, Generation, Ticket};
use super::launch::Action;
use super::launch_groups::{self, Collection, DraftTicket, Group};
use super::model::AppItem;
use super::search::Index;
use super::storage;
use super::{DataError, JsonDocument, Result, library, update};
use crate::platform::windows::{
    everything::{self, Worker as FileSearchWorker},
    group_launch, hotkey, http, icons as native_icons,
    instance::Identity,
    launch, preferences, shell, system_tools, updater,
};

pub fn now() -> DateTime<Utc> {
    std::time::SystemTime::now().into()
}

pub struct Snapshot {
    pub catalog: Arc<Catalog>,
    pub config: Arc<Config>,
    pub history: Arc<History>,
    pub cache: Arc<IndexCache>,
    pub library: Arc<ApplicationLibrary>,
    pub groups: Arc<Collection>,
    pub group_index: Arc<Index>,
    pub groups_available: bool,
}

pub struct LoadedIcon {
    pub key: String,
    pub pixels: Pixels,
}

pub enum Event {
    GroupDetection {
        ticket: DraftTicket,
        result: Result<String>,
    },
    GroupRun(Box<Report>),
    Snapshot(Arc<Snapshot>),
    Status {
        scanning: bool,
        notice: &'static str,
    },
    Icons {
        ticket: Ticket,
        items: Vec<LoadedIcon>,
    },
    Launched {
        id: String,
        result: std::result::Result<(), launch::Failure>,
        history_saved: bool,
    },
    Managed {
        kind: &'static str,
        result: Result<String>,
    },
    Update(Box<UpdateState>),
    FileSearch {
        generation: u64,
        query: String,
        offset: u32,
        result: std::result::Result<(Vec<super::file_search::FileHit>, usize), everything::Error>,
    },
    FileOpened {
        generation: u64,
        locate: bool,
        result: std::result::Result<(), launch::Failure>,
    },
    FileUtility {
        kind: &'static str,
        success: bool,
    },
}

#[derive(Clone, Default)]
pub struct UpdateState {
    pub busy: bool,
    pub progress: Option<InstallProgress>,
    pub message: String,
    pub info: Arc<update::Info>,
    pub scheduled: bool,
}

#[derive(Clone, Copy)]
pub enum InstallProgress {
    Preparing,
    Download(update::DownloadProgress),
    PreparingInstaller,
}
pub enum Manage {
    SaveGroup(Group),
    DeleteGroup(String),
    Save(Config),
    Pin(String, bool),
    SetFolderPinned(PathBuf, bool),
    Import(Vec<PathBuf>),
    Delete(String),
    Backup,
    OpenBackups(isize),
}
enum Work {
    GroupDetection(DraftTicket, PathBuf),
    Group {
        id: String,
        only: Vec<String>,
        force: Option<String>,
        owner: isize,
        run_id: u64,
    },
    Launch(LaunchRequest),
    FileOpen {
        path: String,
        generation: u64,
        locate: bool,
        owner: isize,
    },
    FileUtility {
        kind: &'static str,
        executable: String,
    },
    Manage(Manage),
}
#[derive(Clone)]
pub struct Options {
    pub hotkey: hotkey::Client,
    pub startup: preferences::Startup,
    pub identity: Identity,
}

struct IconRequest {
    ticket: Ticket,
    items: Vec<AppItem>,
}
struct IconMailbox {
    request: Mutex<Option<IconRequest>>,
    wake: Condvar,
}

struct Shared {
    groups: OnceLock<storage::Store<Collection>>,
    group_run_id: AtomicU64,
    group_cancelled: AtomicBool,
    pending_groups: group_launch::Pending,
    directory: PathBuf,
    store: OnceLock<Arc<DataStore>>,
    snapshot: RwLock<Option<Arc<Snapshot>>>,
    // Writers serialize index/history/library acceptance. Readers only clone
    // the immutable Arc and never wait for disk operations on this mutex.
    commit: Mutex<()>,
    refresh: Generation,
    icon_generation: Generation,
    stopping: AtomicBool,
    events: Mutex<(bool, VecDeque<Event>)>,
    notify: Box<dyn Fn() + Send + Sync>,
    icon_mailbox: IconMailbox,
    tools: RwLock<Vec<AppItem>>,
    options: Option<Options>,
}

impl Shared {
    fn event(&self, event: Event) {
        if self.stopping.load(Ordering::Acquire) {
            return;
        }
        let scheduled = {
            let Ok(mut queue) = self.events.lock() else {
                return;
            };
            // A single launch can be active. Other event types replace their
            // previous pending value, bounding this mailbox by the event types.
            queue
                .1
                .retain(|old| std::mem::discriminant(old) != std::mem::discriminant(&event));
            queue.1.push_back(event);
            !std::mem::replace(&mut queue.0, true)
        };
        if scheduled {
            (self.notify)();
        }
    }
    fn status(&self, scanning: bool, notice: &'static str) {
        self.event(Event::Status { scanning, notice });
    }
    fn snapshot(&self) -> Result<Option<Arc<Snapshot>>> {
        self.snapshot
            .read()
            .map(|s| s.clone())
            .map_err(|_| DataError::new("read application snapshot", "poisoned lock"))
    }
    fn publish(&self, cache: Arc<IndexCache>) -> Result<()> {
        let store = self
            .store
            .get()
            .ok_or_else(|| DataError::new("publish catalog", "data not ready"))?;
        let config = store.config.snapshot()?;
        let history = store.history.snapshot()?;
        let original_library = store.library.snapshot()?;
        let mut library = original_library.as_ref().clone();
        for item in library
            .applications
            .iter_mut()
            .flatten()
            .chain(library.pinned.iter_mut().flatten())
        {
            item.icon_url = icons::signature(item);
        }
        let items = indexer::visible(
            cache.apps.as_deref().unwrap_or_default(),
            config.filter_noise,
        );
        let groups = self
            .groups
            .get()
            .map(|store| store.snapshot())
            .transpose()?
            .unwrap_or_else(|| Arc::new(Collection::default()));
        let group_index = Arc::new(Index::new(groups.groups.iter().map(|group| {
            Arc::new(AppItem {
                id: group.id.clone(),
                name: group.name.clone(),
                keywords: Some(group.aliases.clone()),
                source: "LaunchGroup".into(),
                description: format!("启动组合 · {} 个应用", group.members.len()),
                ..AppItem::default()
            })
        })));
        let snapshot = Arc::new(Snapshot {
            groups,
            group_index,
            groups_available: self.groups.get().is_some(),
            catalog: Arc::new(Catalog::new(
                &items,
                &library,
                &self
                    .tools
                    .read()
                    .map_err(|_| DataError::new("read system tools", "poisoned lock"))?,
            )),
            config,
            history,
            cache,
            library: original_library,
        });
        *self
            .snapshot
            .write()
            .map_err(|_| DataError::new("publish catalog", "poisoned lock"))? =
            Some(snapshot.clone());
        self.event(Event::Snapshot(snapshot));
        Ok(())
    }
}

struct LaunchRequest {
    id: String,
    query: String,
    action: Action,
    owner: isize,
}

pub struct Runtime {
    shared: Arc<Shared>,
    refresh: Option<SyncSender<()>>,
    actions: Option<SyncSender<Work>>,
    action_busy: Arc<AtomicBool>,
    updates: Option<SyncSender<bool>>,
    file_search: Option<FileSearchWorker>,
    file_search_enabled: bool,
    update_busy: Arc<AtomicBool>,
    workers: Vec<JoinHandle<()>>,
}

impl Runtime {
    /// The caller holds the instance/data-directory guard. Returns immediately;
    /// cached data is loaded and the first scan runs entirely in the background.
    pub fn start(directory: &Path, notify: impl Fn() + Send + Sync + 'static) -> Result<Self> {
        Self::start_with_options(directory, None, notify)
    }
    pub fn start_with_options(
        directory: &Path,
        options: Option<Options>,
        notify: impl Fn() + Send + Sync + 'static,
    ) -> Result<Self> {
        let shared = Arc::new(Shared {
            groups: OnceLock::new(),
            group_run_id: AtomicU64::new(0),
            group_cancelled: AtomicBool::new(false),
            pending_groups: Mutex::new(Default::default()),
            directory: directory.into(),
            store: OnceLock::new(),
            snapshot: RwLock::new(None),
            commit: Mutex::new(()),
            refresh: Generation::default(),
            icon_generation: Generation::default(),
            stopping: AtomicBool::new(false),
            events: Mutex::new((false, VecDeque::new())),
            notify: Box::new(notify),
            icon_mailbox: IconMailbox {
                request: Mutex::new(None),
                wake: Condvar::new(),
            },
            tools: RwLock::new(Vec::new()),
            options,
        });
        let (refresh, requests) = sync_channel(1);
        let (actions, launches) = sync_channel::<Work>(1);
        let (updates, update_requests) = sync_channel::<bool>(1);
        let action_busy = Arc::new(AtomicBool::new(false));
        let mut runtime = Self {
            shared: shared.clone(),
            refresh: Some(refresh),
            actions: Some(actions),
            action_busy: action_busy.clone(),
            updates: Some(updates),
            file_search: None,
            file_search_enabled: false,
            update_busy: Arc::new(AtomicBool::new(false)),
            workers: Vec::new(),
        };
        let scan_state = shared.clone();
        runtime.workers.push(
            std::thread::Builder::new()
                .name("velo-discovery".into())
                .spawn(move || {
                    let mut initialized = false;
                    loop {
                        if scan_state.stopping.load(Ordering::Acquire) {
                            break;
                        }
                        if !initialized {
                            match initialize(&scan_state) {
                                Ok(()) => initialized = true,
                                Err(error) => scan_state.status(
                                    false,
                                    if error.operation == "preserve pre-migration user data" {
                                        "接管前数据备份未通过，尚未打开用户数据；请检查磁盘空间、备份权限和目录链接后重试"
                                    } else {
                                        "读取应用数据失败，原文件已保留；请检查数据目录后刷新"
                                    },
                                ),
                            }
                        }
                        if initialized {
                            refresh_index(&scan_state);
                        }
                        let minutes = scan_state
                            .store
                            .get()
                            .and_then(|s| s.config.snapshot().ok())
                            .map_or(30, |c| c.refresh_minutes);
                        match requests.recv_timeout(Duration::from_secs(minutes as u64 * 60)) {
                            Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                        }
                    }
                })
                .map_err(|e| DataError::new("start discovery worker", e))?,
        );
        let action_state = shared.clone();
        let refresh_after_action = runtime.refresh.clone();
        runtime.workers.push(
            std::thread::Builder::new()
                .name("velo-shell-actions".into())
                .spawn(move || {
                    while let Ok(request) = launches.recv() {
                        if action_state.stopping.load(Ordering::Acquire) {
                            break;
                        }
                        match request {
                            Work::GroupDetection(ticket, path) => {
                                let result = (|| -> Result<String> {
                                    let path_text = path.to_string_lossy();
                                    if !path.is_absolute()
                                        || !indexer::extension(&path_text, "exe")
                                        || !std::fs::metadata(&path)
                                            .is_ok_and(|metadata| metadata.is_file())
                                    {
                                        return Err(DataError::new(
                                            "check group detection",
                                            "readable executable required",
                                        ));
                                    }
                                    Ok(path_text.into_owned())
                                })();
                                action_state.event(Event::GroupDetection { ticket, result });
                            }
                            Work::Group {
                                id,
                                only,
                                force,
                                owner,
                                run_id,
                            } => {
                                run_group(&action_state, &id, &only, force, owner, run_id);
                            }
                            Work::Launch(request) => {
                                let result = run_action(&action_state, &request);
                                action_state.event(Event::Launched {
                                    id: request.id,
                                    result: result.0,
                                    history_saved: result.1,
                                });
                            }
                            Work::FileOpen {
                                path,
                                generation,
                                locate,
                                owner,
                            } => {
                                let result = shell::Apartment::new()
                                    .map_err(|_| launch::Failure::Shell)
                                    .and_then(|_apartment| {
                                        launch::open_native(&path, owner, locate)
                                    });
                                action_state.event(Event::FileOpened {
                                    generation,
                                    locate,
                                    result,
                                });
                            }
                            Work::FileUtility { kind, executable } => {
                                let success = shell::Apartment::new().is_ok_and(|_apartment| {
                                    if kind == "start" {
                                        everything::launch(&executable).is_ok()
                                    } else {
                                        launch::open_url("https://www.voidtools.com/downloads/")
                                            .is_ok()
                                    }
                                });
                                action_state.event(Event::FileUtility { kind, success });
                            }
                            Work::Manage(request) => {
                                let kind = match request {
                                    Manage::SaveGroup(_) => "group_save",
                                    Manage::DeleteGroup(_) => "group_delete",
                                    Manage::Save(_) => "settings",
                                    Manage::Import(_) => "import",
                                    Manage::Delete(_) => "delete",
                                    Manage::Pin(..) | Manage::SetFolderPinned(..) => "pin",
                                    Manage::Backup => "backup",
                                    Manage::OpenBackups(_) => "open_backups",
                                };
                                let result = manage(&action_state, request);
                                if kind == "settings"
                                    && result.is_ok()
                                    && let Some(sender) = &refresh_after_action
                                {
                                    let _ = sender.try_send(());
                                }
                                action_state.event(Event::Managed { kind, result });
                            }
                        }
                    }
                })
                .map_err(|e| DataError::new("start Shell worker", e))?,
        );
        runtime.workers.push(
            std::thread::Builder::new()
                .name("velo-visible-icons".into())
                .spawn(move || icon_worker(&shared))
                .map_err(|e| DataError::new("start icon worker", e))?,
        );
        if runtime.shared.options.is_some() {
            let update_state = runtime.shared.clone();
            let busy = runtime.update_busy.clone();
            runtime.workers.push(
                std::thread::Builder::new()
                    .name("velo-updates".into())
                    .spawn(move || update_worker(&update_state, update_requests, &busy))
                    .map_err(|e| DataError::new("start update worker", e))?,
            );
        }
        Ok(runtime)
    }

    pub fn refresh(&self) {
        self.shared.refresh.advance();
        if let Some(sender) = &self.refresh {
            let _ = sender.try_send(());
        }
    }
    pub fn configure_file_search(&mut self, enabled: bool) {
        self.file_search_enabled = enabled;
        if !enabled {
            if let Some(worker) = &self.file_search {
                worker.request_stop();
            }
            if self
                .file_search
                .as_ref()
                .is_some_and(FileSearchWorker::is_finished)
            {
                self.file_search.take();
            }
        }
    }
    pub fn cancel_file_search(&self) {
        if let Some(worker) = &self.file_search {
            worker.cancel();
        }
    }
    pub fn file_search(&mut self, query: String, generation: u64, offset: u32) -> bool {
        if !self.file_search_enabled
            || query.is_empty()
            || self.shared.stopping.load(Ordering::Acquire)
        {
            return false;
        }
        if self
            .file_search
            .as_ref()
            .is_some_and(|worker| worker.is_finished())
        {
            self.file_search.take();
        }
        if self
            .file_search
            .as_ref()
            .is_some_and(|worker| !worker.accepting())
        {
            return false;
        }
        if self.file_search.is_none() {
            let shared = self.shared.clone();
            self.file_search =
                match FileSearchWorker::start(move |generation, query, offset, result| {
                    shared.event(Event::FileSearch {
                        generation,
                        query,
                        offset,
                        result,
                    });
                }) {
                    Ok(worker) => Some(worker),
                    Err(_) => return false,
                };
        }
        self.file_search
            .as_ref()
            .is_some_and(|worker| worker.query(query, offset, generation))
    }
    pub fn open_file(&self, path: String, generation: u64, locate: bool, owner: isize) -> bool {
        self.group_work(Work::FileOpen {
            path,
            generation,
            locate,
            owner,
        })
    }
    pub fn file_utility(&self, start: bool, executable: String) -> bool {
        self.group_work(Work::FileUtility {
            kind: if start { "start" } else { "site" },
            executable,
        })
    }
    pub fn drain(&self) -> Vec<Event> {
        let Ok(mut events) = self.shared.events.lock() else {
            return Vec::new();
        };
        events.0 = false;
        let drained: Vec<_> = events.1.drain(..).collect();
        if drained.iter().any(|event| {
            matches!(
                event,
                Event::Launched { .. }
                    | Event::Managed { .. }
                    | Event::GroupDetection { .. }
                    | Event::FileOpened { .. }
                    | Event::FileUtility { .. }
            ) || matches!(event, Event::GroupRun(report) if report.done)
        }) {
            self.action_busy.store(false, Ordering::Release);
        }
        drained
    }
    pub fn launch(&self, id: String, query: String, action: Action, owner: isize) -> bool {
        if self.shared.stopping.load(Ordering::Acquire)
            || self.update_busy.load(Ordering::Acquire)
            || self.action_busy.swap(true, Ordering::AcqRel)
        {
            return false;
        }
        if self.actions.as_ref().is_some_and(|sender| {
            sender
                .try_send(Work::Launch(LaunchRequest {
                    id,
                    query,
                    action,
                    owner,
                }))
                .is_ok()
        }) {
            return true;
        }
        self.action_busy.store(false, Ordering::Release);
        false
    }
    fn group_work(&self, work: Work) -> bool {
        if self.shared.stopping.load(Ordering::Acquire)
            || self.update_busy.load(Ordering::Acquire)
            || self.action_busy.swap(true, Ordering::AcqRel)
        {
            return false;
        }
        if self
            .actions
            .as_ref()
            .is_some_and(|sender| sender.try_send(work).is_ok())
        {
            return true;
        }
        self.action_busy.store(false, Ordering::Release);
        false
    }
    pub fn group_detection(&self, ticket: DraftTicket, path: PathBuf) -> bool {
        self.group_work(Work::GroupDetection(ticket, path))
    }
    pub fn launch_group(
        &self,
        id: String,
        only: Vec<String>,
        force: Option<String>,
        owner: isize,
    ) -> bool {
        if self.action_busy.load(Ordering::Acquire) {
            return false;
        }
        self.shared.group_cancelled.store(false, Ordering::Release);
        let run_id = self.shared.group_run_id.fetch_add(1, Ordering::Relaxed) + 1;
        self.group_work(Work::Group {
            id,
            only,
            force,
            owner,
            run_id,
        })
    }
    pub fn cancel_group(&self) {
        self.shared.group_cancelled.store(true, Ordering::Release);
    }
    pub fn is_pinned(&self, id: &str) -> bool {
        self.shared
            .snapshot()
            .ok()
            .flatten()
            .is_some_and(|snapshot| snapshot.library.has_pin(id))
    }

    pub fn manage(&self, request: Manage) -> bool {
        if self.shared.stopping.load(Ordering::Acquire)
            || self.update_busy.load(Ordering::Acquire)
            || self.action_busy.swap(true, Ordering::AcqRel)
        {
            return false;
        }
        if self
            .actions
            .as_ref()
            .is_some_and(|sender| sender.try_send(Work::Manage(request)).is_ok())
        {
            return true;
        }
        self.action_busy.store(false, Ordering::Release);
        false
    }
    pub fn update(&self, install: bool) -> bool {
        if self.shared.options.is_none()
            || self.shared.stopping.load(Ordering::Acquire)
            || self.action_busy.load(Ordering::Acquire)
            || self.update_busy.swap(true, Ordering::AcqRel)
        {
            return false;
        }
        if self
            .updates
            .as_ref()
            .is_some_and(|sender| sender.try_send(install).is_ok())
        {
            return true;
        }
        self.update_busy.store(false, Ordering::Release);
        false
    }
    pub fn icons(&self, items: Vec<AppItem>) {
        self.cancel_icons();
        if items.is_empty() {
            return;
        }
        let ticket = self.shared.icon_generation.ticket();
        if let Ok(mut request) = self.shared.icon_mailbox.request.lock() {
            *request = Some(IconRequest {
                ticket,
                items: items.into_iter().take(96).collect(),
            });
            self.shared.icon_mailbox.wake.notify_one();
        }
    }
    pub fn cancel_icons(&self) {
        self.shared.icon_generation.advance();
        if let Ok(mut request) = self.shared.icon_mailbox.request.lock() {
            *request = None;
        }
    }
    pub fn stop(&mut self) {
        self.shared.stopping.store(true, Ordering::Release);
        self.shared.refresh.advance();
        self.cancel_icons();
        self.shared.icon_mailbox.wake.notify_all();
        self.refresh.take();
        self.actions.take();
        self.updates.take();
        self.file_search.take();
        for worker in self.workers.drain(..) {
            if worker.join().is_err() {
                eprintln!("event=worker_shutdown_error");
            }
        }
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        self.stop();
    }
}

struct HotkeyReservation<'a>(&'a hotkey::Client, bool);
impl Drop for HotkeyReservation<'_> {
    fn drop(&mut self) {
        if !self.1 && self.0.finish(false).is_err() {
            eprintln!("event=hotkey_reservation_cleanup_failed");
        }
    }
}

fn manage(shared: &Shared, request: Manage) -> Result<String> {
    let _commit = shared
        .commit
        .lock()
        .map_err(|_| DataError::new("commit application change", "poisoned lock"))?;
    let store = shared
        .store
        .get()
        .ok_or_else(|| DataError::new("commit application change", "not initialized"))?;
    let snapshot = shared
        .snapshot()?
        .ok_or_else(|| DataError::new("commit application change", "not initialized"))?;
    let message = match request {
        Manage::SaveGroup(group) => {
            shared
                .groups
                .get()
                .ok_or_else(|| DataError::new("save launch group", "groups unavailable"))?
                .update(|current| current.with_group(group))?;
            "启动组合已保存".into()
        }
        Manage::DeleteGroup(id) => {
            shared
                .groups
                .get()
                .ok_or_else(|| DataError::new("delete launch group", "groups unavailable"))?
                .update(|current| current.without_group(&id))?;
            "启动组合已删除".into()
        }
        Manage::Save(config) => {
            config.validate()?;
            let key = hotkey::Key::parse(&config.hotkey)
                .map_err(|e| DataError::new("validate global hotkey", e))?;
            let options = shared
                .options
                .as_ref()
                .ok_or_else(|| DataError::new("save settings", "platform not configured"))?;
            options
                .hotkey
                .prepare(key)
                .map_err(|e| DataError::new("reserve global hotkey", e))?;
            let mut reservation = HotkeyReservation(&options.hotkey, false);
            // Saving an enabled entry also refreshes its command after a move
            // and revalidates older entries that Windows could not execute.
            let mut startup = if config.launch_at_startup
                || config.launch_at_startup != snapshot.config.launch_at_startup
            {
                Some(options.startup.change(config.launch_at_startup)?)
            } else {
                None
            };
            if let Err(error) = store.config.update(|_| Ok(config)) {
                if let Some(startup) = &mut startup {
                    startup.rollback()?;
                }
                return Err(error);
            }
            options
                .hotkey
                .finish(true)
                .map_err(|e| DataError::new("commit global hotkey", e))?;
            reservation.1 = true;
            if let Some(startup) = startup {
                startup.commit();
            }
            // Cancel scans made with the previous settings before releasing
            // the commit lock; none may slip in between save and rescheduling.
            shared.refresh.advance();
            "设置已保存".into()
        }
        Manage::Pin(id, pinned) => {
            store
                .library
                .update(|library| snapshot.catalog.with_pin(library, &id, pinned))?;
            if pinned {
                "已固定到首页"
            } else {
                "已取消固定"
            }
            .into()
        }
        Manage::SetFolderPinned(path, pinned) => {
            let id = library::folder_id(&path)?;
            if pinned {
                let current = store.library.snapshot()?;
                if !current.has_pin(&id) {
                    let folder = library::folder(&path)?;
                    store
                        .library
                        .update(|library| library.with_new_pin(&folder))?;
                }
                "已将文件夹固定到首页"
            } else {
                store
                    .library
                    .update(|library| snapshot.catalog.with_pin(library, &id, false))?;
                "已取消固定文件夹"
            }
            .into()
        }
        Manage::Import(paths) => {
            let apartment = shell::Apartment::new()?;
            let imported = library::import(
                store.library.snapshot()?.as_ref(),
                &paths,
                &mut shell::ShortcutResolver(&apartment),
            )?;
            if imported.added > 0 {
                store.library.update(|_| Ok(imported.library))?;
            }
            format!(
                "已添加 {} 个项目 · {} 个项目无法导入",
                imported.added, imported.rejected
            )
        }
        Manage::Delete(id) => {
            store
                .library
                .update(|library| library::delete(library, &id))?;
            "已移除自定义项目及其固定项，磁盘内容保留".into()
        }
        Manage::Backup => store
            .config
            .backup(&shared.directory.join("backups"))?
            .to_string_lossy()
            .into_owned(),
        Manage::OpenBackups(owner) => {
            let directory = shared.directory.join("backups");
            std::fs::create_dir_all(&directory)
                .map_err(|e| DataError::new("create backup directory", e))?;
            let path = directory.to_string_lossy().into_owned();
            let item = AppItem {
                path: path.clone(),
                exec_path: path,
                ..AppItem::default()
            };
            launch::execute(&shell::Apartment::new()?, owner, &item, Action::Launch)
                .map_err(|e| DataError::new("open backup directory", format!("{e:?}")))?;
            String::new()
        }
    };
    shared.publish(snapshot.cache.clone())?;
    Ok(message)
}

fn update_worker(shared: &Shared, requests: std::sync::mpsc::Receiver<bool>, busy: &AtomicBool) {
    let mut client = update::Client::for_build(http::Http);
    let mut state = UpdateState::default();
    let mut first = true;
    loop {
        let install = if first {
            first = false;
            match requests.recv_timeout(Duration::from_secs(12)) {
                Ok(install) => install,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    if !shared
                        .store
                        .get()
                        .and_then(|s| s.config.snapshot().ok())
                        .is_some_and(|c| c.auto_check_updates)
                    {
                        continue;
                    }
                    if busy.swap(true, Ordering::AcqRel) {
                        continue;
                    }
                    false
                }
                Err(_) => return,
            }
        } else {
            match requests.recv() {
                Ok(install) => install,
                Err(_) => return,
            }
        };
        if shared.stopping.load(Ordering::Acquire) {
            return;
        }
        state.busy = true;
        state.progress = install.then_some(InstallProgress::Preparing);
        state.scheduled = false;
        state.message = if install {
            "正在连接更新服务器…"
        } else {
            "正在检查更新…"
        }
        .into();
        shared.event(Event::Update(Box::new(state.clone())));
        let result = (|| -> Result<()> {
            let info = Arc::new(client.check(update::CURRENT, &shared.stopping)?);
            state.info = info.clone();
            if !install {
                state.message = if state.info.release.is_none() {
                    "当前已是最新版本".into()
                } else if state.info.portable.is_none() && state.info.installer.is_none() {
                    "发布页尚未提供 Rust 版本更新包".into()
                } else {
                    format!(
                        "发现新版本 {}",
                        state
                            .info
                            .release
                            .as_ref()
                            .map_or("", |r| r.tag_name.as_str())
                    )
                };
                return Ok(());
            }
            if state.info.release.is_none() {
                return Err(DataError::new("install update", "no newer version"));
            }
            let current =
                std::env::current_exe().map_err(|e| DataError::new("find update executable", e))?;
            let installed_product = shared
                .options
                .as_ref()
                .is_some_and(|options| options.identity.migration_id.is_some());
            let portable = !installed_product && current.parent().is_some_and(updater::writable);
            let (source, hash) = client.download(
                &info,
                &shared.directory.join("updates"),
                portable,
                &shared.stopping,
                &mut |progress| {
                    state.progress = Some(InstallProgress::Download(progress));
                    state.message = match progress {
                        update::DownloadProgress::Transfer { .. } => "正在下载更新…",
                        update::DownloadProgress::Verifying => "正在校验下载文件…",
                    }
                    .into();
                    shared.event(Event::Update(Box::new(state.clone())));
                },
            )?;
            update::check_cancel(&shared.stopping)?;
            state.progress = Some(InstallProgress::PreparingInstaller);
            state.message = "正在备份配置并准备安装…".into();
            shared.event(Event::Update(Box::new(state.clone())));
            let script = prepare_update(shared, &source, portable, &hash)?;
            update::check_cancel(&shared.stopping)?;
            updater::schedule(&script)?;
            state.scheduled = true;
            state.message = "更新已准备，将重新启动 Velo".into();
            Ok(())
        })();
        if let Err(error) = result {
            state.message = format!(
                "{}；请检查网络、代理和目录权限后重试",
                match error.operation {
                    "verify update SHA-256" => "更新文件校验失败，已丢弃下载内容",
                    "require update checksum" | "find update checksum" =>
                        "更新缺少有效的 SHA-256 校验记录，已停止安装",
                    "require matching Rust update artifact" => "发布页未提供适用的 Rust 更新包",
                    "read backup source" | "create backup" | "write backup" | "flush backup"
                    | "commit backup" => "更新前配置备份失败，已停止安装",
                    _ => "更新未完成",
                }
            );
            eprintln!("event=update_failed operation={}", error.operation);
        }
        state.busy = false;
        state.progress = None;
        busy.store(false, Ordering::Release);
        shared.event(Event::Update(Box::new(state.clone())));
    }
}

fn prepare_update(shared: &Shared, source: &Path, portable: bool, hash: &str) -> Result<PathBuf> {
    let _commit = shared
        .commit
        .lock()
        .map_err(|_| DataError::new("backup before update", "poisoned lock"))?;
    shared
        .store
        .get()
        .ok_or_else(|| DataError::new("backup before update", "not initialized"))?
        .config
        .backup(&shared.directory.join("backups"))?;
    let identity = &shared
        .options
        .as_ref()
        .ok_or_else(|| DataError::new("prepare update", "missing identity"))?
        .identity;
    let target = updater::target(portable, identity)?;
    updater::prepare(
        &shared.directory.join("updates"),
        &target,
        source,
        portable,
        hash,
        identity,
    )
}

fn initialize(shared: &Shared) -> Result<()> {
    if let Some(id) = shared
        .options
        .as_ref()
        .and_then(|o| o.identity.migration_id.as_deref())
    {
        super::migration::preserve(&shared.directory, id)?;
    }
    let store = Arc::new(DataStore::open(&shared.directory)?);
    match launch_groups::open(&shared.directory) {
        Ok(groups) => {
            let _ = shared.groups.set(groups);
        }
        Err(error) => eprintln!(
            "event=launch_groups_unavailable operation={}",
            error.operation
        ),
    }
    if matches!(store.index, IndexLoad::RebuildRequired(_)) {
        let original = std::fs::read(shared.directory.join("index.json"))
            .map_err(|e| DataError::new("preserve unusable index", e))?;
        storage::preserve_bytes(&shared.directory, "index.json.unusable", &original)?;
    }
    let cache = Arc::new(match &store.index {
        IndexLoad::Ready(cache) => cache.clone(),
        _ => IndexCache {
            version: 1,
            ..IndexCache::default()
        },
    });
    let notice = match &store.index {
        IndexLoad::RebuildRequired(_) => "索引缓存无效，正在后台重建；原文件已保留",
        _ => "",
    };
    shared
        .store
        .set(store)
        .map_err(|_| DataError::new("initialize catalog", "already initialized"))?;
    *shared
        .tools
        .write()
        .map_err(|_| DataError::new("publish system tools", "poisoned lock"))? =
        system_tools::discover()?;
    shared.publish(cache)?;
    shared.status(true, notice);
    Ok(())
}

fn refresh_index(shared: &Shared) {
    let ticket = shared.refresh.ticket();
    shared.status(true, "正在刷新应用索引…");
    let outcome = (|| -> Result<usize> {
        let previous = shared
            .snapshot()?
            .ok_or_else(|| DataError::new("refresh index", "not initialized"))?;
        let apartment = shell::Apartment::new()?;
        let tools = system_tools::discover()?;
        let roots = shell::roots(&previous.config)?;
        let (mut next, mut stats) = indexer::scan(
            &roots,
            &previous.cache,
            &mut shell::ShortcutResolver(&apartment),
            &ticket,
        )?;
        let packaged = match shell::packaged_apps(&apartment, &ticket) {
            Ok(items) => items,
            Err(_) => {
                ticket.check()?;
                stats.warnings += 1;
                previous
                    .cache
                    .apps
                    .iter()
                    .flatten()
                    .filter(|item| item.source == "Windows Apps")
                    .cloned()
                    .collect()
            }
        };
        let apps = next.apps.get_or_insert_default();
        apps.extend(packaged);
        *apps = indexer::deduplicate(std::mem::take(apps));
        icons::assign(apps, &ticket)?;
        ticket.check()?;
        *shared
            .tools
            .write()
            .map_err(|_| DataError::new("publish system tools", "poisoned lock"))? = tools;
        if !commit_index(shared, next, &previous.cache, &ticket)? {
            stats.warnings += 1;
        }
        Ok(stats.warnings)
    })();
    if !ticket.current() {
        return;
    }
    match outcome {
        Ok(0) => shared.status(false, ""),
        Ok(_) => shared.status(
            false,
            "部分目录或图标无法读取，已保留可复用条目；可稍后刷新重试",
        ),
        Err(_) => shared.status(
            false,
            "应用索引刷新失败，继续使用上次结果；请检查目录权限后重试",
        ),
    }
}

fn commit_index(
    shared: &Shared,
    next: IndexCache,
    previous: &IndexCache,
    ticket: &Ticket,
) -> Result<bool> {
    let _commit = shared
        .commit
        .lock()
        .map_err(|_| DataError::new("commit index", "poisoned lock"))?;
    ticket.check()?;
    // Preserve the old published state if the durable replacement fails. Check
    // cancellation again after serialization, flushing, and before replacement.
    storage::write_checked(&shared.directory.join("index.json"), &next, || {
        ticket.check()
    })?;
    ticket.check()?;
    let library = shared
        .store
        .get()
        .ok_or_else(|| DataError::new("commit index", "not initialized"))?
        .library
        .snapshot()?;
    let mut retained = next
        .apps
        .iter()
        .flatten()
        .chain(previous.apps.iter().flatten())
        .chain(library.applications.iter().flatten())
        .chain(library.pinned.iter().flatten())
        .map(|item| item.icon_url.clone())
        .collect::<Vec<_>>();
    shared.publish(Arc::new(next))?;
    if let Some(snapshot) = shared.snapshot()? {
        retained.extend(
            snapshot
                .catalog
                .applications()
                .map(|item| item.icon_url.clone()),
        );
    }
    Ok(!ticket.current() || icons::prune(&shared.directory, retained.into_iter()).is_ok())
}

fn run_group(
    shared: &Shared,
    id: &str,
    only: &[String],
    force: Option<String>,
    owner: isize,
    run_id: u64,
) {
    let snapshot = shared.snapshot().ok().flatten();
    let group = snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.groups.groups.iter().find(|group| group.id == id))
        .cloned();
    let Some(mut group) = group else {
        shared.event(Event::GroupRun(Box::new(Report {
            run_id,
            group_id: id.into(),
            name: "启动组合不可用".into(),
            entries: Vec::new(),
            done: true,
            history_saved: true,
        })));
        return;
    };
    if !only.is_empty() {
        group.members.retain(|member| only.contains(&member.app_id));
    }
    let items = group
        .members
        .iter()
        .map(|member| {
            snapshot.as_ref().and_then(|snapshot| {
                snapshot
                    .catalog
                    .item(&member.app_id)
                    .map(|item| item.as_ref().clone())
            })
        })
        .collect();
    let mut report = group_launch::run(
        group_launch::Request {
            group,
            items,
            run_id,
            owner,
            force,
        },
        &shared.pending_groups,
        &shared.stopping,
        &shared.group_cancelled,
        |report| shared.event(Event::GroupRun(Box::new(report))),
    );
    let successes: Vec<_> = report
        .entries
        .iter()
        .filter(|entry| entry.state == State::Submitted)
        .map(|entry| entry.app_id.as_str())
        .collect();
    if !successes.is_empty() {
        report.history_saved = (|| -> Result<()> {
            let _commit = shared
                .commit
                .lock()
                .map_err(|_| DataError::new("record group history", "poisoned lock"))?;
            let store = shared
                .store
                .get()
                .ok_or_else(|| DataError::new("record group history", "not initialized"))?;
            store.history.update(|current| {
                let mut history = current.clone();
                for id in &successes {
                    history = history.record(id, "", now().fixed_offset())?;
                }
                Ok(history)
            })?;
            if let Some(snapshot) = shared.snapshot()? {
                shared.publish(snapshot.cache.clone())?;
            }
            Ok(())
        })()
        .is_ok();
    }
    shared.event(Event::GroupRun(Box::new(report)));
}

fn run_action(
    shared: &Shared,
    request: &LaunchRequest,
) -> (std::result::Result<(), launch::Failure>, bool) {
    let result = (|| {
        let snapshot = shared
            .snapshot()
            .ok()
            .flatten()
            .ok_or(launch::Failure::Unavailable)?;
        let item = snapshot
            .catalog
            .item(&request.id)
            .ok_or(launch::Failure::Unavailable)?;
        let apartment = shell::Apartment::new().map_err(|_| launch::Failure::Shell)?;
        launch::execute(&apartment, request.owner, item, request.action)
    })();
    finish_action(shared, request, result)
}

fn finish_action(
    shared: &Shared,
    request: &LaunchRequest,
    result: std::result::Result<(), launch::Failure>,
) -> (std::result::Result<(), launch::Failure>, bool) {
    if result.is_err() || request.action == Action::OpenDirectory {
        return (result, true);
    }
    let saved = (|| -> Result<()> {
        let _commit = shared
            .commit
            .lock()
            .map_err(|_| DataError::new("record launch", "poisoned lock"))?;
        let store = shared
            .store
            .get()
            .ok_or_else(|| DataError::new("record launch", "not initialized"))?;
        store
            .history
            .record_success(&request.id, &request.query, now().fixed_offset())?;
        if let Some(snapshot) = shared.snapshot()? {
            shared.publish(snapshot.cache.clone())?;
        }
        Ok(())
    })()
    .is_ok();
    (result, saved)
}

fn icon_worker(shared: &Shared) {
    let apartment = match shell::Apartment::new() {
        Ok(apartment) => apartment,
        Err(_) => {
            shared.status(false, "图标服务不可用，已显示文字图标；请重启后重试");
            return;
        }
    };
    loop {
        let request = {
            let Ok(mut request) = shared.icon_mailbox.request.lock() else {
                return;
            };
            while request.is_none() && !shared.stopping.load(Ordering::Acquire) {
                request = match shared.icon_mailbox.wake.wait(request) {
                    Ok(request) => request,
                    Err(_) => return,
                };
            }
            if shared.stopping.load(Ordering::Acquire) {
                return;
            }
            request.take()
        };
        let Some(request) = request else {
            continue;
        };
        let mut items = Vec::new();
        for item in request.items {
            if !request.ticket.current() {
                break;
            }
            if let Ok(pixels) = icons::load(&shared.directory, &item, &request.ticket, |item| {
                native_icons::extract(&apartment, item)
            }) {
                items.push(LoadedIcon {
                    key: item.icon_url.clone(),
                    pixels,
                });
            }
        }
        if request.ticket.current() {
            shared.event(Event::Icons {
                ticket: request.ticket,
                items,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;

    fn shared(directory: &Path) -> Shared {
        Shared {
            groups: OnceLock::new(),
            group_run_id: AtomicU64::new(0),
            group_cancelled: AtomicBool::new(false),
            pending_groups: Mutex::new(Default::default()),
            directory: directory.into(),
            store: OnceLock::new(),
            snapshot: RwLock::new(None),
            commit: Mutex::new(()),
            refresh: Generation::default(),
            icon_generation: Generation::default(),
            stopping: AtomicBool::new(false),
            events: Mutex::new((false, VecDeque::new())),
            notify: Box::new(|| {}),
            icon_mailbox: IconMailbox {
                request: Mutex::new(None),
                wake: Condvar::new(),
            },
            tools: RwLock::new(Vec::new()),
            options: None,
        }
    }
    fn cache(id: &str) -> IndexCache {
        IndexCache {
            version: 1,
            apps: Some(vec![AppItem {
                id: id.into(),
                name: id.into(),
                source: "Desktop".into(),
                ..AppItem::default()
            }]),
            ..IndexCache::default()
        }
    }

    #[test]
    fn broken_optional_group_file_does_not_block_the_catalogue_or_change_original_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("launch-groups.json");
        let original = br#"{"version":200,"future":"preserve"}"#;
        fs::write(&path, original).unwrap();
        let shared = shared(directory.path());
        initialize(&shared).unwrap();
        assert!(shared.snapshot().unwrap().is_some());
        assert!(!shared.snapshot().unwrap().unwrap().groups_available);
        assert_eq!(fs::read(path).unwrap(), original);
        assert!(manage(&shared, Manage::DeleteGroup("group:missing".into())).is_err());
    }

    #[test]
    fn saved_groups_search_restart_and_native_partial_results_preserve_application_history() {
        use super::launch_groups::{Group, Member};
        let directory = tempfile::tempdir().unwrap();
        let shared = shared(directory.path());
        initialize(&shared).unwrap();
        let apartment = shell::Apartment::new().unwrap();
        let running = library::item(
            &std::env::current_exe().unwrap(),
            &mut shell::ShortcutResolver(&apartment),
        )
        .unwrap();
        let missing_path = directory
            .path()
            .join("synthetic-missing.exe")
            .to_string_lossy()
            .into_owned();
        let missing = AppItem {
            id: super::super::model::identity(&missing_path, ""),
            path: missing_path.clone(),
            exec_path: missing_path,
            name: "失效成员".into(),
            ..AppItem::default()
        };
        shared
            .store
            .get()
            .unwrap()
            .library
            .update(|current| {
                let mut library = current.clone();
                library.applications = Some(vec![running.clone(), missing.clone()]);
                Ok(library)
            })
            .unwrap();
        let cache = shared.snapshot().unwrap().unwrap().cache.clone();
        shared.publish(cache).unwrap();
        let group = Group {
            id: "group:synthetic".into(),
            name: "开黑".into(),
            members: vec![
                Member::from_app(&running).unwrap(),
                Member::from_app(&missing).unwrap(),
            ],
            ..Group::default()
        };
        manage(&shared, Manage::SaveGroup(group.clone())).unwrap();
        let snapshot = shared.snapshot().unwrap().unwrap();
        assert_eq!(
            snapshot
                .group_index
                .query("kh", 10, true, &Default::default(), 0.0)[0]
                .item
                .id,
            group.id
        );
        assert_eq!(
            launch_groups::open(directory.path())
                .unwrap()
                .snapshot()
                .unwrap()
                .groups[0],
            group
        );
        let before = shared.store.get().unwrap().history.snapshot().unwrap();
        run_group(&shared, &group.id, &[], None, 0, 1);
        let events = &shared.events.lock().unwrap().1;
        let report = events
            .iter()
            .find_map(|event| match event {
                Event::GroupRun(report) if report.done => Some(report),
                _ => None,
            })
            .unwrap();
        assert_eq!(report.entries.len(), 2);
        assert_eq!(report.entries[0].state, State::AlreadyRunning);
        assert_eq!(report.entries[1].state, State::Failed);
        assert!(report.accepted());
        assert_eq!(
            before,
            shared.store.get().unwrap().history.snapshot().unwrap()
        );
    }

    #[test]
    fn stale_cancelled_or_failed_commits_keep_the_published_index_and_do_not_prune() {
        let directory = tempfile::tempdir().unwrap();
        let index_path = directory.path().join("index.json");
        storage::write(&index_path, &cache("old")).unwrap();
        let shared = shared(directory.path());
        initialize(&shared).unwrap();
        let old = shared.snapshot().unwrap().unwrap();
        let disk = fs::read(&index_path).unwrap();
        let icon_dir = directory.path().join("cache/icons");
        fs::create_dir_all(&icon_dir).unwrap();
        let unreferenced = icon_dir.join("0123456789abcdef0123456789abcdef.png");
        fs::write(&unreferenced, b"retain until successful commit").unwrap();
        let stale = shared.refresh.ticket();
        shared.refresh.advance();
        assert!(commit_index(&shared, cache("stale"), &old.cache, &stale).is_err());
        let lock = fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .open(&index_path)
            .unwrap();
        assert!(commit_index(&shared, cache("new"), &old.cache, &shared.refresh.ticket()).is_err());
        assert_eq!(fs::read(&index_path).unwrap(), disk);
        assert!(Arc::ptr_eq(&old, &shared.snapshot().unwrap().unwrap()));
        assert!(unreferenced.exists());
        drop(lock);
        assert!(commit_index(&shared, cache("new"), &old.cache, &shared.refresh.ticket()).unwrap());
        assert!(
            shared
                .snapshot()
                .unwrap()
                .unwrap()
                .catalog
                .item("new")
                .is_some()
        );
        assert!(!unreferenced.exists());
    }

    #[test]
    fn scan_acceptance_uses_the_latest_library_and_cannot_resurrect_deleted_custom_entries() {
        let directory = tempfile::tempdir().unwrap();
        let shared = shared(directory.path());
        initialize(&shared).unwrap();
        let store = shared.store.get().unwrap();
        store
            .library
            .update(|library| {
                let mut library = library.clone();
                library.applications = cache("custom").apps;
                library.pinned = library.applications.clone();
                Ok(library)
            })
            .unwrap();
        let old = shared.snapshot().unwrap().unwrap().cache.clone();
        shared.publish(old.clone()).unwrap();
        assert!(
            shared
                .snapshot()
                .unwrap()
                .unwrap()
                .catalog
                .item("custom")
                .is_some()
        );
        store
            .library
            .update(|library| {
                let mut library = library.clone();
                library.applications = Some(vec![]);
                library.pinned = Some(vec![]);
                Ok(library)
            })
            .unwrap();
        commit_index(&shared, cache("scan"), &old, &shared.refresh.ticket()).unwrap();
        assert!(
            shared
                .snapshot()
                .unwrap()
                .unwrap()
                .catalog
                .item("custom")
                .is_none()
        );
    }

    #[test]
    fn failed_or_cancelled_shell_actions_do_not_learn_and_history_write_failure_is_explicit() {
        let directory = tempfile::tempdir().unwrap();
        let shared = shared(directory.path());
        initialize(&shared).unwrap();
        let request = LaunchRequest {
            id: "app".into(),
            query: "  学习 QUERY ".into(),
            action: Action::Launch,
            owner: 0,
        };
        for failure in [
            launch::Failure::Cancelled,
            launch::Failure::MissingTarget,
            launch::Failure::Unavailable,
            launch::Failure::Shell,
        ] {
            assert_eq!(
                finish_action(&shared, &request, Err(failure)).0,
                Err(failure)
            );
            assert!(
                shared
                    .store
                    .get()
                    .unwrap()
                    .history
                    .snapshot()
                    .unwrap()
                    .0
                    .is_empty()
            );
        }
        assert_eq!(finish_action(&shared, &request, Ok(())), (Ok(()), true));
        let history = shared.store.get().unwrap().history.snapshot().unwrap();
        assert_eq!(history.0["app"].launch_count, 1);
        assert_eq!(
            history.0["app"].query_history.as_ref().unwrap()["学习 query"],
            1
        );
        let lock = fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .open(directory.path().join("history.json"))
            .unwrap();
        assert_eq!(finish_action(&shared, &request, Ok(())), (Ok(()), false));
        assert!(Arc::ptr_eq(
            &history,
            &shared.store.get().unwrap().history.snapshot().unwrap()
        ));
        drop(lock);
    }

    #[test]
    fn settings_conflict_invalid_startup_and_disk_failure_preserve_old_state() {
        let directory = tempfile::tempdir().unwrap();
        let mut shared = shared(directory.path());
        let old = hotkey::Key::parse("Ctrl+Alt+Shift+F20").unwrap();
        let new = hotkey::Key::parse("Ctrl+Alt+Shift+F19").unwrap();
        let occupied = hotkey::Key::parse("Ctrl+Alt+Shift+F18").unwrap();
        let service = hotkey::Service::start(|_| {}).unwrap();
        service.bind(old).unwrap();
        let other = hotkey::Service::start(|_| {}).unwrap();
        other.bind(occupied).unwrap();
        let identity = Identity::resolve(
            Some(format!("rs11-transaction-{}", std::process::id())),
            Some(directory.path().into()),
        )
        .unwrap();
        let startup = preferences::Startup::for_instance(&identity).unwrap();
        shared.options = Some(Options {
            hotkey: service.client(),
            startup: startup.clone(),
            identity,
        });
        storage::write(
            &directory.path().join("config.json"),
            &Config {
                hotkey: old.to_string(),
                launch_at_startup: true,
                ..Config::default()
            },
        )
        .unwrap();
        initialize(&shared).unwrap();
        let previous = shared.snapshot().unwrap().unwrap();
        let previous_settings_scan = shared.refresh.ticket();
        let bytes = fs::read(directory.path().join("config.json")).unwrap();
        let mut config = previous.config.as_ref().clone();
        config.hotkey = occupied.to_string();
        assert!(manage(&shared, Manage::Save(config)).is_err());
        assert!(other.bind(old).is_err());
        // An already-enabled old config still needs validation. Failure must
        // release the proposed hotkey and leave the last commit untouched.
        shared.options.as_mut().unwrap().startup.command = "x".repeat(261);
        let mut invalid = previous.config.as_ref().clone();
        invalid.hotkey = new.to_string();
        let error = manage(&shared, Manage::Save(invalid)).unwrap_err();
        assert_eq!(error.operation, "validate login startup command");
        assert_eq!(
            fs::read(directory.path().join("config.json")).unwrap(),
            bytes
        );
        assert!(Arc::ptr_eq(&previous, &shared.snapshot().unwrap().unwrap()));
        assert!(other.bind(old).is_err());
        other.bind(new).unwrap();
        other.bind(occupied).unwrap();
        shared.options.as_mut().unwrap().startup.command = startup.command.clone();
        let lock = fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .open(directory.path().join("config.json"))
            .unwrap();
        let mut config = previous.config.as_ref().clone();
        config.hotkey = new.to_string();
        config.launch_at_startup = true;
        config.everything.enabled = true;
        assert!(manage(&shared, Manage::Save(config.clone())).is_err());
        assert!(other.bind(old).is_err());
        other.bind(new).unwrap();
        other.bind(occupied).unwrap();
        assert_eq!(
            fs::read(directory.path().join("config.json")).unwrap(),
            bytes
        );
        assert!(Arc::ptr_eq(&previous, &shared.snapshot().unwrap().unwrap()));
        assert!(
            !shared
                .snapshot()
                .unwrap()
                .unwrap()
                .config
                .everything
                .enabled
        );
        assert!(
            preferences::string(
                preferences::Root::User,
                r"Software\Microsoft\Windows\CurrentVersion\Run",
                &startup.name
            )
            .is_none()
        );
        drop(lock);
        assert!(previous_settings_scan.current());
        config.launch_at_startup = false;
        manage(&shared, Manage::Save(config.clone())).unwrap();
        assert!(
            commit_index(
                &shared,
                cache("obsolete-settings-scan"),
                &previous.cache,
                &previous_settings_scan
            )
            .is_err()
        );
        other.bind(old).unwrap();
        assert!(other.bind(new).is_err());
        assert_eq!(
            DataStore::open(directory.path())
                .unwrap()
                .config
                .snapshot()
                .unwrap()
                .as_ref(),
            &config
        );
    }

    #[test]
    fn import_pin_delete_failures_restart_and_late_refresh_keep_atomic_library_state() {
        let directory = tempfile::tempdir().unwrap();
        let shared = shared(directory.path());
        initialize(&shared).unwrap();
        let path = directory.path().join("中文 应用.exe");
        fs::write(&path, b"never execute").unwrap();
        manage(
            &shared,
            Manage::Import(vec![
                path.clone(),
                path.clone(),
                directory.path().join("missing.exe"),
            ]),
        )
        .unwrap();
        let snapshot = shared.snapshot().unwrap().unwrap();
        let apps = snapshot.library.applications.as_ref().unwrap();
        assert_eq!(apps.len(), 1);
        assert!(snapshot.library.pinned.is_none());
        let id = apps[0].id.clone();
        manage(&shared, Manage::Pin(id.clone(), true)).unwrap();
        let before = shared.snapshot().unwrap().unwrap();
        let library_path = directory.path().join("application-library.json");
        let bytes = fs::read(&library_path).unwrap();
        let lock = fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .open(&library_path)
            .unwrap();
        assert!(manage(&shared, Manage::Delete(id.clone())).is_err());
        assert_eq!(fs::read(&library_path).unwrap(), bytes);
        assert!(Arc::ptr_eq(&before, &shared.snapshot().unwrap().unwrap()));
        drop(lock);
        manage(&shared, Manage::Delete(id.clone())).unwrap();
        commit_index(
            &shared,
            cache("scan"),
            &before.cache,
            &shared.refresh.ticket(),
        )
        .unwrap();
        assert!(
            shared
                .snapshot()
                .unwrap()
                .unwrap()
                .catalog
                .item(&id)
                .is_none()
        );
        let restarted = DataStore::open(directory.path()).unwrap();
        let library = restarted.library.snapshot().unwrap();
        assert!(library.applications.as_ref().unwrap().is_empty());
        assert!(library.pinned.as_ref().unwrap().is_empty());
        assert!(path.exists());
    }

    #[test]
    fn folder_pins_share_home_and_survive_restart() {
        let directory = tempfile::tempdir().unwrap();
        let runtime = shared(directory.path());
        initialize(&runtime).unwrap();

        let name = "Velo 文件夹.exe";
        let folder = directory.path().join(name);
        fs::create_dir(&folder).unwrap();
        let application = directory.path().join("Velo 应用.exe");
        fs::write(&application, b"never execute").unwrap();
        manage(&runtime, Manage::Import(vec![application.clone()])).unwrap();
        let app_id = runtime
            .snapshot()
            .unwrap()
            .unwrap()
            .library
            .applications
            .as_ref()
            .unwrap()[0]
            .id
            .clone();
        manage(&runtime, Manage::Pin(app_id.clone(), true)).unwrap();
        assert!(manage(&runtime, Manage::SetFolderPinned(application, true)).is_err());
        assert!(manage(&runtime, Manage::SetFolderPinned(PathBuf::from(name), true)).is_err());

        let library_path = directory.path().join("application-library.json");
        let lock = fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .open(&library_path)
            .unwrap();
        let before = runtime.snapshot().unwrap().unwrap();
        assert!(manage(&runtime, Manage::SetFolderPinned(folder.clone(), true)).is_err());
        assert!(Arc::ptr_eq(&before, &runtime.snapshot().unwrap().unwrap()));
        drop(lock);

        manage(&runtime, Manage::SetFolderPinned(folder.clone(), true)).unwrap();
        let alternate = PathBuf::from(format!(
            "{}/./VELO 文件夹.EXE/",
            directory.path().to_string_lossy().replace('\\', "/")
        ));
        let folder_id = library::folder_id(&folder).unwrap();
        assert_eq!(library::folder_id(&alternate).unwrap(), folder_id);
        manage(&runtime, Manage::SetFolderPinned(alternate.clone(), true)).unwrap();

        let restarted = shared(directory.path());
        initialize(&restarted).unwrap();
        let snapshot = restarted.snapshot().unwrap().unwrap();
        let pins = snapshot
            .catalog
            .home(&snapshot.history, now())
            .unwrap()
            .pinned;
        assert_eq!(
            pins.iter().map(|item| item.id.as_str()).collect::<Vec<_>>(),
            [app_id.as_str(), folder_id.as_str()]
        );
        let item = &pins[1];
        assert_eq!(item.name, name);
        assert!(item.is_folder() && item.pinned);
        assert_eq!(snapshot.catalog.len(), 1);
        assert!(!snapshot.library.has_custom_app(&folder_id));
        assert_eq!(
            snapshot
                .catalog
                .search(name, &snapshot.config, &snapshot.history, now())
                .unwrap()[0]
                .item
                .id,
            folder_id
        );
        assert!(
            snapshot
                .catalog
                .search_applications(name, &snapshot.config, &snapshot.history, now())
                .unwrap()
                .iter()
                .all(|result| !result.item.is_folder())
        );
        assert_eq!(super::super::launch::capabilities(item), (false, false));
        let command =
            super::super::launch::command(item, Action::Launch, directory.path(), |path| {
                fs::metadata(path).map(|metadata| metadata.is_dir())
            })
            .unwrap();
        assert_eq!(Path::new(&command.path), folder);
        assert!(command.arguments.is_empty());

        manage(&restarted, Manage::SetFolderPinned(alternate, false)).unwrap();
        assert!(folder.is_dir());
        assert!(
            restarted
                .snapshot()
                .unwrap()
                .unwrap()
                .catalog
                .item(&folder_id)
                .is_none()
        );
        manage(&restarted, Manage::SetFolderPinned(folder.clone(), true)).unwrap();
        fs::remove_dir(&folder).unwrap();
        fs::write(&folder, b"regular file at the former folder path").unwrap();
        let request = LaunchRequest {
            id: folder_id.clone(),
            query: name.into(),
            action: Action::Launch,
            owner: 0,
        };
        assert_eq!(
            run_action(&restarted, &request),
            (Err(launch::Failure::MissingTarget), true)
        );
        manage(&restarted, Manage::Pin(folder_id.clone(), false)).unwrap();
        let snapshot = restarted.snapshot().unwrap().unwrap();
        assert!(snapshot.catalog.item(&folder_id).is_none());
        assert!(snapshot.library.has_pin(&app_id));
        assert!(folder.is_file());
    }

    #[test]
    fn imported_folders_remain_searchable_when_unpinned() {
        let directory = tempfile::tempdir().unwrap();
        let runtime = shared(directory.path());
        initialize(&runtime).unwrap();
        let folder = directory.path().join("Velo 自定义目录.lnk");
        let application = directory.path().join("Velo 自定义程序.exe");
        let unsupported = directory.path().join("notes.txt");
        fs::create_dir(&folder).unwrap();
        fs::write(&application, b"never execute").unwrap();
        fs::write(&unsupported, b"unsupported regular file").unwrap();
        assert_eq!(
            manage(
                &runtime,
                Manage::Import(vec![
                    application.clone(),
                    folder.clone(),
                    folder.clone(),
                    unsupported
                ])
            )
            .unwrap(),
            "已添加 2 个项目 · 1 个项目无法导入"
        );
        let id = library::folder_id(&folder).unwrap();
        let imported = runtime.snapshot().unwrap().unwrap();
        assert!(imported.library.has_custom_app(&id));
        assert!(!imported.catalog.item(&id).unwrap().pinned);

        manage(&runtime, Manage::Pin(id.clone(), true)).unwrap();
        manage(&runtime, Manage::SetFolderPinned(folder.clone(), true)).unwrap();
        let pinned = runtime.snapshot().unwrap().unwrap();
        assert_eq!(pinned.library.pinned.as_ref().unwrap().len(), 1);
        assert!(pinned.catalog.item(&id).unwrap().pinned);
        manage(&runtime, Manage::SetFolderPinned(folder.clone(), false)).unwrap();

        let restarted = shared(directory.path());
        initialize(&restarted).unwrap();
        let snapshot = restarted.snapshot().unwrap().unwrap();
        assert!(!snapshot.library.has_pin(&id));
        let results = snapshot
            .catalog
            .search(
                "Velo 自定义目录.lnk",
                &snapshot.config,
                &snapshot.history,
                now(),
            )
            .unwrap();
        assert_eq!(results[0].item.id, id);
        assert!(results[0].item.is_folder());
        manage(&restarted, Manage::Pin(id.clone(), true)).unwrap();
        manage(&restarted, Manage::Delete(id.clone())).unwrap();
        let deleted = restarted.snapshot().unwrap().unwrap();
        assert!(deleted.catalog.item(&id).is_none());
        assert!(!deleted.library.has_pin(&id));
        assert!(folder.is_dir() && application.is_file());
    }

    #[test]
    fn deleting_missing_custom_apps_removes_pins_and_search_results() {
        for keep_pin in [true, false] {
            let directory = tempfile::tempdir().unwrap();
            let shared = shared(directory.path());
            initialize(&shared).unwrap();
            let name = "VeloMissingCustomFixture";
            let path = directory.path().join(format!("{name}.exe"));
            fs::write(&path, b"never execute").unwrap();
            manage(&shared, Manage::Import(vec![path.clone()])).unwrap();
            let snapshot = shared.snapshot().unwrap().unwrap();
            let id = snapshot.library.applications.as_ref().unwrap()[0]
                .id
                .clone();
            manage(&shared, Manage::Pin(id.clone(), true)).unwrap();
            if !keep_pin {
                manage(&shared, Manage::Pin(id.clone(), false)).unwrap();
            }
            fs::remove_file(path).unwrap();
            let request = LaunchRequest {
                id: id.clone(),
                query: name.into(),
                action: Action::Launch,
                owner: 0,
            };
            assert_eq!(
                run_action(&shared, &request),
                (Err(launch::Failure::MissingTarget), true)
            );
            let before = shared.snapshot().unwrap().unwrap();
            assert!(before.library.has_custom_app(&id));
            assert_eq!(before.catalog.item(&id).unwrap().pinned, keep_pin);
            assert!(
                before
                    .catalog
                    .search(name, &before.config, &before.history, now())
                    .unwrap()
                    .iter()
                    .any(|result| result.item.id == id)
            );

            manage(&shared, Manage::Delete(id.clone())).unwrap();
            let after = shared.snapshot().unwrap().unwrap();
            assert!(after.catalog.item(&id).is_none());
            assert!(
                after
                    .catalog
                    .search(name, &after.config, &after.history, now())
                    .unwrap()
                    .iter()
                    .all(|result| result.item.id != id)
            );
            let restarted = DataStore::open(directory.path()).unwrap();
            let library = restarted.library.snapshot().unwrap();
            assert!(!library.has_custom_app(&id));
            assert!(library.pinned.as_ref().unwrap().is_empty());
        }
    }

    #[test]
    fn backup_preserves_disk_original_and_failure_prevents_any_update_script() {
        let directory = tempfile::tempdir().unwrap();
        let shared = shared(directory.path());
        let bytes = b" { \"version\": 1, \"extra\": [12345678901234567890123456789] }\r\n";
        fs::write(directory.path().join("config.json"), bytes).unwrap();
        initialize(&shared).unwrap();
        let backup = manage(&shared, Manage::Backup).unwrap();
        assert_eq!(fs::read(backup).unwrap(), bytes);
        let lock = fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(directory.path().join("config.json"))
            .unwrap();
        assert!(prepare_update(&shared, Path::new("verified.exe"), true, &"a".repeat(64)).is_err());
        assert!(!directory.path().join("updates").exists());
        drop(lock);
    }
}
