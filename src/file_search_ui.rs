//! UI adapter for optional, session-only file search. Disk and IPC work stays
//! on the bounded runtime workers; this controller only owns visible file state.
use crate::business::owner;
use crate::platform::windows::interaction;
use crate::{AppWindow, FileEntry, navigation};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use velo_slint::core::config::Everything;
use velo_slint::core::file_search::{self, FileHit, Status as FileStatus};
use velo_slint::core::service::{Event, Runtime};
pub struct Controller {
    app: slint::Weak<AppWindow>,
    runtime: Rc<RefCell<Runtime>>,
    file_generation: Cell<u64>,
    file_offset: Cell<u32>,
    file_paths: RefCell<Vec<FileHit>>,
    file_status: Cell<FileStatus>,
    everything_enabled: Cell<bool>,
    app_selected: RefCell<slint::SharedString>,
    file_selected: RefCell<slint::SharedString>,
    file_restore_count: Cell<usize>,
    file_last_query: RefCell<String>,
}
impl Controller {
    pub fn connect(app: &AppWindow, runtime: Rc<RefCell<Runtime>>) -> Rc<Self> {
        let controller = Rc::new(Self {
            app: app.as_weak(),
            runtime,
            file_generation: Cell::new(0),
            file_offset: Cell::new(0),
            file_paths: RefCell::new(Vec::new()),
            file_status: Cell::new(FileStatus::Disabled),
            everything_enabled: Cell::new(false),
            app_selected: RefCell::default(),
            file_selected: RefCell::default(),
            file_restore_count: Cell::new(file_search::PAGE_SIZE as usize),
            file_last_query: RefCell::default(),
        });
        let weak = Rc::downgrade(&controller);
        app.on_file_mode_changed(move |enabled| {
            if let Some(controller) = weak.upgrade()
                && let Some(app) = controller.app.upgrade()
            {
                if !enabled {
                    controller.remember_file_view(&app);
                }
                controller.suspend_file_search(&app);
                let enabled = enabled && controller.everything_enabled.get();
                app.set_selection_notice("".into());
                if enabled {
                    *controller.app_selected.borrow_mut() = app.get_selected_id();
                    controller.resume_file_search(&app);
                } else {
                    app.set_file_status("".into());
                    let selected = controller.app_selected.borrow().clone();
                    app.set_selected_id(selected.clone());
                    app.invoke_input_edited(app.get_input_text());
                    let models = [
                        app.get_pinned_items(),
                        app.get_frequent_items(),
                        app.get_system_items(),
                        app.get_result_items(),
                    ];
                    if models
                        .iter()
                        .any(|model| model.iter().any(|row| row.id == selected))
                    {
                        app.set_selected_id(selected);
                    }
                }
            }
        });
        let weak = Rc::downgrade(&controller);
        app.on_file_load_more(move || {
            if let Some(controller) = weak.upgrade()
                && let Some(app) = controller.app.upgrade()
            {
                controller.queue_file_search(
                    app.get_input_text().to_string(),
                    controller.file_offset.get(),
                );
            }
        });
        let weak = Rc::downgrade(&controller);
        app.on_file_chosen(move |id, locate| {
            if let Some(controller) = weak.upgrade() {
                controller.open_file(&id, locate);
            }
        });
        let weak = Rc::downgrade(&controller);
        app.on_file_context_menu(move |path| {
            if let Some(controller) = weak.upgrade() {
                controller.file_menu(&path);
            }
        });
        let weak = Rc::downgrade(&controller);
        app.on_file_menu_completed(move |path, action| {
            if let Some(controller) = weak.upgrade()
                && let Some(app) = controller.app.upgrade()
            {
                app.set_native_dialog_active(false);
                match action {
                    1 => controller.open_file(&path, false),
                    2 => controller.open_file(&path, true),
                    3 => app.set_selection_notice("已复制完整路径".into()),
                    -1 => app.set_selection_notice("无法打开菜单或复制路径，请重试。".into()),
                    _ => {}
                }
                app.invoke_native_interaction_finished();
            }
        });
        let weak = Rc::downgrade(&controller);
        app.on_file_start_requested(move || {
            if let Some(controller) = weak.upgrade() {
                controller.start_everything();
            }
        });
        let weak = Rc::downgrade(&controller);
        app.on_file_install_requested(move || {
            if let Some(controller) = weak.upgrade() {
                controller.open_everything_site();
            }
        });
        let weak = Rc::downgrade(&controller);
        app.on_file_browse_requested(move || {
            if let Some(controller) = weak.upgrade() {
                controller.browse_everything();
            }
        });
        let weak = Rc::downgrade(&controller);
        app.on_file_browse_completed(move |path, error| {
            if let Some(controller) = weak.upgrade()
                && let Some(app) = controller.app.upgrade()
            {
                app.set_native_dialog_active(false);
                if !path.is_empty() {
                    let mut draft = app.get_settings_draft();
                    draft.everything_executable = path;
                    app.set_settings_draft(draft);
                }
                if !error.is_empty() {
                    app.set_settings_notice(error);
                }
                app.invoke_native_interaction_finished();
            }
        });
        let weak = Rc::downgrade(&controller);
        app.on_file_presentation_changed(move |presented| {
            if let Some(controller) = weak.upgrade()
                && let Some(app) = controller.app.upgrade()
                && app.get_file_mode()
            {
                if presented {
                    controller.resume_file_search(&app);
                } else {
                    controller.remember_file_view(&app);
                    controller.suspend_file_search(&app);
                }
            }
        });

        controller
    }
    pub fn configure(&self, config: &Everything) {
        self.everything_enabled.set(config.enabled);
        self.runtime
            .borrow_mut()
            .configure_file_search(config.enabled);
        if let Some(app) = self.app.upgrade() {
            app.set_everything_enabled(config.enabled);
            app.set_everything_executable(config.executable.as_str().into());
            if !config.enabled && app.get_file_mode() {
                app.set_file_mode(false);
            }
        }
    }
    pub fn input(&self, query: &str) {
        self.queue_file_search(query.to_owned(), 0);
    }

    fn remember_file_view(&self, app: &AppWindow) {
        // Don't shrink the saved range if the user leaves again mid-restoration.
        let loaded = self.file_paths.borrow().len();
        self.file_restore_count
            .set(loaded.max(self.file_restore_count.get()));
        *self.file_selected.borrow_mut() = app.get_selected_id();
    }

    fn suspend_file_search(&self, app: &AppWindow) {
        self.file_generation
            .set(self.file_generation.get().wrapping_add(1));
        self.runtime.borrow().cancel_file_search();
        self.file_offset.set(0);
        self.file_paths.borrow_mut().clear();
        app.set_file_items(ModelRc::default());
        app.set_file_total(0);
        app.set_file_loading(false);
        app.set_file_retryable(false);
    }

    fn resume_file_search(&self, app: &AppWindow) {
        app.set_selected_id(self.file_selected.borrow().clone());
        self.queue_file_search(app.get_input_text().to_string(), 0);
    }

    pub fn navigate(&self, app: &AppWindow, direction: navigation::Direction) {
        let paths = self.file_paths.borrow();
        let rows = navigation::choice_rows(&[paths.len(), 0, 0], 1);
        let current = paths
            .iter()
            .position(|item| item.path == app.get_selected_id().as_str());
        if let Some(index) = navigation::navigate(&rows, current, direction)
            && let Some(item) = paths.get(index)
        {
            app.set_selected_id(item.path.as_str().into());
            app.invoke_ensure_selection_visible(index as i32);
        }
    }
    pub fn receive(&self, event: Event) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        match event {
            Event::FileSearch {
                generation,
                query,
                offset,
                result,
            } => {
                if generation != self.file_generation.get()
                    || !app.get_file_mode()
                    || !app.get_presented()
                    || !app.get_file_active()
                    || query != app.get_input_text().as_str()
                {
                    return;
                }
                match result {
                    Ok((hits, total)) => {
                        let mut paths = self.file_paths.borrow_mut();
                        if offset == 0 {
                            paths.clear();
                        } else if offset != self.file_offset.get() {
                            return;
                        }
                        let next_offset = offset.saturating_add(hits.len() as u32);
                        self.file_offset.set(next_offset);
                        let remaining = file_search::MAX_RESULTS.saturating_sub(paths.len());
                        for hit in hits.into_iter().take(remaining) {
                            if !paths.iter().any(|old| old.path == hit.path) {
                                paths.push(hit);
                            }
                        }
                        let status = if paths.is_empty() {
                            FileStatus::Empty
                        } else {
                            FileStatus::Ready
                        };
                        self.file_status.set(status);
                        app.set_file_status(status.message(false).into());
                        app.set_file_loading(false);
                        app.set_file_retryable(false);
                        self.update_file_rows(&app, &paths, total);
                        if paths.len() < self.file_restore_count.get()
                            && next_offset < total as u32
                            && next_offset < file_search::MAX_RESULTS as u32
                            && next_offset > offset
                        {
                            self.queue_file_search(query, next_offset);
                        }
                    }
                    Err(error) => {
                        self.file_paths.borrow_mut().clear();
                        let status = match error {
                            velo_slint::platform::windows::everything::Error::NotRunning => {
                                FileStatus::NotRunning
                            }
                            velo_slint::platform::windows::everything::Error::NotIndexed => {
                                FileStatus::NotIndexed
                            }
                            velo_slint::platform::windows::everything::Error::Incompatible => {
                                FileStatus::Incompatible
                            }
                            velo_slint::platform::windows::everything::Error::Timeout => {
                                FileStatus::Timeout
                            }
                            velo_slint::platform::windows::everything::Error::Protocol => {
                                FileStatus::Protocol
                            }
                            _ => FileStatus::Error,
                        };
                        self.file_status.set(status);
                        app.set_file_loading(false);
                        app.set_file_retryable(true);
                        app.set_file_status(
                            status
                                .message(!app.get_everything_executable().is_empty())
                                .into(),
                        );
                        self.update_file_rows(&app, &[], 0);
                    }
                }
            }
            Event::FileOpened {
                generation,
                locate,
                result,
            } => {
                app.set_native_dialog_active(false);
                if generation != self.file_generation.get() {
                    return;
                }
                match result {
                    Ok(()) => {
                        app.invoke_clear_input();
                        app.set_file_mode(false);
                        app.set_file_items(ModelRc::default());
                        app.set_file_total(0);
                        app.set_file_status("".into());
                        app.invoke_launch_started();
                        app.invoke_launch_finished(true);
                    }
                    Err(error) => {
                        app.set_file_status(
                            if locate {
                                "Windows 无法在资源管理器中显示该文件；它可能已被移动或删除。"
                            } else {
                                match error {
                                    velo_slint::platform::windows::launch::Failure::Unavailable => {
                                        "文件已移动或删除，请重试搜索。"
                                    }
                                    velo_slint::platform::windows::launch::Failure::Cancelled => {
                                        "已取消打开文件。"
                                    }
                                    velo_slint::platform::windows::launch::Failure::Shell => {
                                        "Windows 无法打开该文件，请检查权限或默认文件关联。"
                                    }
                                }
                            }
                            .into(),
                        );
                        app.set_selection_notice(app.get_file_status());
                        app.invoke_focus_input();
                    }
                }
            }
            Event::FileUtility { kind, success } => {
                if kind == "start" {
                    app.set_file_status(
                        if success {
                            "Everything 已启动，请重试搜索。"
                        } else {
                            "未找到 Everything。请安装或选择便携版程序。"
                        }
                        .into(),
                    );
                } else if !success {
                    app.set_file_status(
                        "无法打开浏览器，请访问 www.voidtools.com/downloads/。".into(),
                    );
                }
            }

            _ => {}
        }
    }
    fn queue_file_search(&self, query: String, offset: u32) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        if !app.get_everything_enabled() {
            return;
        }
        if *self.file_last_query.borrow() != query {
            self.file_restore_count.set(file_search::PAGE_SIZE as usize);
            *self.file_last_query.borrow_mut() = query.clone();
            *self.file_selected.borrow_mut() = "".into();
            app.set_selected_id("".into());
        }
        if !app.get_presented()
            || app.get_appearance_open()
            || app.get_import_open()
            || app.get_groups_open()
        {
            return;
        }
        if query.is_empty() {
            self.suspend_file_search(&app);
            app.set_file_status("输入名称，查找本机文件".into());
            return;
        }
        if offset == 0 {
            self.file_generation
                .set(self.file_generation.get().wrapping_add(1));
            self.file_paths.borrow_mut().clear();
            self.file_offset.set(0);
            app.set_file_items(ModelRc::default());
            app.set_file_total(0);
        }
        if app.get_file_loading() && offset > 0 {
            return;
        }
        self.file_status.set(FileStatus::Searching);
        app.set_file_loading(true);
        app.set_file_retryable(false);
        app.set_file_status("正在搜索文件…".into());
        if !self
            .runtime
            .borrow_mut()
            .file_search(query, self.file_generation.get(), offset)
        {
            self.file_status.set(FileStatus::Error);
            app.set_file_loading(false);
            app.set_file_retryable(true);
            app.set_file_status("无法启动 Everything 查询，请稍后重试。".into());
        }
    }

    fn update_file_rows(&self, app: &AppWindow, paths: &[FileHit], total: usize) {
        let rows = paths
            .iter()
            .map(|hit| {
                let name = hit.name();
                FileEntry {
                    path: hit.path.as_str().into(),
                    name: name.into(),
                    directory: hit.parent().into(),
                    kind: hit.extension().to_uppercase().into(),
                    folder: hit.is_folder,
                }
            })
            .collect::<Vec<_>>();
        app.set_file_items(Rc::new(VecModel::from(rows)).into());
        app.set_file_total(total.min(i32::MAX as usize) as i32);
        if !paths.is_empty()
            && !paths
                .iter()
                .any(|hit| hit.path == app.get_selected_id().as_str())
            && (paths.len() >= self.file_restore_count.get()
                || self.file_offset.get() as usize >= total)
        {
            app.set_selected_id(paths[0].path.as_str().into());
        }
        if paths.len() == file_search::MAX_RESULTS && total > file_search::MAX_RESULTS {
            app.set_selection_notice("已显示前 200 项，请缩小搜索范围。".into());
        } else {
            app.set_selection_notice("".into());
        }
        app.invoke_layout_changed();
        if let Some(index) = paths
            .iter()
            .position(|hit| hit.path == app.get_selected_id().as_str())
        {
            app.invoke_ensure_selection_visible(index as i32);
        }
    }

    fn open_file(&self, path: &str, locate: bool) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        if !app.get_presented()
            || app.get_native_dialog_active()
            || !app.get_file_mode()
            || !app.get_everything_enabled()
            || !self.file_paths.borrow().iter().any(|hit| hit.path == path)
        {
            return;
        }
        let Some(owner) = owner(&app) else {
            app.set_file_status("Windows 窗口尚未就绪，请重试。".into());
            return;
        };
        app.set_native_dialog_active(true);
        app.invoke_suspend_input();
        if !self
            .runtime
            .borrow()
            .open_file(path.into(), self.file_generation.get(), locate, owner)
        {
            app.set_native_dialog_active(false);
            app.invoke_focus_input();
            app.set_file_status("其他操作正在进行，请稍后重试。".into());
        }
    }

    fn file_menu(self: &Rc<Self>, path: &str) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        if !app.get_presented() || app.get_native_dialog_active() || app.get_management_busy() {
            return;
        }
        if !app.get_file_mode() || !self.file_paths.borrow().iter().any(|hit| hit.path == path) {
            return;
        }
        let Some(owner) = owner(&app) else {
            return;
        };
        app.set_native_dialog_active(true);
        app.invoke_suspend_input();
        let weak = app.as_weak();
        let path = path.to_owned();
        if std::thread::Builder::new()
            .name("velo-file-menu".into())
            .spawn(move || {
                let action = interaction::file_menu(owner, &path);
                let _ = weak.upgrade_in_event_loop(move |app| {
                    app.invoke_file_menu_completed(path.into(), action)
                });
            })
            .is_err()
        {
            app.set_native_dialog_active(false);
            app.set_file_status("无法打开文件菜单，请重试。".into());
            app.invoke_native_interaction_finished();
        }
    }

    fn browse_everything(&self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        if !app.get_presented() || app.get_native_dialog_active() || !app.get_appearance_open() {
            return;
        }
        let Some(owner) = owner(&app) else {
            return;
        };
        app.set_native_dialog_active(true);
        app.invoke_suspend_input();
        let weak = app.as_weak();
        if std::thread::Builder::new()
            .name("velo-everything-picker".into())
            .spawn(move || {
                let result = interaction::run(owner, interaction::BROWSE_EXECUTABLE);
                let path = result
                    .paths
                    .first()
                    .map_or(String::new(), |path| path.to_string_lossy().into_owned());
                let _ = weak.upgrade_in_event_loop(move |app| {
                    app.invoke_file_browse_completed(path.into(), result.error.into());
                });
            })
            .is_err()
        {
            app.set_native_dialog_active(false);
            app.set_settings_notice("无法打开文件选择器，请输入完整程序路径。".into());
            app.invoke_native_interaction_finished();
        }
    }

    fn start_everything(&self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let executable = app.get_everything_executable().to_string();
        if self.runtime.borrow().file_utility(true, executable) {
            app.set_file_status("正在启动 Everything…".into());
        } else {
            app.set_file_status("其他操作正在进行，请稍后重试。".into());
        }
    }
    fn open_everything_site(&self) {
        if !self.runtime.borrow().file_utility(false, String::new())
            && let Some(app) = self.app.upgrade()
        {
            app.set_file_status("其他操作正在进行，请稍后重试。".into());
        }
    }
}
