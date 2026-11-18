//! Static renderer snapshots of synthetic states. No store, Shell, hotkey or tray.
//! These are component captures, not monitor frames or physical input evidence.
use image::ImageEncoder;
use slint::ComponentHandle;
use std::cell::Cell;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::sync_channel;
use std::time::Duration;

slint::include_modules!();

#[path = "support/rs13_fixture.rs"]
mod fixture;
use fixture::{SCENES, prepare};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args_os().skip(1);
    let directory = PathBuf::from(arguments.next().ok_or("provide a fresh output directory")?);
    if arguments.next().is_some() {
        return Err("expected only an output directory".into());
    }
    std::fs::create_dir(&directory)?;
    std::fs::write(
        directory.join("scenes.json"),
        serde_json::to_vec_pretty(&SCENES)?,
    )?;
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
    let writer = std::thread::Builder::new()
        .name("velo-visual-output".into())
        .spawn(move || -> std::io::Result<()> {
            for (name, width, height, bytes) in receiver {
                let file = std::fs::File::create(directory.join(name))?;
                image::codecs::png::PngEncoder::new(std::io::BufWriter::new(file))
                    .write_image(&bytes, width, height, image::ExtendedColorType::Rgba8)
                    .map_err(std::io::Error::other)?;
            }
            Ok(())
        })?;
    let stage = Rc::new(Cell::new(0usize));
    let geometries = Rc::new(RefCell::new(Vec::new()));
    let geometry_output = geometries.clone();
    let geometry_directory = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("missing directory")?;
    let step = stage.clone();
    let weak = app.as_weak();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(250),
        move || {
            let Some(app) = weak.upgrade() else {
                return;
            };
            let index = step.get() / 3;
            if index >= SCENES.len() * 2 {
                let _ = slint::quit_event_loop();
                return;
            }
            let scene = SCENES[index % SCENES.len()];
            let dark = index >= SCENES.len();
            if step.get().is_multiple_of(3) {
                prepare(&app, scene, dark);
            } else if step.get() % 3 == 1 {
                fixture::settle(&app, scene);
            } else {
                if scene == "settings-6-bottom"
                    && (!app.get_licenses_expanded() || app.get_settings_scroll_y() >= 0.0)
                {
                    eprintln!("event=visual_capture_failed reason=license_bottom_state");
                    let _ = slint::quit_event_loop();
                    return;
                }
                if scene.starts_with("settings-") {
                    use slint::Model;
                    geometry_output.borrow_mut().push(serde_json::json!({
                        "scene": format!("{}-{scene}", if dark { "dark" } else { "light" }),
                        "bounds": app.invoke_settings_geometry().iter().map(|r| serde_json::json!({
                            "name": r.name.as_str(), "x": r.x, "y": r.y,
                            "width": r.width, "height": r.height
                        })).collect::<Vec<_>>()
                    }));
                }
                let result = app
                    .window()
                    .take_snapshot()
                    .map_err(|_| "snapshot failed")
                    .and_then(|pixels| {
                        sender
                            .try_send((
                                format!("{}-{scene}.png", if dark { "dark" } else { "light" }),
                                pixels.width(),
                                pixels.height(),
                                pixels.as_bytes().to_vec(),
                            ))
                            .map_err(|_| "snapshot queue closed or full")
                    });
                if let Err(reason) = result {
                    eprintln!("event=visual_capture_failed reason={reason}");
                    let _ = slint::quit_event_loop();
                    return;
                }
            }
            step.set(step.get() + 1);
        },
    );
    slint::run_event_loop_until_quit()?;
    timer.stop();
    drop(timer); // Release the sender before joining its bounded writer.
    app.hide()?;
    writer.join().map_err(|_| "visual writer panicked")??;
    std::fs::write(
        geometry_directory.join("geometry.json"),
        serde_json::to_vec_pretty(&*geometries.borrow())?,
    )?;
    if stage.get() != SCENES.len() * 6 {
        return Err("visual capture incomplete".into());
    }
    println!(
        "event=visual_capture_complete renderer={renderer} captures={} reduced_motion=true synthetic=true",
        SCENES.len() * 2
    );
    Ok(())
}
