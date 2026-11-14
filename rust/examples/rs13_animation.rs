//! Bounded component recordings with actual snapshot timestamps. These use
//! synthetic component events, not desktop input or presented-frame timings.
use image::ImageEncoder;
use serde::Serialize;
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition};
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::sync_channel;
use std::time::{Duration, Instant};

slint::include_modules!();
#[path = "support/rs13_fixture.rs"]
mod fixture;

const CLIPS: &[&str] = &[
    "home-entry",
    "settings-enter",
    "settings-exit",
    "custom-enter",
    "custom-exit",
    "tile-hover",
    "tile-press",
    "gear-hover",
    "scanning",
    "recording",
    "reduced-scanning",
    "hidden-scanning",
    "reduced-hover",
];
const TICKS: usize = 46;
const SETTLE: usize = 10;

#[derive(Serialize)]
struct Frame {
    clip: &'static str,
    name: String,
    elapsed_us: u128,
    snapshot_us: u128,
    active_animations: bool,
    width: u32,
    height: u32,
}

fn pointer(app: &AppWindow, x: f32, y: f32) {
    app.window().dispatch_event(WindowEvent::PointerMoved {
        position: LogicalPosition::new(x, y),
    });
}

fn click(app: &AppWindow, x: f32, y: f32) {
    pointer(app, x, y);
    for pressed in [true, false] {
        let position = LogicalPosition::new(x, y);
        let button = PointerEventButton::Left;
        app.window().dispatch_event(if pressed {
            WindowEvent::PointerPressed { position, button }
        } else {
            WindowEvent::PointerReleased { position, button }
        });
    }
}

#[derive(Default)]
struct Actions {
    save: Cell<u32>,
    browse: Cell<u32>,
    refresh: Cell<u32>,
}

fn setup(app: &AppWindow, clip: &str) {
    app.set_presented(true);
    app.window().dispatch_event(WindowEvent::PointerExited);
    app.window().dispatch_event(WindowEvent::PointerReleased {
        position: LogicalPosition::new(-20.0, -20.0),
        button: PointerEventButton::Left,
    });
    fixture::prepare(
        app,
        match clip {
            "home-entry" => "home-two-rows",
            "settings-exit" => "settings-0",
            "custom-exit" => "custom",
            "scanning" | "reduced-scanning" | "hidden-scanning" => "scanning",
            "recording" => "recording",
            _ => "home",
        },
        false,
    );
    // Evaluate the resting animated properties during the settling frames.
    // Otherwise reduced-motion's short circuit can leave a lazy property's
    // first read at the final value, producing a falsely static recording.
    app.set_reduce_motion(clip.starts_with("reduced-"));
    // Park focus to separate decoration activity from the text caret clock.
    app.invoke_suspend_input();
    match clip {
        "home-entry" | "hidden-scanning" => app.set_presented(false),
        "tile-press" => pointer(app, 70.0, 230.0),
        _ => {}
    }
}

fn trigger(app: &AppWindow, clip: &str, actions: &Actions) -> Result<(), &'static str> {
    app.set_reduce_motion(clip.starts_with("reduced-"));
    match clip {
        "home-entry" => app.set_presented(true),
        "settings-enter" => {
            app.invoke_show_appearance(true);
            if !app.get_appearance_open() || app.get_input_has_focus() {
                return Err("settings must take focus immediately, during its transition");
            }
            let previous = actions.save.get();
            click(app, 560.0, 626.0);
            if actions.save.get() != previous + 1 {
                return Err("settings cannot receive a click during its first transition frame");
            }
        }
        "settings-exit" => {
            app.invoke_show_appearance(false);
            if app.get_appearance_open() || !app.get_input_has_focus() {
                return Err("closing settings must restore search focus immediately");
            }
            let previous = actions.refresh.get();
            click(app, 612.0, 638.0);
            if actions.refresh.get() != previous + 1 {
                return Err("outgoing settings intercepts a click on the launcher");
            }
        }
        "custom-enter" => {
            app.invoke_show_import(true);
            if !app.get_import_open() || app.get_input_has_focus() {
                return Err("import must take focus immediately, during its transition");
            }
            let previous = actions.browse.get();
            click(app, 562.0, 158.0);
            if actions.browse.get() != previous + 1 {
                return Err("import cannot receive a click during its first transition frame");
            }
        }
        "custom-exit" => {
            app.invoke_show_import(false);
            if app.get_import_open() || !app.get_input_has_focus() {
                return Err("closing import must restore search focus immediately");
            }
            let previous = actions.refresh.get();
            let old_browse = actions.browse.get();
            click(app, 612.0, 638.0);
            if actions.refresh.get() != previous + 1 || actions.browse.get() != old_browse {
                return Err("outgoing import intercepts a click on the launcher");
            }
        }
        "tile-hover" | "reduced-hover" => pointer(app, 70.0, 230.0),
        "tile-press" => app.window().dispatch_event(WindowEvent::PointerPressed {
            position: LogicalPosition::new(70.0, 230.0),
            button: PointerEventButton::Left,
        }),
        "gear-hover" => pointer(app, 603.0, 40.0),
        _ => {}
    }
    app.invoke_suspend_input();
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args_os().skip(1);
    let directory = PathBuf::from(arguments.next().ok_or("provide a fresh output directory")?);
    if arguments.next().is_some() {
        return Err("expected only an output directory".into());
    }
    std::fs::create_dir(&directory)?;
    let renderer = if cfg!(feature = "renderer-software") {
        "software"
    } else {
        "femtovg"
    };
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name(renderer.into())
        .select()?;
    let app = AppWindow::new()?;
    let actions = Rc::new(Actions::default());
    let save = actions.clone();
    app.on_settings_save(move || save.save.set(save.save.get() + 1));
    let browse = actions.clone();
    app.on_import_native_requested(move |_| browse.browse.set(browse.browse.get() + 1));
    let refresh = actions.clone();
    app.on_refresh_requested(move || refresh.refresh.set(refresh.refresh.get() + 1));
    app.set_renderer_label(format!("Slint 1.18.1 · winit · {renderer}").into());
    app.set_presented(true);
    app.on_quit_requested(|| {
        let _ = slint::quit_event_loop();
    });
    app.on_hide_requested(|| {
        let _ = slint::quit_event_loop();
    });
    app.show()?;
    let (sender, receiver) = sync_channel::<(String, u32, u32, Vec<u8>)>(2);
    let image_directory = directory.clone();
    let writer = std::thread::Builder::new()
        .name("velo-animation-output".into())
        .spawn(move || -> std::io::Result<()> {
            for (name, width, height, bytes) in receiver {
                let file = std::fs::File::create(image_directory.join(name))?;
                image::codecs::png::PngEncoder::new(std::io::BufWriter::new(file))
                    .write_image(&bytes, width, height, image::ExtendedColorType::Rgba8)
                    .map_err(std::io::Error::other)?;
            }
            Ok(())
        })?;
    let stage = Rc::new(Cell::new(0usize));
    let records = Rc::new(RefCell::new(Vec::<Frame>::new()));
    let failure = Rc::new(RefCell::new(None));
    let started = Rc::new(Cell::new(Instant::now()));
    let weak = app.as_weak();
    let step = stage.clone();
    let output = records.clone();
    let error = failure.clone();
    let clock = started.clone();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(40),
        move || {
            let Some(app) = weak.upgrade() else {
                return;
            };
            let index = step.get() / TICKS;
            let tick = step.get() % TICKS;
            if index == CLIPS.len() {
                let _ = slint::quit_event_loop();
                return;
            }
            let clip = CLIPS[index];
            let result = (|| -> Result<(), String> {
                if tick == 0 {
                    setup(&app, clip);
                }
                if tick == 1 {
                    fixture::settle(&app, clip);
                }
                if tick == SETTLE {
                    clock.set(Instant::now());
                    trigger(&app, clip, &actions)?;
                }
                if tick >= SETTLE {
                    let elapsed_us = clock.get().elapsed().as_micros();
                    let snapshot = Instant::now();
                    let pixels = app
                        .window()
                        .take_snapshot()
                        .map_err(|_| "animation snapshot failed")?;
                    let snapshot_us = snapshot.elapsed().as_micros();
                    let name = format!("{clip}-{:03}.png", tick - SETTLE);
                    sender
                        .try_send((
                            name.clone(),
                            pixels.width(),
                            pixels.height(),
                            pixels.as_bytes().to_vec(),
                        ))
                        .map_err(|_| "animation snapshot queue full or closed")?;
                    output.borrow_mut().push(Frame {
                        clip,
                        name,
                        elapsed_us,
                        snapshot_us,
                        active_animations: app.window().has_active_animations(),
                        width: pixels.width(),
                        height: pixels.height(),
                    });
                }
                Ok(())
            })();
            if let Err(reason) = result {
                *error.borrow_mut() = Some(reason);
                let _ = slint::quit_event_loop();
            } else {
                step.set(step.get() + 1);
            }
        },
    );
    slint::run_event_loop_until_quit()?;
    timer.stop();
    drop(timer);
    app.hide()?;
    writer.join().map_err(|_| "animation writer panicked")??;
    let complete = stage.get() == CLIPS.len() * TICKS && failure.borrow().is_none();
    std::fs::write(
        directory.join("recording.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "Renderer": renderer, "Complete": complete, "Failure": *failure.borrow(),
            "SyntheticInput": true, "ComponentSnapshots": true, "PresentedFrames": false,
            "NominalIntervalMilliseconds": 40, "FixtureScenes": fixture::SCENES,
            "Clips": CLIPS, "Frames": *records.borrow(),
            "Scope": "Timestamped component snapshots; encoding and capture add overhead. Not physical input, native Wails, desktop frames or product performance."
        }))?,
    )?;
    if !complete {
        return Err(failure
            .borrow()
            .clone()
            .unwrap_or("recording interrupted".into())
            .into());
    }
    println!(
        "event=animation_capture_complete renderer={renderer} clips={} frames={} synthetic=true",
        CLIPS.len(),
        records.borrow().len()
    );
    Ok(())
}
