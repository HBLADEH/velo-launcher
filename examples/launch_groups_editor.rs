//! Synthetic component check and screenshots; never launches applications or
//! opens a real data directory. Pointer dispatch is not physical drag evidence.
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, mpsc::sync_channel};
use std::time::Duration;

use image::ImageEncoder;
use slint::{ComponentHandle, Model};
use velo_slint::core::{
    catalog::Catalog,
    config::Config,
    data::{ApplicationLibrary, History, IndexCache},
    launch_groups::{Collection, Group, Member},
    model::{AppItem, identity},
    service::Snapshot,
};
use velo_slint::platform;

slint::include_modules!();
#[path = "../src/confirmations.rs"]
mod confirmations;
#[path = "../src/group_editor.rs"]
mod group_editor;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("provide a fresh output directory")?,
    );
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
    app.set_viewport_width(640.0);
    app.set_viewport_height(660.0);
    app.set_reduce_motion(true);
    app.set_presented(true);
    app.set_business_enabled(true);
    // Read only Windows' own executable icons on a background STA. No fixture
    // application is launched; these pixels make the library rows inspectable.
    let windows = PathBuf::from(std::env::var_os("WINDIR").ok_or("WINDIR unavailable")?);
    let icon_pixels = std::thread::spawn(
        move || -> velo_slint::core::Result<Vec<velo_slint::core::icons::Pixels>> {
            let apartment = platform::windows::shell::Apartment::new()?;
            ["explorer.exe", "regedit.exe", r"System32\taskmgr.exe"]
                .into_iter()
                .map(|name| {
                    let path = windows.join(name).to_string_lossy().into_owned();
                    platform::windows::icons::extract(
                        &apartment,
                        &AppItem {
                            path: path.clone(),
                            exec_path: path,
                            ..AppItem::default()
                        },
                    )
                })
                .collect()
        },
    )
    .join()
    .map_err(|_| "icon worker panicked")??;
    let commands = Rc::new(RefCell::new(Vec::new()));
    let capture = commands.clone();
    let confirmations = confirmations::Controller::connect(&app);
    let editor = group_editor::Controller::connect(&app, confirmations, move |command| {
        capture.borrow_mut().push(command);
        false
    });
    let applications: Vec<_> = ["游戏软件", "语音软件", "加速器软件"]
        .into_iter()
        .enumerate()
        .map(|(index, name)| {
            let path = format!(r"C:\SyntheticApps\app{index}.exe");
            AppItem {
                id: identity(&path, ""),
                name: name.into(),
                path: path.clone(),
                exec_path: path,
                ..AppItem::default()
            }
        })
        .collect();
    let library = ApplicationLibrary::empty();
    let existing = Group {
        id: "group:synthetic".into(),
        name: "日常工作".into(),
        members: applications[..2]
            .iter()
            .map(Member::from_app)
            .collect::<Result<_, _>>()?,
        ..Group::default()
    };
    let group_id = existing.id.clone();
    let weak_app = app.as_weak();
    app.on_item_pin_toggled(move |id| {
        assert_eq!(id, group_id);
        let app = weak_app.unwrap();
        let groups = app.get_group_items();
        let mut row = groups.row_data(0).unwrap();
        assert_eq!(row.id, id);
        row.pinned = !row.pinned;
        groups.set_row_data(0, row);
    });
    editor.snapshot(Arc::new(Snapshot {
        catalog: Arc::new(Catalog::new(&applications, &library, &[])),
        config: Arc::new(Config::default()),
        history: Arc::new(History::default()),
        cache: Arc::new(IndexCache::default()),
        library: Arc::new(library),
        groups: Arc::new(Collection::default().with_group(existing)?),
        groups_available: true,
        enabled_workspace: None,
    }));
    app.invoke_show_groups(true);
    app.invoke_group_new();
    app.set_group_name("开黑".into());
    let (sender, receiver) = sync_channel::<(String, u32, u32, Vec<u8>)>(2);
    let writer = std::thread::spawn(move || -> std::io::Result<()> {
        for (name, width, height, bytes) in receiver {
            image::codecs::png::PngEncoder::new(std::fs::File::create(directory.join(name))?)
                .write_image(&bytes, width, height, image::ExtendedColorType::Rgba8)
                .map_err(std::io::Error::other)?;
        }
        Ok(())
    });
    let stage = Rc::new(Cell::new(0));
    let step = stage.clone();
    let weak = app.as_weak();
    let timer = slint::Timer::default();
    let step_commands = commands.clone();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(250),
        move || {
            let Some(app) = weak.upgrade() else {
                return;
            };
            match step.get() {
                0 => {
                    let sources = app.get_group_sources();
                    for (index, pixels) in icon_pixels.iter().enumerate() {
                        let mut row = sources.row_data(index).unwrap();
                        row.icon = slint::Image::from_rgba8(slint::SharedPixelBuffer::<
                            slint::Rgba8Pixel,
                        >::clone_from_slice(
                            &pixels.rgba,
                            pixels.width,
                            pixels.height,
                        ));
                        sources.set_row_data(index, row);
                        assert!(app.invoke_icon_visible(5, index as i32));
                    }
                    app.invoke_group_add(app.get_group_sources().row_data(0).unwrap().id.clone());
                    app.invoke_group_add(app.get_group_sources().row_data(0).unwrap().id);
                    assert_eq!(app.get_group_members().row_count(), 1);
                    use slint::platform::{PointerEventButton, WindowEvent};
                    let start = slint::LogicalPosition::new(60.0, 316.0);
                    let end = slint::LogicalPosition::new(450.0, 316.0);
                    app.window().dispatch_event(WindowEvent::PointerPressed {
                        position: start,
                        button: PointerEventButton::Left,
                    });
                    app.window()
                        .dispatch_event(WindowEvent::PointerMoved { position: end });
                    app.window().dispatch_event(WindowEvent::PointerReleased {
                        position: end,
                        button: PointerEventButton::Left,
                    });
                    assert_eq!(
                        app.get_group_members().row_count(),
                        2,
                        "dragging a source row into members must add exactly one item"
                    );
                    let plus = slint::LogicalPosition::new(286.0, 361.0);
                    app.window().dispatch_event(WindowEvent::PointerPressed {
                        position: plus,
                        button: PointerEventButton::Left,
                    });
                    app.window().dispatch_event(WindowEvent::PointerReleased {
                        position: plus,
                        button: PointerEventButton::Left,
                    });
                    assert_eq!(
                        app.get_group_members().row_count(),
                        3,
                        "the centered add icon must remain clickable"
                    );
                    app.invoke_group_save();
                    let commands = step_commands.borrow();
                    let Some(group_editor::Command::Save(group)) = commands.last() else {
                        panic!("save must submit the drag draft");
                    };
                    assert_eq!(group.name, "开黑");
                    assert_eq!(group.members.len(), 3);
                    app.set_group_notice("已从应用库添加三个应用 · 相同应用只保留一份".into());
                }
                1 | 3 => {
                    let pixels = app.window().take_snapshot().expect("component screenshot");
                    sender
                        .try_send((
                            format!(
                                "{}-editor.png",
                                if step.get() == 1 { "light" } else { "dark" }
                            ),
                            pixels.width(),
                            pixels.height(),
                            pixels.as_bytes().to_vec(),
                        ))
                        .expect("bounded screenshot writer");
                }
                2 => {
                    app.set_dark_theme(true);
                }
                4 => {
                    app.invoke_group_close();
                    assert!(app.get_confirmation_open());
                    app.invoke_confirmation_answered(false);
                    assert_eq!(app.get_group_mode(), 1);
                    app.invoke_group_close();
                    app.invoke_confirmation_answered(true);
                    assert_eq!(app.get_group_mode(), 0);
                    editor.managed("group_delete", false);
                    editor.detection_ready(
                        velo_slint::core::launch_groups::Draft::default().ticket(),
                        Ok(r"C:\SyntheticApps\app0.exe".into()),
                    );
                    editor.report(velo_slint::core::group_run::Report {
                        run_id: 1,
                        group_id: "group:synthetic".into(),
                        name: "开黑".into(),
                        done: true,
                        history_saved: true,
                        entries: applications
                            .iter()
                            .enumerate()
                            .map(|(index, item)| velo_slint::core::group_run::Entry {
                                app_id: item.id.clone(),
                                name: item.name.clone(),
                                state: if index == 1 {
                                    velo_slint::core::group_run::State::AlreadyRunning
                                } else {
                                    velo_slint::core::group_run::State::Submitted
                                },
                                message: "",
                            })
                            .collect(),
                    });
                    app.set_group_mode(2);
                    app.set_group_notice("合成结果示例，未启动任何应用".into());
                }
                5 => {
                    let pixels = app.window().take_snapshot().expect("result screenshot");
                    sender
                        .try_send((
                            "dark-results.png".into(),
                            pixels.width(),
                            pixels.height(),
                            pixels.as_bytes().to_vec(),
                        ))
                        .expect("bounded screenshot writer");
                }
                6 => app.set_group_mode(0),
                7 => {
                    use slint::platform::{PointerEventButton, WindowEvent};
                    let position = slint::LogicalPosition::new(463.0, 195.0);
                    for event in [
                        WindowEvent::PointerPressed {
                            position,
                            button: PointerEventButton::Left,
                        },
                        WindowEvent::PointerReleased {
                            position,
                            button: PointerEventButton::Left,
                        },
                    ] {
                        app.window().dispatch_event(event);
                    }
                    assert!(app.get_group_items().row_data(0).unwrap().pinned);
                    assert_eq!(app.get_group_mode(), 0);
                    let pixels = app
                        .window()
                        .take_snapshot()
                        .expect("pinned group screenshot");
                    sender
                        .try_send((
                            "dark-pinned.png".into(),
                            pixels.width(),
                            pixels.height(),
                            pixels.as_bytes().to_vec(),
                        ))
                        .expect("bounded screenshot writer");
                }
                _ => {
                    let _ = slint::quit_event_loop();
                }
            }
            step.set(step.get() + 1);
        },
    );
    app.show()?;
    slint::run_event_loop_until_quit()?;
    timer.stop();
    drop(timer);
    app.hide()?;
    writer.join().map_err(|_| "screenshot writer panicked")??;
    if stage.get() != 9 {
        return Err("editor check did not finish".into());
    }
    // Reference other command payloads so this shared adapter example also
    // type-checks every callback when clippy checks all targets.
    for command in commands.borrow().iter() {
        match command {
            group_editor::Command::Save(group) => {
                assert!(!group.id.is_empty());
            }
            group_editor::Command::Delete(id) => {
                let _ = id;
            }
            group_editor::Command::Detection(ticket, path) => {
                let _ = (ticket, path);
            }
            group_editor::Command::Run { id, only, force } => {
                let _ = (id, only, force);
            }
            group_editor::Command::Cancel => {}
        }
    }
    println!(
        "event=launch_group_editor_check passed=true renderer={renderer} synthetic=true captures=4"
    );
    Ok(())
}
