//! The UI owns only the current snapshot, display rows and bounded image LRU.

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use crate::platform::windows::{interaction, preferences};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::winit_030::WinitWindowAccessor;
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use velo_slint::core::config::Config;
use velo_slint::core::launch::{APPS_FOLDER_PREFIX, Action};
use velo_slint::core::model::AppItem;
use velo_slint::core::service::{
    Event, IconTarget, InstallProgress, Manage, Options, Runtime, Snapshot, WorkspaceUserRequest,
};
use velo_slint::core::system_tools::{Prompt as SystemPrompt, needs_confirmation};
use velo_slint::core::update::DownloadProgress;
use velo_slint::core::workspace::{
    actions::{Success as WorkspaceSuccess, WorkspaceAction},
    config::WorkspaceConfig,
};
use velo_slint::core::{icons, normalize::normalize, service};
use velo_slint::platform::windows::launch::Failure as LaunchFailure;
use velo_slint::platform::windows::{workspace::Foreground, workspace_actions::Application};

use crate::{AppWindow, Confirmation, LauncherItem, SettingsDraft, confirmations, navigation, ui};

pub struct Controller {
    groups: RefCell<Option<Rc<crate::group_editor::Controller>>>,
    app: slint::Weak<AppWindow>,
    confirmations: Rc<confirmations::Controller>,
    runtime: Rc<RefCell<Runtime>>,
    snapshot: RefCell<Option<Arc<Snapshot>>>,
    choices: RefCell<Vec<Arc<AppItem>>>,
    lengths: Cell<[usize; 3]>,
    images: RefCell<icons::Lru<slint::Image>>,
    requested_icons: RefCell<Vec<String>>,
    viewport_queued: Cell<bool>,
    dirty: Cell<bool>,
    diagnostics: bool,
    configured: Cell<bool>,
    hotkey_override: bool,
    observer: Option<crate::measurement::Observer>,
    files: Rc<crate::file_search_ui::Controller>,
}

impl Controller {
    pub(crate) fn enabled_workspace(&self) -> Option<Arc<WorkspaceConfig>> {
        self.snapshot.borrow().as_ref()?.enabled_workspace.clone()
    }

    pub(crate) fn workspace_config(&self) -> Option<Arc<WorkspaceConfig>> {
        let snapshot = self.snapshot.borrow();
        let snapshot = snapshot.as_ref()?;
        snapshot.enabled_workspace.clone().or_else(|| {
            // A disabled layout is needed only when editing it or using the explicit preview.
            WorkspaceConfig::read(&snapshot.config).ok().map(Arc::new)
        })
    }

    pub(crate) fn workspace_action(
        &self,
        action: WorkspaceAction,
        target: Option<(Foreground, Application)>,
        owner: isize,
    ) -> bool {
        self.runtime.borrow().workspace(action, target, owner)
    }

    pub(crate) fn save_workspace(&self, config: WorkspaceConfig) -> bool {
        self.runtime
            .borrow()
            .manage(Manage::SaveWorkspace(Box::new(config)))
    }

    pub(crate) fn workspace_user_action(&self, request: WorkspaceUserRequest) -> bool {
        self.runtime.borrow_mut().workspace_user(request)
    }

    pub fn connect(
        app: &AppWindow,
        confirmations: Rc<confirmations::Controller>,
        directory: &Path,
        diagnostics: bool,
        options: Option<Options>,
        hotkey_override: bool,
        observer: Option<crate::measurement::Observer>,
    ) -> velo_slint::core::Result<Rc<Self>> {
        let weak_app = app.as_weak();
        let runtime = Runtime::start_with_options(directory, options, move || {
            let _ = weak_app.upgrade_in_event_loop(|app| app.invoke_backend_changed());
        })?;
        let runtime = Rc::new(RefCell::new(runtime));
        let files = crate::file_search_ui::Controller::connect(app, runtime.clone());
        let controller = Rc::new(Self {
            groups: RefCell::new(None),
            app: app.as_weak(),
            confirmations,
            runtime,
            files,
            snapshot: RefCell::new(None),
            choices: RefCell::default(),
            lengths: Cell::new([0; 3]),
            images: RefCell::new(icons::Lru::new(icons::BUDGET - icons::TRANSFER_BUDGET)),
            requested_icons: RefCell::default(),
            viewport_queued: Cell::new(false),
            dirty: Cell::new(false),
            diagnostics,
            configured: Cell::new(false),
            hotkey_override,
            observer,
        });
        app.set_business_enabled(true);
        app.set_business_status("正在读取应用索引…".into());
        app.set_data_directory(directory.to_string_lossy().into_owned().into());
        app.set_third_party_licenses(
            Rc::new(VecModel::from(vec![
                crate::LicenseEntry {
                    title: "界面图标采用 Microsoft Fluent UI System Icons（MIT License）。".into(),
                    text: include_str!("../ui/assets/fluent/LICENSE.txt").into(),
                },
                crate::LicenseEntry {
                    title: "拼音检索数据来源：go-pinyin（MIT License）。".into(),
                    text: include_str!("../licenses/go-pinyin-MIT.txt").into(),
                },
                crate::LicenseEntry {
                    title: "Unicode 数据源自 Go（BSD License）。".into(),
                    text: include_str!("../licenses/Go-BSD.txt").into(),
                },
                crate::LicenseEntry {
                    title: "Everything 本机 IPC 协议参考官方 SDK（MIT License）。".into(),
                    text: include_str!("../licenses/Everything-SDK-MIT.txt").into(),
                },
            ]))
            .into(),
        );
        let weak = Rc::downgrade(&controller);
        app.on_backend_changed(move || {
            if let Some(controller) = weak.upgrade() {
                controller.receive();
            }
        });
        let weak = Rc::downgrade(&controller);
        app.on_input_edited(move |_| {
            if let Some(controller) = weak.upgrade() {
                if let Some(app) = controller.app.upgrade() {
                    if app.get_file_mode() {
                        controller.files.input(&app.get_input_text());
                    } else {
                        controller.render(false);
                    }
                }
                if let (Some(observer), Some(app)) =
                    (&controller.observer, controller.app.upgrade())
                {
                    observer.input(&app, app.get_input_text().len());
                }
            }
        });
        let weak = Rc::downgrade(&controller);
        app.on_item_chosen(move |id| {
            if let Some(controller) = weak.upgrade() {
                controller.choose(&id, Action::Launch);
            }
        });
        let weak = Rc::downgrade(&controller);
        app.on_item_pin_toggled(move |id| {
            if let Some(controller) = weak.upgrade() {
                controller.toggle_pin(&id);
            }
        });
        let weak = Rc::downgrade(&controller);
        app.on_settings_requested(move || {
            if let Some(controller) = weak.upgrade() {
                controller.settings();
            }
        });
        let weak = Rc::downgrade(&controller);
        app.on_system_expansion_changed(move || {
            if let Some(controller) = weak.upgrade() {
                controller.render(true);
            }
        });
        let weak = Rc::downgrade(&controller);
        app.on_settings_save(move || {
            if let Some(controller) = weak.upgrade() {
                controller.save();
            }
        });
        let weak = Rc::downgrade(&controller);
        app.on_application_menu(move |id| {
            if let Some(controller) = weak.upgrade() {
                controller.application_menu(&id);
            }
        });
        let weak = Rc::downgrade(&controller);
        app.on_application_delete(move |id| {
            if let Some(controller) = weak.upgrade() {
                controller.confirm_removal(&id, false);
            }
        });
        let weak = Rc::downgrade(&controller);
        app.on_application_menu_completed(move |id, action| {
            if let Some(controller) = weak.upgrade() {
                controller.application_menu_completed(&id, action);
            }
        });
        let weak = Rc::downgrade(&controller);
        app.on_import_commit(move || {
            if let Some(controller) = weak.upgrade()
                && let Some(app) = controller.app.upgrade()
            {
                let paths = app
                    .get_preview_entries()
                    .iter()
                    .filter(|e| e.valid)
                    .map(|e| e.path.as_str().into())
                    .collect();
                controller.manage(Manage::Import(paths));
            }
        });
        let weak = Rc::downgrade(&controller);
        app.on_backup_requested(move |open| {
            if let Some(controller) = weak.upgrade()
                && let Some(app) = controller.app.upgrade()
            {
                if open {
                    if let Some(owner) = owner(&app) {
                        controller.manage(Manage::OpenBackups(owner));
                    }
                } else {
                    controller.manage(Manage::Backup);
                }
            }
        });
        let weak = Rc::downgrade(&controller);
        app.on_update_requested(move |install| {
            if let Some(controller) = weak.upgrade() {
                if install {
                    controller.confirm_update();
                } else {
                    controller.update(false);
                }
            }
        });
        let weak = Rc::downgrade(&controller);
        app.on_environment_changed(move || {
            if let Some(controller) = weak.upgrade()
                && let Some(snapshot) = controller.snapshot.borrow().as_ref()
            {
                controller.preferences(&snapshot.config);
            }
        });
        let weak = Rc::downgrade(&controller);
        app.on_refresh_requested(move || {
            if let Some(controller) = weak.upgrade() {
                controller.runtime.borrow().refresh();
            }
        });
        let weak = Rc::downgrade(&controller);
        app.on_icon_viewport_changed(move || {
            if let Some(controller) = weak.upgrade() {
                controller.queue_viewport();
            }
        });
        let weak = Rc::downgrade(&controller);
        ui::connect_commands(app, move |app, home, direction| {
            let Some(controller) = weak.upgrade() else {
                return;
            };
            if app.get_file_mode() {
                controller.files.navigate(app, direction);
                return;
            }
            let choices = controller.choices.borrow();
            let lengths = if app.get_is_home() {
                controller.lengths.get()
            } else {
                [choices.len(), 0, 0]
            };
            let rows = navigation::choice_rows(&lengths, if home { 6 } else { 1 });
            let current = choices
                .iter()
                .position(|item| item.id == app.get_selected_id().as_str());
            if let Some(index) = navigation::navigate(&rows, current, direction) {
                app.set_selected_id(choices[index].id.as_str().into());
                app.invoke_ensure_selection_visible(index as i32);
            }
        });
        let weak = Rc::downgrade(&controller);
        *controller.groups.borrow_mut() = Some(crate::group_editor::Controller::connect(
            app,
            controller.confirmations.clone(),
            move |command| {
                let Some(controller) = weak.upgrade() else {
                    return false;
                };
                match command {
                    crate::group_editor::Command::Save(group) => {
                        controller.runtime.borrow().manage(Manage::SaveGroup(group))
                    }
                    crate::group_editor::Command::Delete(id) => {
                        controller.runtime.borrow().manage(Manage::DeleteGroup(id))
                    }
                    crate::group_editor::Command::Detection(ticket, path) => {
                        controller.runtime.borrow().group_detection(ticket, path)
                    }
                    crate::group_editor::Command::Run { id, only, force } => {
                        controller.launch_group(id, only, force)
                    }
                    crate::group_editor::Command::Cancel => {
                        controller.runtime.borrow().cancel_group();
                        true
                    }
                }
            },
        ));
        Ok(controller)
    }

    fn receive(self: &Rc<Self>) {
        let events = self.runtime.borrow().drain();
        let Some(app) = self.app.upgrade() else {
            return;
        };
        for event in events {
            match event {
                Event::GroupDetection { ticket, result } => {
                    if let Some(editor) = self.groups.borrow().as_ref() {
                        editor.detection_ready(ticket, result);
                    }
                }
                Event::GroupRun(report) => {
                    if report.done {
                        app.invoke_launch_finished(report.accepted());
                        app.set_selection_notice(report.summary().into());
                    }
                    if let Some(editor) = self.groups.borrow().as_ref() {
                        editor.report(*report);
                    }
                }
                Event::Snapshot(snapshot) => {
                    if let Some(editor) = self.groups.borrow().as_ref() {
                        editor.snapshot(snapshot.clone());
                    }
                    if let Some(observer) = &self.observer {
                        observer.event(
                            "catalog_published",
                            &format!("count={}", snapshot.catalog.len()),
                        );
                    }
                    app.set_business_revision(app.get_business_revision().wrapping_add(1));
                    if self.diagnostics {
                        eprintln!("event=catalog_published count={}", snapshot.catalog.len());
                    }
                    let first = !self.configured.replace(true);
                    let config_changed = self
                        .snapshot
                        .borrow()
                        .as_ref()
                        .is_none_or(|old| old.config != snapshot.config);
                    if first {
                        let hotkey = if self.hotkey_override {
                            app.get_hotkey_draft()
                        } else {
                            snapshot.config.hotkey.as_str().into()
                        };
                        app.invoke_hotkey_apply(hotkey);
                    } else if self
                        .snapshot
                        .borrow()
                        .as_ref()
                        .is_some_and(|old| old.config.hotkey != snapshot.config.hotkey)
                    {
                        app.invoke_hotkey_committed(snapshot.config.hotkey.as_str().into());
                    }
                    if config_changed {
                        self.preferences(&snapshot.config);
                    }
                    app.set_grid_results(snapshot.config.result_layout == "grid");
                    self.files.configure(&snapshot.config.everything);
                    app.set_space_launch(snapshot.config.space_launch);
                    app.set_disable_hotkey_fullscreen(snapshot.config.disable_hotkey_fullscreen);
                    app.set_custom_items(
                        Rc::new(VecModel::from(
                            snapshot
                                .library
                                .applications
                                .iter()
                                .flatten()
                                .map(|item| LauncherItem {
                                    id: item.id.as_str().into(),
                                    name: item.name.as_str().into(),
                                    subtitle: item.path.as_str().into(),
                                    icon_key: if item.is_folder() { "folder" } else { "" }.into(),
                                    fallback: item
                                        .name
                                        .chars()
                                        .next()
                                        .unwrap_or('?')
                                        .to_uppercase()
                                        .to_string()
                                        .into(),
                                    ..LauncherItem::default()
                                })
                                .collect::<Vec<_>>(),
                        ))
                        .into(),
                    );
                    *self.snapshot.borrow_mut() = Some(snapshot);
                    if first || app.get_presented() {
                        if !app.get_file_mode() {
                            self.render(true);
                        }
                    } else {
                        self.dirty.set(true);
                    }
                    if first {
                        app.invoke_startup_configuration_ready(true);
                    }
                }
                Event::Status { scanning, notice } => {
                    if let Some(observer) = &self.observer {
                        observer.event(
                            "index_status",
                            &format!("scanning={scanning} warning={}", !notice.is_empty()),
                        );
                    }
                    if !self.configured.get() && !scanning && !notice.is_empty() {
                        app.invoke_startup_configuration_ready(false);
                    }
                    app.set_scanning(scanning);
                    app.set_business_status(notice.into());
                    if self.diagnostics {
                        eprintln!(
                            "event=index_status scanning={scanning} warning={}",
                            !notice.is_empty()
                        );
                    }
                }
                Event::Icons { ticket, items } if ticket.current() && app.get_presented() => {
                    for item in items {
                        let cost = item.pixels.cost();
                        let pixels =
                            slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                                &item.pixels.rgba,
                                item.pixels.width,
                                item.pixels.height,
                            );
                        self.images.borrow_mut().insert(
                            item.key,
                            slint::Image::from_rgba8(pixels),
                            cost,
                        );
                    }
                    self.apply_icons(&app, false);
                    if let Some(observer) = &self.observer {
                        let cache = self.images.borrow();
                        observer.event(
                            "icon_cache",
                            &format!(
                                "entries={} cpu_and_texture_estimate_bytes={}",
                                cache.len(),
                                cache.bytes()
                            ),
                        );
                    }
                    if self.diagnostics {
                        let cache = self.images.borrow();
                        eprintln!(
                            "event=icon_cache entries={} cpu_and_texture_estimate_bytes={}",
                            cache.len(),
                            cache.bytes()
                        );
                    }
                }
                Event::Icons { .. } => {}
                Event::SystemPrompt(prompt) => {
                    self.confirm_system_action(prompt);
                }
                Event::Launched {
                    id,
                    result,
                    history_saved,
                } => {
                    // Lifecycle owns failure restoration and its visibility
                    // generation. A late result cannot override a newer hide.
                    app.invoke_launch_finished(result.is_ok());
                    if let Err(error) = result {
                        let folder = self.snapshot.borrow().as_ref().is_some_and(|snapshot| {
                            snapshot
                                .catalog
                                .item(&id)
                                .is_some_and(|item| item.is_folder())
                        });
                        let notice = if folder {
                            match error {
                                LaunchFailure::MissingTarget => "找不到文件夹，可能已移动或删除",
                                LaunchFailure::Cancelled => "已取消打开文件夹",
                                _ => "无法打开文件夹，请检查路径或访问权限后重试",
                            }
                        } else {
                            error.message()
                        };
                        app.set_selection_notice(notice.into());
                        if error == LaunchFailure::MissingTarget {
                            self.confirm_removal(&id, true);
                        }
                    } else if !history_saved {
                        app.set_selection_notice(
                            "应用已启动，但历史记录保存失败，请检查数据目录权限".into(),
                        );
                    }
                    if self.diagnostics {
                        eprintln!(
                            "event=launch_completed success={} history_saved={history_saved}",
                            result.is_ok()
                        );
                    }
                }
                Event::WorkspaceFinished { action, result } => {
                    let message = match result {
                        Ok(WorkspaceSuccess::Opened) => format!("已打开{}", action.label()),
                        Ok(WorkspaceSuccess::ShortcutSubmitted) => {
                            format!("已提交「{}」快捷键", action.label())
                        }
                        Ok(_) => "工作台动作已完成".into(),
                        Err(error) => error.message().to_string(),
                    };
                    app.set_workspace_notice(message.into());
                    if self.diagnostics {
                        eprintln!(
                            "event=workspace_action_finished action={action:?} result={result:?}"
                        );
                    }
                    if let Some(observer) = &self.observer {
                        observer.event(
                            "workspace_action_finished",
                            &format!("action={action:?} result={result:?}"),
                        );
                    }
                }
                Event::WorkspaceUserActionFinished {
                    kind,
                    ticket,
                    result,
                } => {
                    let message = match result {
                        Ok(WorkspaceSuccess::Opened) => "已打开动作目标",
                        Ok(WorkspaceSuccess::ShortcutSubmitted) => "已提交快捷键步骤",
                        Ok(WorkspaceSuccess::TextSubmitted) => "已提交文本输入",
                        Ok(WorkspaceSuccess::ScriptCompleted) => "脚本已完成",
                        Err(error) => error.message(),
                    };
                    if app.get_workspace_action_ticket() == ticket {
                        app.set_workspace_notice(message.into());
                    }
                    if self.diagnostics {
                        eprintln!(
                            "event=workspace_user_action_finished kind={kind} result={result:?}"
                        );
                    }
                    if let Some(observer) = &self.observer {
                        observer.event(
                            "workspace_user_action_finished",
                            &format!("kind={kind} result={result:?}"),
                        );
                    }
                }
                Event::Managed { kind, result } => {
                    if kind == "workspace" {
                        let (saved, message) = match result {
                            Ok(message) => (true, message),
                            Err(error) => {
                                eprintln!(
                                    "event=workspace_save_failed operation={}",
                                    error.operation
                                );
                                (
                                    false,
                                    match error.operation {
                                        "verify Workspace script" => {
                                            "无法验证脚本文件，请检查路径或文件占用情况"
                                        }
                                        "read backup source" | "create backup" | "write backup"
                                        | "flush backup" | "commit backup" => {
                                            "旧工作台配置备份失败，设置尚未保存"
                                        }
                                        _ => "工作台设置未保存，请检查数据目录权限后重试",
                                    }
                                    .into(),
                                )
                            }
                        };
                        app.invoke_workspace_saved(saved, message.into());
                        continue;
                    }
                    if matches!(kind, "group_save" | "group_delete") {
                        if let Some(editor) = self.groups.borrow().as_ref() {
                            editor.managed(kind, result.is_ok());
                        }
                        continue;
                    }
                    app.set_management_busy(false);
                    match result {
                        Ok(message) => {
                            if kind == "backup" {
                                app.set_backup_path(message.as_str().into());
                            } else {
                                app.set_selection_notice(message.as_str().into());
                                app.set_import_notice(message.as_str().into());
                            }
                            if kind == "group_pin" {
                                app.set_group_notice(message.as_str().into());
                            }
                            app.set_settings_notice(
                                if kind == "backup" {
                                    "配置备份已保存"
                                } else {
                                    &message
                                }
                                .into(),
                            );
                            if kind == "settings" {
                                if let Some(snapshot) = self.snapshot.borrow().as_ref() {
                                    app.invoke_hotkey_committed(
                                        snapshot.config.hotkey.as_str().into(),
                                    );
                                }
                                app.invoke_show_appearance(false);
                            }
                        }
                        Err(error) => {
                            let notice = match error.operation {
                                "reserve global hotkey" => {
                                    "快捷键被占用或不可用，原快捷键和设置已保留"
                                }
                                "validate global hotkey" => "快捷键格式无效，请重新录制",
                                "validate login startup command" => {
                                    "登录启动命令过长，请将程序或数据目录移到较短路径后重试"
                                }
                                "update login startup" => {
                                    "登录启动项修改或恢复失败，请检查权限与 Windows 启动设置"
                                }
                                "import entries" => "每次最多导入 100 个项目，请减少预览后重试",
                                "add home pin" => "首页最多固定 200 项，请取消一项固定后重试",
                                "pin launch group" => "启动组合已不可用，请返回列表后重试",
                                "pin folder path" | "read pinned folder" => {
                                    "无法固定文件夹，请检查路径或访问权限后重试"
                                }
                                _ => "操作未完成，原文件已保留；请检查文件或目录权限后重试",
                            };
                            app.set_settings_notice(notice.into());
                            app.set_import_notice(notice.into());
                            app.set_selection_notice(notice.into());
                            if kind == "group_pin" {
                                app.set_group_notice(notice.into());
                            }
                            eprintln!("event=management_failed operation={}", error.operation);
                        }
                    }
                }
                event @ (Event::FileSearch { .. }
                | Event::FileOpened { .. }
                | Event::FileUtility { .. }) => self.files.receive(event),
                Event::Update(state) => {
                    app.set_update_busy(state.busy);
                    let (percent, detail) = match state.progress {
                        Some(InstallProgress::Download(DownloadProgress::Transfer {
                            received,
                            total: Some(total),
                        })) => {
                            let percent = (received * 100 / total.get()).min(99) as i32;
                            let detail = format!(
                                "{percent}% · {} / {}",
                                download_size(received),
                                download_size(total.get())
                            );
                            (percent, detail)
                        }
                        Some(InstallProgress::Download(DownloadProgress::Transfer {
                            received,
                            total: None,
                        })) if received > 0 => (-1, format!("已下载 {}", download_size(received))),
                        Some(
                            InstallProgress::Download(DownloadProgress::Verifying)
                            | InstallProgress::PreparingInstaller,
                        ) => (100, String::new()),
                        _ => (-1, String::new()),
                    };
                    app.set_update_progress(percent);
                    app.set_update_progress_visible(state.progress.is_some());
                    app.set_update_progress_detail(detail.into());
                    app.set_update_message(state.message.into());
                    app.set_update_available(
                        state.info.portable.is_some() || state.info.installer.is_some(),
                    );
                    let release = state.info.release.as_ref();
                    let notes = release.map_or("", |r| r.body.as_str());
                    if app.get_update_notes().as_str() != notes {
                        app.set_update_notes(notes.into());
                    }
                    app.set_update_release_label(
                        release
                            .map_or_else(String::new, |release| {
                                match release
                                    .published_at
                                    .split('T')
                                    .next()
                                    .filter(|date| !date.is_empty())
                                {
                                    Some(date) => format!("{} · {}", release.tag_name, date),
                                    None => release.tag_name.clone(),
                                }
                            })
                            .into(),
                    );
                    if state.scheduled {
                        app.invoke_quit_requested();
                    }
                }
            }
        }
    }

    fn render(self: &Rc<Self>, preserve_selection: bool) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        if app.get_file_mode() {
            return;
        }
        let snapshot = self.snapshot.borrow().clone();
        let Some(snapshot) = snapshot else {
            return;
        };
        let home = normalize(&app.get_input_text()).is_empty();
        let mut groups = [Vec::new(), Vec::new(), Vec::new()];
        let items = if home {
            let Ok(mut home) = snapshot.catalog.home(&snapshot.history, service::now()) else {
                app.set_selection_notice("读取首页失败，请刷新后重试".into());
                return;
            };
            app.set_system_total_count(home.tools.len() as i32);
            if !app.get_all_system_tools() {
                home.tools.truncate(12);
            }
            groups = [home.pinned, home.frequent, home.tools];
            self.lengths.set(groups.each_ref().map(|group| group.len()));
            groups.iter().flatten().cloned().collect::<Vec<_>>()
        } else {
            let Ok(results) = snapshot.catalog.search(
                &app.get_input_text(),
                &snapshot.config,
                &snapshot.history,
                service::now(),
            ) else {
                app.set_selection_notice("搜索失败，请刷新后重试".into());
                return;
            };
            results.into_iter().map(|result| result.item).collect()
        };
        let selected = app.get_selected_id();
        // Keep a blank-click deselection while refreshing an existing result list.
        // A search receiving its first results still selects the first item.
        let deselected = selected.is_empty() && app.get_result_items().row_count() > 0;
        if !preserve_selection
            || (!deselected && !items.iter().any(|item| item.id == selected.as_str()))
        {
            let initial_selection = if home {
                ""
            } else {
                items.first().map_or("", |item| item.id.as_str())
            };
            app.set_selected_id(initial_selection.into());
        }
        app.set_is_home(home);
        app.set_item_count(snapshot.catalog.len() as i32);
        app.set_space_launch(snapshot.config.space_launch);
        let model = |items: &[Arc<AppItem>]| -> ModelRc<LauncherItem> {
            Rc::new(VecModel::from(
                items
                    .iter()
                    .map(|item| LauncherItem {
                        id: item.id.as_str().into(),
                        name: item.name.as_str().into(),
                        subtitle: if item.is_launch_group() {
                            item.description.as_str()
                        } else {
                            item.path.as_str()
                        }
                        .into(),
                        fallback: item
                            .name
                            .chars()
                            .next()
                            .unwrap_or('?')
                            .to_uppercase()
                            .to_string()
                            .into(),
                        pinned: item.pinned,
                        system: item.source == "System",
                        icon_key: if item.is_folder() {
                            "folder"
                        } else if item.is_launch_group() {
                            "apps"
                        } else if item.source == "System" {
                            match item.id.as_str() {
                                "system:empty-recycle-bin" => "delete",
                                "system:shutdown" => "power",
                                "system:restart" => "refresh",
                                "system:lock" => "lock",
                                "system:calculator" => "calculator",
                                "system:explorer" => "folder",
                                "system:terminal" => "terminal",
                                "system:devices" | "system:taskmanager" => "monitor",
                                "system:apps" => "apps",
                                _ => "settings",
                            }
                        } else {
                            ""
                        }
                        .into(),
                        ..LauncherItem::default()
                    })
                    .collect::<Vec<_>>(),
            ))
            .into()
        };
        app.set_pinned_items(model(&groups[0]));
        app.set_frequent_items(model(&groups[1]));
        app.set_system_items(model(&groups[2]));
        app.set_result_items(if home {
            ModelRc::default()
        } else {
            model(&items)
        });
        *self.choices.borrow_mut() = items;
        if !preserve_selection {
            app.set_selection_notice("".into());
        }
        app.invoke_layout_changed();
        self.dirty.set(false);
        self.requested_icons.borrow_mut().clear();
        self.runtime.borrow().cancel_icons();
        self.queue_viewport();
    }

    fn choose(self: &Rc<Self>, id: &str, action: Action) {
        if action == Action::Launch
            && self
                .snapshot
                .borrow()
                .as_ref()
                .is_some_and(|snapshot| snapshot.groups.groups.iter().any(|group| group.id == id))
        {
            self.launch_group(id.into(), Vec::new(), None);
            return;
        }
        let Some(app) = self.app.upgrade() else {
            return;
        };
        if !app.get_presented() || app.get_native_dialog_active() {
            return;
        }
        let keep_visible = {
            let snapshot = self.snapshot.borrow();
            let Some(snapshot) = snapshot.as_ref() else {
                return;
            };
            let Some(item) = snapshot.catalog.item(id) else {
                return;
            };
            (action == Action::Launch && needs_confirmation(item))
                || snapshot.library.has_custom_app(id)
                || (item.pinned
                    && (Path::new(&item.path).is_absolute()
                        || item.path.starts_with(APPS_FOLDER_PREFIX)))
        };
        let Some(owner) = owner(&app) else {
            return;
        };
        if self.runtime.borrow().launch_interactive(
            id.into(),
            app.get_input_text().to_string(),
            action,
            owner,
        ) {
            if action != Action::OpenDirectory {
                if !keep_visible {
                    app.invoke_clear_input();
                }
                app.invoke_launch_started(!keep_visible);
            }
        } else {
            app.set_selection_notice("正在打开应用，请稍候".into());
        }
    }

    fn launch_group(&self, id: String, only: Vec<String>, force: Option<String>) -> bool {
        let Some(app) = self.app.upgrade() else {
            return false;
        };
        if !app.get_presented() || app.get_native_dialog_active() {
            return false;
        }
        let Some(owner) = owner(&app) else {
            return false;
        };
        if self.runtime.borrow().launch_group(id, only, force, owner) {
            app.set_group_run_busy(true);
            app.set_group_mode(2);
            app.invoke_clear_input();
            app.invoke_launch_started(true);
            true
        } else {
            app.set_selection_notice("正在处理启动或保存，请稍候".into());
            false
        }
    }

    fn preferences(&self, config: &Config) {
        if let Some(app) = self.app.upgrade() {
            app.set_dark_theme(match config.theme.as_str() {
                "dark" => true,
                "light" => false,
                _ => preferences::dark_theme(),
            });
            app.set_reduce_motion(preferences::reduced_motion());
        }
    }
    fn settings(&self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let snapshot = self.snapshot.borrow();
        let Some(snapshot) = snapshot.as_ref() else {
            return;
        };
        let c = &snapshot.config;
        app.set_settings_draft(SettingsDraft {
            startup: c.launch_at_startup,
            space: c.space_launch,
            maximum: c.max_results.to_string().into(),
            fullscreen: c.disable_hotkey_fullscreen,
            everything: c.everything.enabled,
            everything_executable: c.everything.executable.as_str().into(),
            theme: match c.theme.as_str() {
                "light" => 1,
                "dark" => 2,
                _ => 0,
            },
            layout: i32::from(c.result_layout == "grid"),
            fuzzy: c.search.fuzzy,
            weight: c.search.history_weight.to_string().into(),
            program_files: c.scan_program_files,
            noise: c.filter_noise,
            directories: c
                .custom_directories
                .iter()
                .flatten()
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
                .into(),
            refresh: c.refresh_minutes.to_string().into(),
            updates: c.auto_check_updates,
        });
        app.set_hotkey_draft(if app.get_hotkey_current().is_empty() {
            c.hotkey.as_str().into()
        } else {
            app.get_hotkey_current()
        });
        app.set_settings_notice("".into());
    }
    fn save(&self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        if app.get_hotkey_recording() || app.get_update_busy() || app.get_management_busy() {
            return;
        }
        let snapshot = self.snapshot.borrow();
        let Some(snapshot) = snapshot.as_ref() else {
            return;
        };
        let draft = app.get_settings_draft();
        let mut config = snapshot.config.as_ref().clone();
        let (Ok(maximum), Ok(weight), Ok(refresh)) = (
            draft.maximum.trim().parse(),
            draft.weight.trim().parse(),
            draft.refresh.trim().parse(),
        ) else {
            app.set_settings_notice("请输入有效的结果数、历史权重和刷新间隔".into());
            return;
        };
        config.max_results = maximum;
        config.search.history_weight = weight;
        config.refresh_minutes = refresh;
        config.hotkey = app.get_hotkey_draft().to_string();
        config.launch_at_startup = draft.startup;
        config.space_launch = draft.space;
        config.disable_hotkey_fullscreen = draft.fullscreen;
        config.theme = match draft.theme {
            1 => "light",
            2 => "dark",
            _ => "system",
        }
        .into();
        config.result_layout = if draft.layout == 1 { "grid" } else { "list" }.into();
        config.search.fuzzy = draft.fuzzy;
        config.everything.enabled = draft.everything;
        config.everything.executable = draft.everything_executable.trim().to_owned();
        config.scan_program_files = draft.program_files;
        config.filter_noise = draft.noise;
        config.custom_directories = Some(
            draft
                .directories
                .lines()
                .map(str::trim)
                .filter(|p| !p.is_empty())
                .map(str::to_owned)
                .collect(),
        );
        config.auto_check_updates = draft.updates;
        use velo_slint::core::JsonDocument;
        if config.validate().is_err() {
            app.set_settings_notice(
                "设置超出允许范围，或索引目录不是绝对路径；请检查后重试".into(),
            );
            return;
        }
        self.manage(Manage::Save(config));
    }
    fn manage(&self, request: Manage) {
        if let Some(app) = self.app.upgrade() {
            request_management(&app, &self.runtime.borrow(), request);
        }
    }
    fn update(&self, install: bool) {
        if let Some(app) = self.app.upgrade() {
            if self.runtime.borrow().update(install) {
                app.set_update_busy(true);
            } else {
                app.set_update_message("其他操作正在进行，请稍后重试".into());
            }
        }
    }
    fn confirm_removal(self: &Rc<Self>, id: &str, missing: bool) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        if app.get_management_busy() {
            return;
        }
        let (item, is_custom) = {
            let snapshot = self.snapshot.borrow();
            let Some(snapshot) = snapshot.as_ref() else {
                return;
            };
            let Some(item) = snapshot.catalog.item(id) else {
                return;
            };
            (item.clone(), snapshot.library.has_custom_app(id))
        };
        if !(is_custom || (missing && item.pinned)) {
            return;
        }
        let mut confirmation = if is_custom {
            let reason = if missing && item.is_folder() {
                "找不到此文件夹。\n"
            } else if missing {
                "找不到此应用。\n"
            } else {
                ""
            };
            let detail = format!("{reason}移除范围仅限 Velo 中的自定义记录及首页固定项。");
            let mut confirmation =
                confirmations::question("移除自定义项目？", detail, "移除记录", "delete");
            confirmation.destructive = true;
            confirmation
        } else {
            let (title, detail, icon) = if item.is_folder() {
                (
                    "取消固定文件夹？",
                    "找不到此文件夹，可能已被移动或删除。\n取消固定后，首页将移除此快捷入口。",
                    "folder",
                )
            } else {
                (
                    "取消固定应用？",
                    "找不到此应用，可能已卸载或移动。\n取消固定后，首页将移除此快捷入口。",
                    "pinned",
                )
            };
            let mut confirmation = confirmations::question(title, detail, "取消固定", icon);
            confirmation.cancel_label = "保留固定".into();
            confirmation
        };
        confirmation.subject = item.name.as_str().into();
        let request = if is_custom {
            Manage::Delete(id.into())
        } else {
            Manage::Pin(id.into(), false)
        };
        let weak = Rc::downgrade(self);
        self.confirmations
            .show(confirmation, false, move |accepted| {
                if accepted && let Some(controller) = weak.upgrade() {
                    controller.manage(request);
                }
            });
    }

    fn confirm_update(self: &Rc<Self>) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        if app.get_management_busy() || app.get_update_busy() {
            return;
        }
        let mut confirmation = confirmations::question(
            "下载并安装更新？",
            "Velo 会在安装前备份已保存的配置，随后退出并在更新后重新启动。",
            "下载并安装",
            "download",
        );
        confirmation.subject = app.get_update_release_label();
        let weak = Rc::downgrade(self);
        self.confirmations
            .show(confirmation, false, move |accepted| {
                if accepted && let Some(controller) = weak.upgrade() {
                    controller.update(true);
                }
            });
    }

    fn confirm_system_action(&self, prompt: SystemPrompt) {
        let mut confirmation = match prompt {
            SystemPrompt::Shutdown => {
                confirmations::question("关闭电脑？", "请先保存正在编辑的文件。", "关机", "power")
            }
            SystemPrompt::Restart => confirmations::question(
                "重启电脑？",
                "请先保存正在编辑的文件。",
                "重新启动",
                "refresh",
            ),
            SystemPrompt::EmptyRecycleBin(count) => {
                let detail = match count.filter(|count| *count > 0) {
                    Some(count) => format!("将永久删除回收站中的 {count} 个项目。此操作无法撤销。"),
                    None => "将永久删除回收站中的所有项目。此操作无法撤销。".into(),
                };
                let mut confirmation =
                    confirmations::question("清空回收站？", detail, "清空回收站", "delete");
                confirmation.default_accept = true;
                confirmation
            }
            SystemPrompt::RecycleBinAlreadyEmpty => Confirmation {
                title: "回收站已为空".into(),
                detail: "当前没有需要清理的项目。".into(),
                confirm_label: "确定".into(),
                icon_key: "check".into(),
                ..Confirmation::default()
            },
        };
        confirmation.destructive = !confirmation.cancel_label.is_empty();
        let runtime = Rc::downgrade(&self.runtime);
        if !self
            .confirmations
            .show(confirmation, true, move |accepted| {
                if let Some(runtime) = runtime.upgrade() {
                    runtime.borrow().answer_system_prompt(accepted);
                }
            })
        {
            self.runtime.borrow().answer_system_prompt(false);
        }
    }

    fn application_menu(self: &Rc<Self>, id: &str) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        if !app.get_presented() || app.get_native_dialog_active() || app.get_management_busy() {
            return;
        }
        let Some(item) = self
            .snapshot
            .borrow()
            .as_ref()
            .and_then(|s| s.catalog.item(id).cloned())
        else {
            return;
        };
        let Some(owner) = owner(&app) else {
            return;
        };
        app.set_native_dialog_active(true);
        app.invoke_suspend_input();
        let weak = app.as_weak();
        let id = id.to_owned();
        let completion_id = id.clone();
        let started = std::thread::Builder::new()
            .name("velo-item-interaction".into())
            .spawn(move || {
                let result = interaction::application_menu(owner, &item);
                let _ = weak.upgrade_in_event_loop(move |app| {
                    app.invoke_application_menu_completed(id.into(), result)
                });
            });
        if started.is_err() {
            app.invoke_application_menu_completed(completion_id.into(), 0);
            app.set_selection_notice("无法打开原生交互，请重试".into());
        }
    }
    fn application_menu_completed(self: &Rc<Self>, id: &str, action: i32) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        app.set_native_dialog_active(false);
        match action {
            1 => self.choose(id, Action::Launch),
            2 => self.choose(id, Action::OpenDirectory),
            3 => self.choose(id, Action::RunAsAdmin),
            4 => self.toggle_pin(id),
            -1 => app.set_selection_notice("无法打开菜单，请重试".into()),
            _ => {}
        }
        app.invoke_native_interaction_finished();
    }

    fn toggle_pin(&self, id: &str) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        if !app.get_presented() || app.get_native_dialog_active() || app.get_management_busy() {
            return;
        }
        let runtime = self.runtime.borrow();
        request_management(
            &app,
            &runtime,
            Manage::Pin(id.into(), !runtime.is_pinned(id)),
        );
    }

    fn queue_viewport(self: &Rc<Self>) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        if !app.get_presented()
            || (app.get_file_mode() && !app.get_import_open() && !app.get_groups_open())
        {
            self.runtime.borrow().cancel_icons();
            self.requested_icons.borrow_mut().clear();
            self.apply_icons(&app, true);
            return;
        }
        if self.viewport_queued.replace(true) {
            return;
        }
        let weak = Rc::downgrade(self);
        slint::Timer::single_shot(Duration::from_millis(20), move || {
            if let Some(controller) = weak.upgrade() {
                controller.viewport_queued.set(false);
                if controller.dirty.get() {
                    controller.render(true);
                } else if let Some(app) = controller.app.upgrade() {
                    controller.apply_icons(&app, false);
                }
            }
        });
    }

    fn apply_icons(&self, app: &AppWindow, hidden: bool) {
        let mut requested = Vec::new();
        let mut protected = BTreeSet::new();
        let scale = app.window().scale_factor();
        for (group, model) in [
            app.get_pinned_items(),
            app.get_frequent_items(),
            app.get_system_items(),
            app.get_result_items(),
            app.get_custom_items(),
            app.get_group_sources(),
        ]
        .into_iter()
        .enumerate()
        {
            let size = (app.invoke_icon_display_size(group as i32) * scale).round() as u32;
            for index in 0..model.row_count() {
                let Some(mut row) = model.row_data(index) else {
                    continue;
                };
                let visible = !hidden && app.invoke_icon_visible(group as i32, index as i32);
                let image = if visible {
                    let item = self
                        .choices
                        .borrow()
                        .iter()
                        .find(|item| item.id == row.id.as_str())
                        .cloned()
                        .or_else(|| {
                            self.snapshot.borrow().as_ref().and_then(|snapshot| {
                                snapshot.catalog.item(row.id.as_str()).cloned()
                            })
                        });
                    item.filter(|item| !item.icon_url.is_empty())
                        .and_then(|item| {
                            let target = IconTarget::new(item, size);
                            let first = protected.insert(target.key().to_owned());
                            if let Some(image) = self.images.borrow_mut().get(target.key()) {
                                return Some(image);
                            }
                            if first {
                                requested.push(target);
                            }
                            None
                        })
                } else {
                    None
                };
                let image = image.unwrap_or_default();
                if row.icon != image {
                    row.icon = image;
                    model.set_row_data(index, row);
                }
            }
        }
        self.images.borrow_mut().protect(protected);
        let keys: Vec<_> = requested
            .iter()
            .map(|target| target.key().to_owned())
            .collect();
        if *self.requested_icons.borrow() != keys {
            *self.requested_icons.borrow_mut() = keys;
            self.runtime.borrow().icons(requested);
        }
    }

    pub fn stop(&self) {
        self.runtime.borrow_mut().stop();
    }
}

fn download_size(bytes: u64) -> String {
    match bytes {
        0..1024 => format!("{bytes} B"),
        1024..1048576 => format!("{:.1} KiB", bytes as f64 / 1024.0),
        _ => format!("{:.1} MiB", bytes as f64 / 1048576.0),
    }
}

pub(crate) fn request_management(app: &AppWindow, runtime: &Runtime, request: Manage) {
    if runtime.manage(request) {
        app.set_management_busy(true);
        app.set_settings_notice("正在保存…".into());
    } else {
        let notice = "其他操作正在进行，请稍后重试";
        app.set_settings_notice(notice.into());
        app.set_import_notice(notice.into());
        app.set_selection_notice(notice.into());
        if app.get_groups_open() {
            app.set_group_notice(notice.into());
        }
    }
}

pub(crate) fn owner(app: &AppWindow) -> Option<isize> {
    app.window()
        .with_winit_window(|w| match w.window_handle().ok()?.as_raw() {
            RawWindowHandle::Win32(h) => Some(h.hwnd.get()),
            _ => None,
        })
        .flatten()
}
