#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(any(
    all(feature = "renderer-femtovg", feature = "renderer-software"),
    not(any(feature = "renderer-femtovg", feature = "renderer-software"))
))]
compile_error!("Enable exactly one of renderer-femtovg or renderer-software");

use std::cell::{Cell, RefCell};
use std::error::Error;
use std::rc::Rc;
use std::time::Duration;
use std::time::Instant;

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::winit_030::winit::platform::windows::WindowAttributesExtWindows;
use slint::winit_030::{EventResult, WinitWindowAccessor, winit::event::WindowEvent};
use slint::{ComponentHandle, GraphicsAPI, RenderingState};

slint::include_modules!();

mod fixtures;
mod import_preview;
mod input;
mod interaction_probe;
mod lifecycle;
mod measurement;
mod navigation;
mod platform;
mod ui;

const RENDERER: &str = if cfg!(feature = "renderer-femtovg") {
    "femtovg"
} else {
    "software"
};

#[derive(Default)]
struct Probe {
    hwnd: Option<isize>,
    redraw_events: u32,
    opengl_frame: bool,
    dropped_files: u32,
    logical_size_ok: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let started = Instant::now();
    let mut diagnostics = false;
    let mut smoke_test = false;
    let mut input_smoke_test = false;
    let mut dark_theme = false;
    let mut reduce_motion = false;
    let mut empty_pinned = false;
    let mut initial_query = String::new();
    let mut measurement_log = None;
    let mut measurement_stdio = false;
    let mut isolated = false;
    let mut background = false;
    let mut hotkey = platform::windows::hotkey::Key::parse("Alt+Space")?;
    let mut allow_hotkey_fullscreen = false;
    let mut instance_id = None;
    let mut data_dir = None;
    for argument in std::env::args().skip(1) {
        match argument.as_str() {
            "--diagnostics" => diagnostics = true,
            "--smoke-test" => smoke_test = true,
            "--input-smoke-test" => {
                smoke_test = true;
                input_smoke_test = true;
            }
            "--theme=light" => dark_theme = false,
            "--theme=dark" => dark_theme = true,
            "--reduce-motion" => reduce_motion = true,
            "--empty-pinned" => empty_pinned = true,
            "--measurement-stdio" => measurement_stdio = true,
            "--isolated" => isolated = true,
            "--background" => background = true,
            "--allow-hotkey-fullscreen" => allow_hotkey_fullscreen = true,
            "--version" => {
                println!(
                    "velo-slint {} slint=1.18.1 backend=winit renderer={RENDERER} target={}",
                    env!("CARGO_PKG_VERSION"),
                    env!("VELO_BUILD_TARGET")
                );
                return Ok(());
            }
            "--help" | "-h" => {
                println!(
                    "velo-slint [--background] [--hotkey=Alt+Space] [--allow-hotkey-fullscreen] [--instance-id=ID --data-dir=PATH] [--diagnostics] [--isolated] [--smoke-test|--input-smoke-test] [--theme=light|dark] [--reduce-motion] [--empty-pinned] [--query=TEXT] [--measurement-log=PATH [--measurement-stdio]] [--version]"
                );
                return Ok(());
            }
            _ => {
                if let Some(query) = argument.strip_prefix("--query=") {
                    initial_query = query.to_owned();
                } else if let Some(path) = argument.strip_prefix("--measurement-log=") {
                    measurement_log = Some(std::path::PathBuf::from(path));
                } else if let Some(text) = argument.strip_prefix("--hotkey=") {
                    hotkey = platform::windows::hotkey::Key::parse(text)?;
                } else if let Some(id) = argument.strip_prefix("--instance-id=") {
                    instance_id = Some(id.to_owned());
                } else if let Some(path) = argument.strip_prefix("--data-dir=") {
                    data_dir = Some(std::path::PathBuf::from(path));
                } else {
                    return Err(format!("Unknown argument: {argument}").into());
                }
            }
        }
    }
    isolated |= smoke_test || measurement_stdio;
    if isolated && background {
        return Err("--background cannot be combined with isolated measurement/smoke modes".into());
    }
    if measurement_stdio && measurement_log.is_none() {
        return Err("--measurement-stdio requires --measurement-log=PATH".into());
    }
    if measurement_log.is_some() && (smoke_test || !initial_query.is_empty()) {
        return Err(
            "Measurement requires real input; do not combine with smoke tests or --query".into(),
        );
    }
    let identity = platform::windows::instance::Identity::resolve(instance_id, data_dir)?;
    let _instance_guard = if isolated {
        None
    } else {
        use platform::windows::instance::Acquisition;
        match identity.acquire()? {
            Acquisition::Primary(guard) => Some(guard),
            Acquisition::Forwarded => return Ok(()),
        }
    };
    let measurement = measurement_log
        .as_deref()
        .map(|path| measurement::Observer::open(path, started))
        .transpose()?;
    measurement::event(
        &measurement,
        "main_started",
        &format!("renderer={RENDERER} backend=winit"),
    );

    // Both variants compile exactly one renderer. BackendSelector alone can fall back
    // in Slint 1.18.1; feature isolation prevents a different renderer being sampled.
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name(RENDERER.into())
        .with_winit_window_attributes_hook(move |attributes| {
            attributes
                .with_visible(false)
                .with_class_name("VeloSlintLauncherWindow")
                .with_resizable(false)
                .with_skip_taskbar(!isolated)
        })
        .select()?;
    let app = AppWindow::new()?;
    ui::connect(&app, diagnostics, empty_pinned, measurement.clone());
    let imports = import_preview::Controller::connect(&app, diagnostics, measurement.clone());
    app.set_dark_theme(dark_theme);
    app.set_reduce_motion(reduce_motion);
    app.set_lifecycle_enabled(!isolated);
    app.set_disable_hotkey_fullscreen(!allow_hotkey_fullscreen);
    app.set_input_text(initial_query.clone().into());
    app.invoke_input_edited(initial_query.into());
    app.set_renderer_label(format!("Slint 1.18.1 · winit · {RENDERER}（独立构建）").into());
    let probe = Rc::new(RefCell::new(Probe::default()));
    measurement::event(&measurement, "component_created", "");
    let lifecycle = (!isolated).then(|| {
        lifecycle::Controller::connect(
            &app,
            background,
            hotkey,
            &identity,
            diagnostics,
            measurement.clone(),
        )
    });

    if cfg!(feature = "renderer-femtovg") {
        let rendering_probe = probe.clone();
        let rendering_measurement = measurement.clone();
        app.window().set_rendering_notifier(move |state, api| {
            if matches!(state, RenderingState::AfterRendering)
                && matches!(api, GraphicsAPI::NativeOpenGL { .. })
                && !rendering_probe.borrow().opengl_frame
            {
                rendering_probe.borrow_mut().opengl_frame = true;
                eprintln!("event=renderer_observed renderer=femtovg graphics_api=NativeOpenGL");
                measurement::event(
                    &rendering_measurement,
                    "first_opengl_frame",
                    "graphics_api=NativeOpenGL",
                );
            }
        })?;
    }

    let event_probe = probe.clone();
    let weak_app = app.as_weak();
    let mut composition = input::Composition::default();
    let event_measurement = measurement.clone();
    let event_lifecycle = lifecycle.clone();
    let event_imports = imports.clone();
    app.window().on_winit_window_event(move |_, event| {
        if let Some(app) = weak_app.upgrade() {
            observe_native(&app, &event_probe, &event_measurement);
        }
        match event {
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                measurement::event(
                    &event_measurement,
                    "native_dpi_changed",
                    &format!("scale={scale_factor}"),
                );
                if diagnostics {
                    eprintln!("event=native_dpi_changed scale={scale_factor}");
                }
                if let Some(controller) = &event_lifecycle {
                    controller.queue_layout("dpi");
                }
            }
            WindowEvent::Moved(_) | WindowEvent::Resized(_) => {
                if let Some(controller) = &event_lifecycle {
                    controller.queue_layout("native_geometry");
                }
            }
            WindowEvent::RedrawRequested => {
                if event_probe.borrow().redraw_events == 0 {
                    measurement::event(
                        &event_measurement,
                        "first_redraw_requested",
                        "completed_frame=false",
                    );
                }
                event_probe.borrow_mut().redraw_events += 1;
            }
            WindowEvent::KeyboardInput { event, .. } if event.state.is_pressed() => {
                measurement::event(&event_measurement, "native_key_down", "");
                if event_lifecycle
                    .as_ref()
                    .is_some_and(|controller| controller.record_key(event))
                {
                    return EventResult::PreventDefault;
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                if let Some(controller) = &event_lifecycle {
                    controller.modifiers(modifiers.state());
                }
            }
            WindowEvent::HoveredFile(_) => {
                if let Some(app) = weak_app.upgrade() {
                    app.set_drop_hovering(app.get_presented() && !app.get_native_dialog_active());
                }
            }
            WindowEvent::HoveredFileCancelled => {
                if let Some(app) = weak_app.upgrade() {
                    app.set_drop_hovering(false);
                }
            }
            WindowEvent::DroppedFile(path) => {
                event_probe.borrow_mut().dropped_files += 1;
                let count = event_probe.borrow().dropped_files;
                eprintln!("event=file_dropped count={count}");
                event_imports.add_paths(vec![path.clone()], "ole_drop");
            }
            WindowEvent::Focused(focused) => {
                measurement::event(
                    &event_measurement,
                    "native_focus",
                    &format!("focused={focused}"),
                );
                if let Some(app) = weak_app.upgrade() {
                    app.set_native_focused(*focused);
                    if let Some(controller) = &event_lifecycle {
                        controller.focused(*focused);
                    } else if *focused {
                        let focus_app = app.as_weak();
                        // Defer until Slint has processed native activation. Check
                        // state again so a late callback cannot steal focus.
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(app) = focus_app.upgrade()
                                && app.get_native_focused()
                                && app.window().is_visible()
                            {
                                app.invoke_focus_input();
                            }
                        });
                    }
                    if diagnostics {
                        eprintln!("event=window_focus focused={focused}");
                    }
                }
            }
            WindowEvent::Ime(event) => {
                let active = composition.update(event);
                if let Some(app) = weak_app.upgrade() {
                    app.set_native_composing(active);
                }
                // Keep user text and file paths out of diagnostics.
                let kind = match event {
                    slint::winit_030::winit::event::Ime::Enabled => "enabled",
                    slint::winit_030::winit::event::Ime::Disabled => "disabled",
                    slint::winit_030::winit::event::Ime::Preedit(..) => "preedit",
                    slint::winit_030::winit::event::Ime::Commit(..) => "commit",
                };
                if diagnostics {
                    eprintln!("event=ime kind={kind} composing={active}");
                }
            }
            _ => {}
        }
        EventResult::Propagate
    });
    if isolated {
        app.on_quit_requested(|| {
            let _ = slint::quit_event_loop();
        });
        app.window().on_close_requested(|| {
            let _ = slint::quit_event_loop();
            slint::CloseRequestResponse::KeepWindowShown
        });
    }
    let focus_measurement = measurement.clone();
    app.on_input_focus_changed(move |focused| {
        measurement::event(
            &focus_measurement,
            "input_focus",
            &format!("focused={focused}"),
        );
        if diagnostics {
            eprintln!("event=input_focus focused={focused}");
        }
    });

    let smoke_result = Rc::new(Cell::new(false));
    let input_result = Rc::new(Cell::new(!input_smoke_test));
    if input_smoke_test {
        let result = input_result.clone();
        let weak_app = app.as_weak();
        slint::Timer::single_shot(Duration::from_millis(500), move || {
            if let Some(app) = weak_app.upgrade() {
                let check = interaction_probe::run(&app);
                eprintln!(
                    "event=input_smoke_result passed={} error={:?}",
                    check.is_ok(),
                    check.as_ref().err()
                );
                result.set(check.is_ok());
            }
        });
    }
    if smoke_test {
        let result = smoke_result.clone();
        let smoke_probe = probe.clone();
        let weak_app = app.as_weak();
        slint::Timer::single_shot(Duration::from_secs(2), move || {
            if let Some(app) = weak_app.upgrade() {
                let snapshot = app.window().take_snapshot();
                let rendered = snapshot.is_ok_and(|pixels| {
                    pixels.width() > 0
                        && pixels.height() > 0
                        && pixels.as_slice().windows(2).any(|pair| pair[0] != pair[1])
                });
                let state = smoke_probe.borrow();
                let passed = state.hwnd.is_some()
                    && input_result.get()
                    && state.redraw_events > 0
                    && state.logical_size_ok
                    && rendered
                    && (RENDERER == "software" || state.opengl_frame);
                eprintln!(
                    "event=smoke_result passed={passed} backend=winit renderer={RENDERER} hwnd={} redraw_events={} logical_size_640x540={} nonuniform_snapshot={rendered} opengl_frame={}",
                    state.hwnd.is_some(),
                    state.redraw_events,
                    state.logical_size_ok,
                    state.opengl_frame
                );
                result.set(passed);
            }
            let _ = slint::quit_event_loop();
        });
    }

    if let Some(controller) = &lifecycle {
        let controller = controller.clone();
        let weak_app = app.as_weak();
        let ready_probe = probe.clone();
        let ready_measurement = measurement.clone();
        slint::spawn_local(async move {
            let Some(app) = weak_app.upgrade() else {
                return;
            };
            // AppWindow creates its native window hidden. No show/hide flash is
            // needed to acquire HWND, including in --background mode.
            let native = app.window().winit_window().await;
            let hwnd = native.ok().and_then(|native| {
                let RawWindowHandle::Win32(handle) = native.window_handle().ok()?.as_raw() else {
                    return None;
                };
                Some(handle.hwnd.get())
            });
            if let Some(hwnd) = hwnd {
                observe_native(&app, &ready_probe, &ready_measurement);
                // The accessor future can resolve inside native creation,
                // before winit has registered the HWND for focus events.
                slint::Timer::single_shot(Duration::ZERO, move || controller.native_ready(hwnd));
            } else {
                eprintln!("event=lifecycle_error operation=native_window");
                controller.quit();
            }
        })?;
    } else {
        app.set_presented(true);
        app.show()?;
        app.invoke_focus_input();
        measurement::event(&measurement, "show_returned", "");
    }
    if measurement_stdio {
        measurement::connect_stdio(&app, measurement.as_ref().unwrap().clone());
    }
    slint::run_event_loop_until_quit()?;
    app.set_presented(false);
    app.hide()?;
    app.window()
        .on_winit_window_event(|_, _| EventResult::Propagate);
    drop(lifecycle);
    drop(imports);
    if smoke_test && !smoke_result.get() {
        return Err("Window/renderer smoke test failed (or closed before completion)".into());
    }
    Ok(())
}

fn observe_native(
    app: &AppWindow,
    probe: &Rc<RefCell<Probe>>,
    measurement: &Option<measurement::Observer>,
) {
    if probe.borrow().hwnd.is_some() {
        return;
    }
    let native = app.window().with_winit_window(|native| {
        let RawWindowHandle::Win32(handle) = native.window_handle().ok()?.as_raw() else {
            return None;
        };
        Some((
            handle.hwnd.get(),
            native.scale_factor(),
            native.inner_size(),
        ))
    });
    if let Some(Some((hwnd, scale, size))) = native {
        let mut probe = probe.borrow_mut();
        probe.hwnd = Some(hwnd);
        probe.logical_size_ok = (f64::from(size.width) / scale - 640.0).abs() <= 1.0
            && (f64::from(size.height) / scale - 540.0).abs() <= 1.0;
        drop(probe);
        measurement::event(
            measurement,
            "native_window",
            &format!(
                "hwnd={hwnd:X} scale={scale} physical_width={} physical_height={}",
                size.width, size.height
            ),
        );
        eprintln!(
            "event=native_window backend=winit renderer={RENDERER} hwnd={hwnd:#x} scale={scale} physical_width={} physical_height={}",
            size.width, size.height
        );
        app.set_native_status(
            format!(
                "HWND 已取得 · 缩放 {scale} · 客户区 {} × {}",
                size.width, size.height
            )
            .into(),
        );
    }
}
