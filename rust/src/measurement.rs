//! Opt-in, bounded measurement instrumentation. No product data or input text.

use std::io::{BufRead, Write};
use std::path::Path;
use std::sync::{Arc, mpsc};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use slint::ComponentHandle;
use slint::winit_030::WinitWindowAccessor;

use crate::AppWindow;

#[derive(Clone)]
pub struct Observer {
    started: Instant,
    sender: mpsc::SyncSender<String>,
    dropped: Arc<std::sync::atomic::AtomicU32>,
}

impl Observer {
    pub fn open(path: &Path, started: Instant) -> std::io::Result<Self> {
        // Never overwrite earlier evidence. File I/O runs on a bounded worker.
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        let (sender, receiver) = mpsc::sync_channel::<String>(128);
        std::thread::spawn(move || {
            for line in receiver {
                if writeln!(file, "{line}")
                    .and_then(|()| file.flush())
                    .is_err()
                {
                    break;
                }
            }
        });
        Ok(Self {
            started,
            sender,
            dropped: Arc::default(),
        })
    }

    pub fn event(&self, name: &str, fields: &str) {
        use std::sync::atomic::Ordering;
        let unix_us = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros();
        let line = format!(
            "event={name} pid={} unix_us={unix_us} elapsed_us={} dropped={} {fields}",
            std::process::id(),
            self.started.elapsed().as_micros(),
            self.dropped.load(Ordering::Relaxed)
        );
        if self.sender.try_send(line).is_err() {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn input(&self, app: &AppWindow, bytes: usize) {
        if bytes > 0 {
            self.event(
                "input_accepted",
                &format!(
                    "utf8_bytes={bytes} focused={} native_focused={} visible={} composing={}",
                    app.get_input_has_focus(),
                    app.get_native_focused(),
                    app.get_presented(),
                    app.get_native_composing() || app.get_has_preedit()
                ),
            );
        }
    }
}

pub fn event(observer: &Option<Observer>, name: &str, fields: &str) {
    if let Some(observer) = observer {
        observer.event(name, fields);
    }
}

/// An inherited anonymous pipe is the only controller. It adds no listening
/// socket, polling timer, hotkey or production window-lifecycle behavior.
pub fn connect_stdio(app: &AppWindow, observer: Observer) {
    let weak_app = app.as_weak();
    std::thread::spawn(move || {
        for command in std::io::stdin().lock().lines() {
            let Ok(command) = command else { break };
            let weak_app = weak_app.clone();
            let observer = observer.clone();
            if slint::invoke_from_event_loop(move || {
                let Some(app) = weak_app.upgrade() else { return };
                match command.as_str() {
                    "hide" => {
                        app.set_presented(false);
                        app.window().with_winit_window(|window| window.set_visible(false));
                    }
                    "show" => {
                        app.window().with_winit_window(|window| {
                            window.set_visible(true);
                            window.focus_window();
                        });
                        app.set_presented(true);
                        app.invoke_focus_input();
                    }
                    "clear" => app.invoke_clear_input(),
                    "warm-bottom" => app.invoke_ensure_selection_visible(27),
                    "warm-top" => app.invoke_ensure_selection_visible(0),
                    "status" => {}
                    "quit" => { let _ = slint::quit_event_loop(); }
                    _ => {
                        println!("event=measurement_error reason=unknown_command");
                        return;
                    }
                }
                let visible = app.window().with_winit_window(|window| window.is_visible()).flatten();
                let fields = format!("command={command} visible={} presented={} focused={} native_focused={} utf8_bytes={}",
                    visible.unwrap_or(false), app.get_presented(), app.get_input_has_focus(),
                    app.get_native_focused(), app.get_input_text().len());
                observer.event("measurement_state", &fields);
                println!("event=measurement_state {fields}");
            }).is_err() { break; }
        }
        // The owning collector going away must not leave a test window behind.
        let _ = slint::invoke_from_event_loop(|| {
            let _ = slint::quit_event_loop();
        });
    });
}
