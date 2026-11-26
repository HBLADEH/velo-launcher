//! Opt-in component verification against a real isolated store. This exercises
//! callbacks and persistence; it is deliberately not native keyboard/IME proof.
use crate::{AppWindow, business_probe::QUERY, platform::windows::interaction};
use image::ImageEncoder;
use slint::platform::Key;
use slint::{ComponentHandle, Model};
use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;
use std::sync::mpsc::{SyncSender, sync_channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

type Picture = (String, u32, u32, Vec<u8>);
pub struct Controller {
    app: slint::Weak<AppWindow>,
    timer: slint::Timer,
    stage: Cell<u8>,
    started: Instant,
    passed: Cell<bool>,
    restart: bool,
    selected: RefCell<slint::SharedString>,
    manual: String,
    sender: RefCell<Option<SyncSender<Picture>>>,
    writer: RefCell<Option<JoinHandle<std::io::Result<()>>>>,
}
impl Controller {
    pub fn start(app: &AppWindow, directory: &Path, restart: bool) -> std::io::Result<Rc<Self>> {
        let (sender, receiver) = sync_channel::<Picture>(2);
        let destination = directory.to_owned();
        let writer = std::thread::Builder::new()
            .name("velo-pages-probe-output".into())
            .spawn(move || {
                for (name, width, height, bytes) in receiver {
                    let file = std::fs::File::create(destination.join(name))?;
                    image::codecs::png::PngEncoder::new(std::io::BufWriter::new(file))
                        .write_image(&bytes, width, height, image::ExtendedColorType::Rgba8)
                        .map_err(std::io::Error::other)?;
                }
                Ok(())
            })?;
        let manual = directory
            .parent()
            .unwrap_or(directory)
            .join("manual/RS11 自定义应用.URL")
            .to_string_lossy()
            .into_owned();
        let controller = Rc::new(Self {
            app: app.as_weak(),
            timer: slint::Timer::default(),
            stage: Cell::new(0),
            started: Instant::now(),
            passed: Cell::new(false),
            restart,
            selected: RefCell::default(),
            manual,
            sender: RefCell::new(Some(sender)),
            writer: RefCell::new(Some(writer)),
        });
        let weak = Rc::downgrade(&controller);
        controller.timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(250),
            move || {
                if let Some(controller) = weak.upgrade() {
                    controller.tick();
                }
            },
        );
        Ok(controller)
    }
    fn check(&self, passed: bool, name: &str) -> Result<(), String> {
        eprintln!("event=pages_smoke_assert name={name} passed={passed}");
        if passed { Ok(()) } else { Err(name.into()) }
    }
    fn picture(&self, app: &AppWindow, name: String) -> Result<(), String> {
        let pixels = app
            .window()
            .take_snapshot()
            .map_err(|_| "snapshot failed")?;
        if app.get_confirmation_open() {
            let scale = pixels.width() as f32 / app.get_viewport_width();
            self.check(
                pixels.height() as f32 + 1.0 >= app.get_confirmation_layout_height() * scale,
                "confirmation_snapshot_uses_resized_window",
            )?;
        }
        self.sender
            .borrow()
            .as_ref()
            .ok_or("writer closed")?
            .try_send((
                name,
                pixels.width(),
                pixels.height(),
                pixels.as_bytes().to_vec(),
            ))
            .map_err(|_| "snapshot queue full".into())
    }
    fn query(app: &AppWindow, text: &str) {
        app.set_input_text(text.into());
        app.invoke_input_edited(text.into());
    }
    fn key(app: &AppWindow, key: slint::platform::Key) {
        let text: slint::SharedString = key.into();
        app.window()
            .dispatch_event(slint::platform::WindowEvent::KeyPressed { text: text.clone() });
        app.window()
            .dispatch_event(slint::platform::WindowEvent::KeyReleased { text });
    }
    fn tick(&self) {
        let Some(app) = self.app.upgrade() else {
            self.timer.stop();
            return;
        };
        // This opt-in probe drives component callbacks. A desktop focus change
        // may hide its isolated window while an async pin/import completes;
        // resume the synthetic session instead of waiting for physical input.
        // Stage 29 deliberately verifies a hidden draft and must stay hidden.
        if self.stage.get() != 29 && !app.get_presented() {
            app.invoke_desktop_event(1);
            return;
        }
        let result = if self.started.elapsed() > Duration::from_secs(90) {
            Err("pages probe timed out".into())
        } else {
            self.step(&app)
        };
        if let Err(reason) = result {
            eprintln!("event=pages_smoke_result passed=false reason={reason}");
            self.timer.stop();
            app.invoke_quit_requested();
        }
    }
    fn step(&self, app: &AppWindow) -> Result<(), String> {
        match self.stage.get() {
            0 if app.get_business_revision() > 0
                && !app.get_scanning()
                && app.get_presented()
                && app.get_result_items().row_count() == 1 =>
            {
                self.check(app.get_input_text() == QUERY, "isolated_catalog_loaded")?;
                if self.restart {
                    self.check(app.get_grid_results(), "saved_grid_reloaded")?;
                    self.check(
                        app.get_result_items().row_data(0).is_some_and(|i| i.pinned),
                        "pin_survives_restart",
                    )?;
                    self.check(
                        app.get_custom_items()
                            .iter()
                            .any(|i| i.name == "RS11 自定义应用"),
                        "custom_import_survives_restart",
                    )?;
                    // The default list has less room than the grid for a single result.
                    app.set_grid_results(false);
                    app.invoke_layout_changed();
                    app.invoke_item_chosen(app.get_selected_id());
                    self.stage.set(42);
                } else {
                    let selected = app.get_selected_id();
                    crate::interaction_probe::click(app, 6.0, 112.0);
                    self.check(
                        app.get_selected_id().is_empty() && app.get_input_text() == QUERY,
                        "search_margin_click_clears_selection",
                    )?;
                    *self.selected.borrow_mut() = selected.clone();
                    app.invoke_item_pin_toggled(selected);
                    self.stage.set(1);
                }
            }
            1 if !app.get_management_busy()
                && app.get_result_items().row_data(0).is_some_and(|i| i.pinned) =>
            {
                self.check(true, "pin_callback_persists_and_publishes")?;
                self.check(
                    app.get_selected_id().is_empty(),
                    "catalog_refresh_keeps_deselected_search_result",
                )?;
                app.invoke_show_appearance(true);
                app.set_settings_tab(0);
                app.set_dark_theme(false);
                self.stage.set(10);
            }
            stage @ 10..=23 => {
                let tab = (stage - 10) % 7;
                let dark = stage >= 17;
                self.check(
                    app.get_appearance_open() && app.get_settings_tab() == i32::from(tab),
                    &format!("settings_tab_{tab}_{}", if dark { "dark" } else { "light" }),
                )?;
                self.picture(
                    app,
                    format!("settings-{}-{tab}.png", if dark { "dark" } else { "light" }),
                )?;
                if stage < 23 {
                    app.set_settings_tab(i32::from((tab + 1) % 7));
                    if stage == 16 {
                        app.set_dark_theme(true);
                    }
                    self.stage.set(stage + 1);
                } else {
                    let mut draft = app.get_settings_draft();
                    draft.layout = 1;
                    draft.theme = 1;
                    draft.maximum = "20".into();
                    draft.space = false;
                    draft.updates = false;
                    draft.startup = false;
                    app.set_settings_draft(draft);
                    app.invoke_settings_save();
                    self.stage.set(24);
                }
            }
            24 if !app.get_management_busy()
                && !app.get_appearance_open()
                && app.get_grid_results() =>
            {
                self.check(
                    app.get_hotkey_error().is_empty(),
                    "settings_committed_to_platform_and_store",
                )?;
                Self::query(app, "系统");
                self.stage.set(25);
            }
            24 if !app.get_update_busy()
                && !app.get_management_busy()
                && app.get_appearance_open() =>
            {
                app.invoke_settings_save();
            }
            25 if app.get_result_items().row_count() > 6 && !app.get_scanning() => {
                Self::query(app, "");
                self.check(
                    app.get_selected_id().is_empty(),
                    "clearing_search_leaves_home_unselected",
                )?;
                self.check(
                    app.get_system_total_count() > 12 && app.get_system_items().row_count() == 12,
                    "home_system_shortcuts_start_with_twelve",
                )?;
                app.set_all_system_tools(true);
                // Slint changed handlers run on the next event-loop turn.
                // Observe the published choices after that turn, as users do.
                self.stage.set(50);
            }
            50 if app.get_system_items().row_count() == app.get_system_total_count() as usize => {
                self.check(
                    app.get_system_items().row_count() > 12,
                    "system_expansion_publishes_all_choices",
                )?;
                self.check(
                    app.get_selected_id().is_empty(),
                    "expanding_system_tools_leaves_home_unselected",
                )?;
                app.set_all_system_tools(false);
                Self::query(app, "系统");
                self.stage.set(51);
            }
            51 if app.get_result_items().row_count() > 6 => {
                let first = app.get_selected_id();
                let query = app.get_input_text();
                Self::key(app, slint::platform::Key::RightArrow);
                self.check(
                    app.get_selected_id() != first && app.get_input_text() == query,
                    "grid_horizontal_navigation_keeps_query",
                )?;
                app.set_native_composing(true);
                let selected = app.get_selected_id();
                Self::key(app, slint::platform::Key::LeftArrow);
                self.check(
                    app.get_selected_id() == selected,
                    "grid_composition_protects_navigation",
                )?;
                app.set_native_composing(false);
                self.picture(app, "grid-light.png".into())?;
                app.set_dark_theme(true);
                self.stage.set(26);
            }
            26 => {
                self.picture(app, "grid-dark.png".into())?;
                app.invoke_show_import(true);
                app.invoke_import_path_requested(self.manual.as_str().into());
                self.stage.set(30);
            }
            30 if !app.get_import_busy() && app.get_preview_entries().row_count() == 1 => {
                self.check(
                    app.get_preview_entries()
                        .row_data(0)
                        .is_some_and(|entry| entry.valid),
                    "real_import_preview_callback",
                )?;
                app.invoke_import_native_requested(interaction::CLEAR);
                self.check(app.get_confirmation_open(), "clear_preview_asks_first")?;
                self.stage.set(31);
            }
            31 if app.get_confirmation_layout_ready() => {
                self.picture(app, "confirmation-clear-preview.png".into())?;
                Self::key(app, Key::Escape);
                self.check(
                    !app.get_confirmation_open() && app.get_preview_entries().row_count() == 1,
                    "clear_preview_cancel_keeps_entries",
                )?;
                app.invoke_import_commit();
                self.stage.set(27);
            }
            27 if !app.get_management_busy()
                && app
                    .get_custom_items()
                    .iter()
                    .any(|i| i.name == "RS11 自定义应用") =>
            {
                self.check(
                    app.invoke_icon_visible(4, 0),
                    "custom_first_icon_is_in_viewport",
                )?;
                self.stage.set(52);
            }
            52 => {
                if app
                    .get_custom_items()
                    .iter()
                    .all(|item| item.icon.size().width == 0)
                {
                    // Icon work returns asynchronously; keep the regression tied
                    // to a real imported image instead of accepting a blank tile.
                    return Ok(());
                }
                self.check(true, "import_callback_persists_without_launch_or_pin")?;
                self.check(
                    app.get_custom_items()
                        .iter()
                        .any(|i| i.icon.size().width > 0),
                    "custom_list_visible_icons_loaded",
                )?;
                self.picture(app, "custom-applications.png".into())?;
                app.invoke_show_import(false);
                app.invoke_show_appearance(true);
                app.set_settings_tab(5);
                app.invoke_backup_requested(false);
                self.stage.set(28);
            }
            28 if !app.get_management_busy() && !app.get_backup_path().is_empty() => {
                self.check(true, "backup_callback_returns_saved_original")?;
                self.picture(app, "backup-created.png".into())?;
                let mut draft = app.get_settings_draft();
                draft.maximum = "17".into();
                app.set_settings_draft(draft);
                app.invoke_hide_requested();
                self.stage.set(29);
            }
            29 => {
                self.check(
                    !app.get_presented()
                        && app.get_appearance_open()
                        && app.get_settings_draft().maximum == "17",
                    "hide_preserves_settings_draft",
                )?;
                let toggle = crate::platform::windows::desktop::Event::Toggle as i32;
                app.invoke_desktop_event(toggle);
                self.check(
                    app.get_presented()
                        && app.get_appearance_open()
                        && app.get_settings_draft().maximum == "17",
                    "tray_show_preserves_settings_draft",
                )?;
                app.invoke_desktop_event(toggle);
                self.check(!app.get_presented(), "tray_second_activation_hides")?;
                app.set_native_dialog_active(true);
                app.invoke_desktop_event(toggle);
                self.check(!app.get_presented(), "tray_defers_to_native_dialog")?;
                app.set_native_dialog_active(false);
                let open = crate::platform::windows::desktop::Event::Open as i32;
                app.invoke_desktop_event(open);
                app.invoke_desktop_event(open);
                self.check(app.get_presented(), "tray_open_menu_always_shows")?;
                self.complete(app);
            }
            40 => {
                self.check(
                    app.get_result_items().row_count() == 1,
                    "import_is_searchable_after_restart",
                )?;
                app.invoke_application_delete(app.get_selected_id());
                self.check(app.get_confirmation_open(), "custom_removal_asks_first")?;
                self.stage.set(46);
            }
            41 if !app.get_management_busy() => {
                self.check(
                    app.get_custom_items()
                        .iter()
                        .all(|i| i.name != "RS11 自定义应用")
                        && app.get_result_items().row_count() == 0,
                    "delete_callback_commits_without_revival",
                )?;
                Self::query(app, "关机");
                app.invoke_item_chosen("system:shutdown".into());
                self.stage.set(47);
            }
            42 if app.get_confirmation_open() && app.get_confirmation_layout_ready() => {
                self.check(
                    app.get_confirmation().title == "取消固定应用？" && app.get_presented(),
                    "missing_pinned_target_opens_themed_confirmation",
                )?;
                self.check(
                    app.get_viewport_height() + 1.0 >= app.get_confirmation_minimum_height(),
                    "compact_list_makes_room_for_confirmation",
                )?;
                self.picture(app, "confirmation-unpin-light.png".into())?;
                app.set_dark_theme(true);
                self.stage.set(43);
            }
            43 => {
                self.picture(app, "confirmation-unpin-dark.png".into())?;
                Self::key(app, Key::Escape);
                self.check(
                    !app.get_confirmation_open()
                        && app.get_input_has_focus()
                        && app.get_input_text() == QUERY
                        && app
                            .get_result_items()
                            .row_data(0)
                            .is_some_and(|item| item.pinned),
                    "missing_target_cancel_keeps_pin_and_search",
                )?;
                app.invoke_item_chosen(app.get_selected_id());
                self.stage.set(44);
            }
            44 if app.get_confirmation_open() && app.get_confirmation_layout_ready() => {
                Self::key(app, Key::Tab);
                Self::key(app, Key::Return);
                self.stage.set(45);
            }
            45 if !app.get_management_busy() => {
                self.check(
                    !app.get_confirmation_open() && app.get_result_items().row_count() == 0,
                    "missing_target_unpin_commits",
                )?;
                Self::query(app, "RS11 自定义应用");
                self.stage.set(40);
            }
            46 if app.get_confirmation_layout_ready() => {
                self.picture(app, "confirmation-remove-dark.png".into())?;
                Self::key(app, Key::Return);
                self.check(
                    !app.get_confirmation_open() && app.get_result_items().row_count() == 1,
                    "custom_removal_default_keeps_record",
                )?;
                app.invoke_application_delete(app.get_selected_id());
                self.check(app.get_confirmation_open(), "custom_removal_reopens")?;
                self.stage.set(49);
            }
            49 if app.get_confirmation_layout_ready() => {
                Self::key(app, Key::Tab);
                Self::key(app, Key::Return);
                self.check(app.get_management_busy(), "custom_removal_keyboard_submits")?;
                self.stage.set(41);
            }
            47 if app.get_confirmation_open() && app.get_confirmation_layout_ready() => {
                self.check(
                    app.get_confirmation().title == "关闭电脑？" && app.get_presented(),
                    "power_confirmation_stays_in_launcher",
                )?;
                self.picture(app, "confirmation-shutdown-dark.png".into())?;
                Self::key(app, Key::Escape);
                self.stage.set(48);
            }
            48 if !app.get_native_dialog_active() => {
                self.check(
                    app.get_presented() && app.get_input_text() == "关机",
                    "power_confirmation_cancel_restores_search",
                )?;
                self.complete(app);
            }
            _ => {}
        }
        Ok(())
    }
    fn complete(&self, app: &AppWindow) {
        self.timer.stop();
        self.passed.set(true);
        eprintln!(
            "event=pages_smoke_result passed=true restart={}",
            self.restart
        );
        app.invoke_quit_requested();
    }
    pub fn finish(&self) -> std::io::Result<bool> {
        self.timer.stop();
        self.sender.borrow_mut().take();
        if let Some(writer) = self.writer.borrow_mut().take() {
            writer
                .join()
                .map_err(|_| std::io::Error::other("pages writer panicked"))??;
        }
        Ok(self.passed.get())
    }
}
