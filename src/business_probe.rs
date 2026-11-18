//! Explicit diagnostics for the real business adapter. Component events are
//! synthetic; screenshots and state assertions do not establish native input.

use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;
use std::sync::mpsc::{SyncSender, sync_channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::AppWindow;
use image::ImageEncoder;
use slint::{ComponentHandle, Model};

pub const QUERY: &str = "RS10 启动验证";
type Screenshot = (&'static str, u32, u32, Vec<u8>);

pub struct Controller {
    app: slint::Weak<AppWindow>,
    timer: slint::Timer,
    stage: Cell<u8>,
    revision: Cell<i32>,
    started: Instant,
    passed: Cell<bool>,
    screenshots: RefCell<Option<SyncSender<Screenshot>>>,
    writer: RefCell<Option<JoinHandle<std::io::Result<()>>>>,
}

impl Controller {
    pub fn start(app: &AppWindow, directory: &Path) -> std::io::Result<Rc<Self>> {
        let (sender, receiver) = sync_channel::<Screenshot>(2);
        let directory = directory.to_owned();
        let writer = std::thread::Builder::new()
            .name("velo-business-probe-output".into())
            .spawn(move || {
                for (name, width, height, pixels) in receiver {
                    let file = std::fs::File::create(directory.join(name))?;
                    image::codecs::png::PngEncoder::new(std::io::BufWriter::new(file))
                        .write_image(&pixels, width, height, image::ExtendedColorType::Rgba8)
                        .map_err(std::io::Error::other)?;
                }
                Ok(())
            })?;
        let controller = Rc::new(Self {
            app: app.as_weak(),
            timer: slint::Timer::default(),
            stage: Cell::new(0),
            revision: Cell::new(0),
            started: Instant::now(),
            passed: Cell::new(false),
            screenshots: RefCell::new(Some(sender)),
            writer: RefCell::new(Some(writer)),
        });
        let weak = Rc::downgrade(&controller);
        controller.timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(100),
            move || {
                if let Some(controller) = weak.upgrade() {
                    controller.tick();
                }
            },
        );
        Ok(controller)
    }

    fn assert(&self, value: bool, name: &str) -> Result<(), &'static str> {
        eprintln!("event=business_smoke_assert name={name} passed={value}");
        if value {
            Ok(())
        } else {
            Err("business assertion failed")
        }
    }
    fn picture(&self, app: &AppWindow, name: &'static str) -> Result<(), &'static str> {
        let pixels = app
            .window()
            .take_snapshot()
            .map_err(|_| "snapshot failed")?;
        self.screenshots
            .borrow()
            .as_ref()
            .ok_or("snapshot writer closed")?
            .try_send((
                name,
                pixels.width(),
                pixels.height(),
                pixels.as_bytes().to_vec(),
            ))
            .map_err(|_| "snapshot queue failed")
    }
    fn query(app: &AppWindow, text: &str) {
        app.set_input_text(text.into());
        app.invoke_input_edited(text.into());
    }
    fn has_icon(app: &AppWindow) -> bool {
        app.get_result_items()
            .row_data(0)
            .is_some_and(|row| row.icon.size().width > 0)
    }

    fn tick(&self) {
        let Some(app) = self.app.upgrade() else {
            self.timer.stop();
            return;
        };
        let result = if self.started.elapsed() > Duration::from_secs(45) {
            Err("business UI timeout")
        } else {
            self.step(&app)
        };
        if let Err(error) = result {
            eprintln!("event=business_smoke_result passed=false reason={error}");
            self.timer.stop();
            app.invoke_quit_requested();
        }
    }
    fn step(&self, app: &AppWindow) -> Result<(), &'static str> {
        match self.stage.get() {
            0 if !app.get_scanning() && Self::has_icon(app) => {
                self.assert(
                    app.get_presented()
                        && app.get_input_text() == QUERY
                        && app.get_result_items().row_count() == 1,
                    "real_catalog_query_and_visible_icon",
                )?;
                self.picture(app, "business-light.png")?;
                let selected = app.get_selected_id();
                let key = slint::SharedString::from(slint::platform::Key::DownArrow);
                app.window()
                    .dispatch_event(slint::platform::WindowEvent::KeyPressed { text: key.clone() });
                app.window()
                    .dispatch_event(slint::platform::WindowEvent::KeyReleased { text: key });
                self.assert(
                    app.get_selected_id() == selected,
                    "real_result_component_navigation",
                )?;
                app.set_dark_theme(true);
                self.revision.set(app.get_business_revision());
                app.invoke_refresh_requested();
                Self::query(app, "zz-rs10-no-results-无匹配");
                self.stage.set(1);
            }
            1 if app.get_business_revision() > self.revision.get() && !app.get_scanning() => {
                self.assert(
                    app.get_input_text() == "zz-rs10-no-results-无匹配"
                        && app.get_result_items().row_count() == 0,
                    "refresh_preserves_newer_query",
                )?;
                Self::query(app, QUERY);
                self.stage.set(2);
            }
            2 if Self::has_icon(app) => {
                self.picture(app, "business-dark.png")?;
                self.revision.set(app.get_business_revision());
                app.invoke_hide_requested();
                self.assert(
                    !app.get_presented() && !app.window().is_visible(),
                    "normal_lifecycle_hide",
                )?;
                app.invoke_refresh_requested();
                self.stage.set(3);
            }
            3 if app.get_business_revision() > self.revision.get() && !app.get_scanning() => {
                self.assert(
                    !app.get_presented() && !app.window().is_visible(),
                    "late_refresh_does_not_show_hidden_window",
                )?;
                self.assert(
                    app.get_input_text() == QUERY,
                    "hide_and_refresh_keep_session_query",
                )?;
                self.assert(
                    !Self::has_icon(app),
                    "hidden_models_release_image_references",
                )?;
                self.timer.stop();
                self.passed.set(true);
                eprintln!("event=business_smoke_result passed=true");
                app.invoke_quit_requested();
            }
            _ => {}
        }
        Ok(())
    }

    pub fn finish(&self) -> std::io::Result<bool> {
        self.timer.stop();
        self.screenshots.borrow_mut().take();
        if let Some(writer) = self.writer.borrow_mut().take() {
            writer
                .join()
                .map_err(|_| std::io::Error::other("snapshot writer panicked"))??;
        }
        Ok(self.passed.get())
    }
}
