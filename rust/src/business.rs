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
use velo_slint::core::model::AppItem;
use velo_slint::core::service::{Event, Manage, Options, Runtime, Snapshot};
use velo_slint::core::{icons, launch::Action, normalize::normalize, service};

use crate::{AppWindow, LauncherItem, SettingsDraft, navigation, ui};

pub struct Controller {
    app: slint::Weak<AppWindow>,
    runtime: RefCell<Runtime>,
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
}

impl Controller {
    pub fn connect(
        app: &AppWindow,
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
        let controller = Rc::new(Self {
            app: app.as_weak(),
            runtime: RefCell::new(runtime),
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
                    title: "拼音检索采用 go-pinyin（MIT License）。".into(),
                    text: include_str!("../licenses/go-pinyin-MIT.txt").into(),
                },
                crate::LicenseEntry {
                    title: "Unicode 数据源自 Go（BSD License）。".into(),
                    text: include_str!("../licenses/Go-BSD.txt").into(),
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
                controller.render(false);
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
        app.on_settings_requested(move || {
            if let Some(controller) = weak.upgrade() {
                controller.settings();
            }
        });
        let weak = Rc::downgrade(&controller);
        app.on_system_expansion_changed(move || {
            if let Some(controller) = weak.upgrade() {
                controller.render(false);
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
                controller.native(&id, 0);
            }
        });
        let weak = Rc::downgrade(&controller);
        app.on_application_delete(move |id| {
            if let Some(controller) = weak.upgrade() {
                controller.native(&id, 5);
            }
        });
        let weak = Rc::downgrade(&controller);
        app.on_application_menu_completed(move |id, action| {
            if let Some(controller) = weak.upgrade() {
                controller.native_completed(&id, action);
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
                    controller.native("", 6);
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
        Ok(controller)
    }

    fn receive(self: &Rc<Self>) {
        let events = self.runtime.borrow().drain();
        let Some(app) = self.app.upgrade() else {
            return;
        };
        for event in events {
            match event {
                Event::Snapshot(snapshot) => {
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
                        self.render(true);
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
                Event::Launched {
                    result,
                    history_saved,
                } => {
                    // Lifecycle owns failure restoration and its visibility
                    // generation. A late result cannot override a newer hide.
                    app.invoke_launch_finished(result.is_ok());
                    if let Err(error) = result {
                        app.set_selection_notice(error.message().into());
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
                Event::Managed { kind, result } => {
                    app.set_management_busy(false);
                    match result {
                        Ok(message) => {
                            if kind == "backup" {
                                app.set_backup_path(message.as_str().into());
                            } else {
                                app.set_selection_notice(message.as_str().into());
                                app.set_import_notice(message.as_str().into());
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
                                "import applications" => {
                                    "每次最多导入 100 个文件，请减少预览后重试"
                                }
                                _ => "操作未完成，原文件已保留；请检查文件或目录权限后重试",
                            };
                            app.set_settings_notice(notice.into());
                            app.set_import_notice(notice.into());
                            app.set_selection_notice(notice.into());
                            eprintln!("event=management_failed operation={}", error.operation);
                        }
                    }
                }
                Event::Update(state) => {
                    app.set_update_busy(state.busy);
                    app.set_update_progress(state.progress.into());
                    app.set_update_message(state.message.into());
                    app.set_update_available(
                        state.info.portable.is_some() || state.info.installer.is_some(),
                    );
                    app.set_update_notes(
                        state
                            .info
                            .release
                            .as_ref()
                            .map_or(String::new(), |r| {
                                format!("{} · {}\n{}", r.tag_name, r.published_at, r.body)
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
        if !preserve_selection || !items.iter().any(|item| item.id == selected.as_str()) {
            app.set_selected_id(items.first().map_or("", |item| item.id.as_str()).into());
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
                        subtitle: item.path.as_str().into(),
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
                        icon_key: if item.source == "System" {
                            match item.id.as_str() {
                                "system:calculator" => "calculator",
                                "system:explorer" => "folder",
                                "system:terminal" => "terminal",
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
        let Some(app) = self.app.upgrade() else {
            return;
        };
        if !app.get_presented()
            || app.get_native_dialog_active()
            || !self
                .snapshot
                .borrow()
                .as_ref()
                .is_some_and(|s| s.catalog.item(id).is_some())
        {
            return;
        }
        let Some(owner) = owner(&app) else {
            return;
        };
        if self
            .runtime
            .borrow()
            .launch(id.into(), app.get_input_text().to_string(), action, owner)
        {
            if action != Action::OpenDirectory {
                app.invoke_clear_input();
                app.invoke_launch_started();
            }
        } else {
            app.set_selection_notice("正在打开应用，请稍候".into());
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
            if self.runtime.borrow().manage(request) {
                app.set_management_busy(true);
                app.set_settings_notice("正在保存…".into());
            } else {
                app.set_settings_notice("其他操作正在进行，请稍后重试".into());
                app.set_import_notice("其他操作正在进行，请稍后重试".into());
            }
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
    fn native(self: &Rc<Self>, id: &str, request: i32) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        if !app.get_presented() || app.get_native_dialog_active() || app.get_management_busy() {
            return;
        }
        let item = self
            .snapshot
            .borrow()
            .as_ref()
            .and_then(|s| s.catalog.item(id).cloned());
        if request != 6 && item.is_none() {
            return;
        }
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
                let result = interaction::application(owner, item.as_deref(), request);
                let _ = weak.upgrade_in_event_loop(move |app| {
                    app.invoke_application_menu_completed(id.into(), result)
                });
            });
        if started.is_err() {
            app.invoke_application_menu_completed(completion_id.into(), 0);
            app.set_selection_notice("无法打开原生交互，请重试".into());
        }
    }
    fn native_completed(self: &Rc<Self>, id: &str, action: i32) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        app.set_native_dialog_active(false);
        match action {
            1 => self.choose(id, Action::Launch),
            2 => self.choose(id, Action::OpenDirectory),
            3 => self.choose(id, Action::RunAsAdmin),
            4 => {
                let pinned = self
                    .snapshot
                    .borrow()
                    .as_ref()
                    .is_some_and(|s| s.library.pinned.iter().flatten().any(|i| i.id == id));
                self.manage(Manage::Pin(id.into(), !pinned));
            }
            5 => self.manage(Manage::Delete(id.into())),
            6 => self.update(true),
            -1 => app.set_selection_notice("无法打开原生菜单或确认框，请重试".into()),
            _ => {}
        }
        app.invoke_native_interaction_finished();
    }

    fn queue_viewport(self: &Rc<Self>) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        if !app.get_presented() {
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
        let mut seen = BTreeSet::new();
        let mut protected = BTreeSet::new();
        for (group, model) in [
            app.get_pinned_items(),
            app.get_frequent_items(),
            app.get_system_items(),
            app.get_result_items(),
            app.get_custom_items(),
        ]
        .into_iter()
        .enumerate()
        {
            for index in 0..model.row_count() {
                let Some(mut row) = model.row_data(index) else {
                    continue;
                };
                // Wails renders semantic Fluent glyphs for system shortcuts.
                // Do not request/cache a Shell bitmap that would replace them.
                let visible =
                    !hidden && !row.system && app.invoke_icon_visible(group as i32, index as i32);
                let item =
                    self.choices
                        .borrow()
                        .iter()
                        .find(|item| item.id == row.id.as_str())
                        .cloned()
                        .or_else(|| {
                            self.snapshot.borrow().as_ref().and_then(|snapshot| {
                                snapshot.catalog.item(row.id.as_str()).cloned()
                            })
                        });
                let image = if visible {
                    item.and_then(|item| {
                        protected.insert(item.icon_url.clone());
                        if let Some(image) = self.images.borrow_mut().get(&item.icon_url) {
                            return Some(image);
                        }
                        if !item.icon_url.is_empty() && seen.insert(item.icon_url.clone()) {
                            requested.push(item.as_ref().clone());
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
        let keys: Vec<_> = requested.iter().map(|item| item.icon_url.clone()).collect();
        if *self.requested_icons.borrow() != keys {
            *self.requested_icons.borrow_mut() = keys;
            self.runtime.borrow().icons(requested);
        }
    }

    pub fn stop(&self) {
        self.runtime.borrow_mut().stop();
    }
}

fn owner(app: &AppWindow) -> Option<isize> {
    app.window()
        .with_winit_window(|w| match w.window_handle().ok()?.as_raw() {
            RawWindowHandle::Win32(h) => Some(h.hwnd.get()),
            _ => None,
        })
        .flatten()
}
