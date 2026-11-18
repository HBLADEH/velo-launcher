//! Opt-in integration check against an isolated synthetic Everything index.
//! Callback checks do not claim to validate physical keyboard or real IME input.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use image::ImageEncoder;
use slint::platform::Key;
use slint::{ComponentHandle, Model};

use crate::AppWindow;

pub struct Controller {
    app: slint::Weak<AppWindow>,
    directory: PathBuf,
    timer: slint::Timer,
    stage: Cell<u8>,
    started: Instant,
    passed: Cell<bool>,
    selected: RefCell<slint::SharedString>,
    pictures: RefCell<Vec<std::thread::JoinHandle<Result<(), String>>>>,
    hidden_since: Cell<Option<Instant>>,
}

impl Controller {
    pub fn start(app: &AppWindow, directory: &Path) -> Rc<Self> {
        let controller = Rc::new(Self {
            app: app.as_weak(),
            directory: directory.into(),
            timer: slint::Timer::default(),
            stage: Cell::new(0),
            started: Instant::now(),
            passed: Cell::new(false),
            selected: RefCell::default(),
            pictures: RefCell::new(Vec::new()),
            hidden_since: Cell::new(None),
        });
        let weak = Rc::downgrade(&controller);
        controller.timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(150),
            move || {
                if let Some(controller) = weak.upgrade() {
                    controller.tick();
                }
            },
        );
        controller
    }

    fn check(&self, passed: bool, name: &str) -> Result<(), String> {
        eprintln!("event=everything_smoke_assert name={name} passed={passed}");
        if passed { Ok(()) } else { Err(name.into()) }
    }
    fn key(app: &AppWindow, key: Key, control: bool) -> bool {
        app.invoke_key_requested(key.into(), control, false, false, false, false)
    }
    fn query(app: &AppWindow, query: &str) {
        app.set_input_text(query.into());
        app.invoke_input_edited(query.into());
    }
    fn picture(&self, app: &AppWindow, name: &str) -> Result<(), String> {
        let pixels = app
            .window()
            .take_snapshot()
            .map_err(|_| "snapshot failed")?;
        let (width, height) = (pixels.width(), pixels.height());
        let bytes = pixels.as_bytes().to_vec();
        let path = self.directory.join(name);
        let writer = std::thread::spawn(move || {
            let file = std::fs::File::create(path).map_err(|_| "create screenshot failed")?;
            image::codecs::png::PngEncoder::new(std::io::BufWriter::new(file))
                .write_image(&bytes, width, height, image::ExtendedColorType::Rgba8)
                .map_err(|_| "encode screenshot failed".to_owned())
        });
        self.pictures.borrow_mut().push(writer);
        Ok(())
    }
    fn tick(&self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let outcome = self.step(&app);
        if self.started.elapsed() > Duration::from_secs(75) || outcome.is_err() {
            eprintln!(
                "event=everything_smoke_result passed=false stage={}",
                self.stage.get()
            );
            self.timer.stop();
            app.invoke_quit_requested();
        }
    }
    fn step(&self, app: &AppWindow) -> Result<(), String> {
        match self.stage.get() {
            0 if app.get_everything_enabled()
                && app.get_presented()
                && app.get_input_has_focus() =>
            {
                self.check(!app.get_file_mode(), "starts_in_application_mode")?;
                self.check(
                    velo_slint::platform::windows::everything::active_workers() == 0,
                    "no_worker_before_first_file_query",
                )?;
                Self::query(app, "Velo 原应用查询");
                self.check(Self::key(app, Key::Tab, false), "tab_switches_mode")?;
                self.stage.set(1);
            }
            1 if app.get_file_mode() => {
                Self::query(app, "Velo文件验证 ext:txt");
                self.stage.set(2);
            }
            2 if !app.get_file_loading() && app.get_file_items().row_count() == 50 => {
                self.check(app.get_file_total() == 125, "real_query_and_total")?;
                self.check(
                    velo_slint::platform::windows::everything::active_workers() == 1,
                    "single_query_worker",
                )?;
                self.check(
                    Self::key(app, Key::DownArrow, false),
                    "file_list_navigation",
                )?;
                *self.selected.borrow_mut() = app.get_selected_id();
                self.picture(app, "files-light.png")?;
                Self::key(app, Key::Tab, false);
                self.stage.set(3);
            }
            3 if !app.get_file_mode() => {
                self.check(
                    app.get_input_text() == "Velo 原应用查询",
                    "restores_application_query",
                )?;
                Self::key(app, Key::Tab, false);
                self.stage.set(4);
            }
            4 if app.get_file_mode()
                && !app.get_file_loading()
                && app.get_file_items().row_count() == 50 =>
            {
                self.check(
                    app.get_input_text() == "Velo文件验证 ext:txt",
                    "restores_file_query",
                )?;
                self.check(
                    app.get_selected_id() == *self.selected.borrow(),
                    "restores_file_selection",
                )?;
                app.set_native_composing(true);
                self.check(
                    !Self::key(app, Key::Tab, false) && !Self::key(app, Key::Return, false),
                    "ime_owns_commands",
                )?;
                app.set_native_composing(false);
                app.set_native_dialog_active(true);
                self.check(!Self::key(app, Key::Tab, false), "native_dialog_owns_tab")?;
                app.set_native_dialog_active(false);
                app.invoke_file_load_more();
                self.stage.set(5);
            }
            5 if !app.get_file_loading() && app.get_file_items().row_count() == 100 => {
                app.invoke_file_load_more();
                self.stage.set(6);
            }
            6 if !app.get_file_loading() && app.get_file_items().row_count() == 125 => {
                self.check(app.get_file_total() == 125, "explicit_three_page_loading")?;
                app.set_dark_theme(true);
                self.stage.set(7);
            }
            7 => {
                self.picture(app, "files-dark.png")?;
                Self::query(app, "Velo文件验证不存在");
                Self::query(app, "Velo文件验证 ext:txt");
                self.stage.set(8);
            }
            8 if !app.get_file_loading() && app.get_file_items().row_count() == 50 => {
                self.check(app.get_file_total() == 125, "latest_query_wins")?;
                app.invoke_file_load_more();
                self.stage.set(9);
            }
            9 if !app.get_file_loading() && app.get_file_items().row_count() == 100 => {
                let row = app.get_file_items().row_data(79).ok_or("missing row 80")?;
                app.set_selected_id(row.path.clone());
                app.invoke_ensure_selection_visible(79);
                *self.selected.borrow_mut() = row.path;
                app.set_presented(false);
                self.hidden_since.set(Some(Instant::now()));
                self.stage.set(10);
            }
            10 => {
                if self
                    .hidden_since
                    .get()
                    .is_some_and(|since| since.elapsed() < Duration::from_secs(31))
                {
                    return Ok(());
                }
                self.check(
                    app.get_file_items().row_count() == 0,
                    "hidden_results_released",
                )?;
                self.check(
                    velo_slint::platform::windows::everything::active_workers() == 0,
                    "idle_query_thread_and_window_released",
                )?;
                app.set_presented(true);
                app.invoke_focus_input();
                self.stage.set(11);
            }
            11 if !app.get_file_loading() && app.get_file_items().row_count() > 0 => {
                self.check(
                    app.get_file_items().row_count() == 100
                        && app.get_selected_id() == *self.selected.borrow(),
                    "hidden_round_trip_restores_two_pages_and_row_80",
                )?;
                self.picture(app, "files-restored.png")?;
                app.invoke_show_appearance(true);
                app.set_settings_tab(3);
                app.set_dark_theme(false);
                self.stage.set(12);
            }
            12 => {
                self.check(
                    app.get_file_items().row_count() == 0,
                    "settings_page_suspends_file_query",
                )?;
                self.picture(app, "everything-settings.png")?;
                app.invoke_show_appearance(false);
                self.stage.set(13);
            }
            13 if !app.get_file_loading() && app.get_file_items().row_count() > 0 => {
                self.check(
                    app.get_file_items().row_count() == 100
                        && app.get_selected_id() == *self.selected.borrow(),
                    "settings_round_trip_restores_two_pages_and_row_80",
                )?;
                app.invoke_show_appearance(true);
                self.stage.set(14);
            }
            14 if app.get_appearance_open() => {
                let mut draft = app.get_settings_draft();
                draft.everything = false;
                app.set_settings_draft(draft);
                app.invoke_settings_save();
                self.stage.set(15);
            }
            15 if !app.get_everything_enabled()
                && !app.get_management_busy()
                && !app.get_appearance_open() =>
            {
                self.check(
                    !app.get_file_mode() && app.get_file_items().row_count() == 0,
                    "disable_clears_mode_and_rows",
                )?;
                self.check(
                    !Self::key(app, Key::Tab, false),
                    "disabled_tab_preserves_focus_navigation",
                )?;
                self.passed.set(true);
                self.timer.stop();
                eprintln!("event=everything_smoke_result passed=true");
                app.invoke_quit_requested();
            }
            _ => {}
        }
        Ok(())
    }
    pub fn finish(&self) -> bool {
        let mut passed = self.passed.get();
        for picture in self.pictures.borrow_mut().drain(..) {
            passed &= picture.join().is_ok_and(|result| result.is_ok());
        }
        passed
    }
}
