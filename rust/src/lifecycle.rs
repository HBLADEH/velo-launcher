//! Production visibility state and its UI-thread adapter. No product data or
//! filesystem access; the same Slint component/HWND survives every hide/show.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use slint::winit_030::WinitWindowAccessor;
use slint::winit_030::winit::{
    event::KeyEvent,
    keyboard::{KeyCode, ModifiersState, PhysicalKey},
};
use slint::{CloseRequestResponse, ComponentHandle, Model};

use crate::AppWindow;
use crate::measurement::{self, Observer};
use crate::platform::windows::{
    desktop, geometry,
    hotkey::{self, Key, Service},
    instance::Identity,
    window::NativeWindow,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    None,
    Show,
    Hide,
    Record,
}

#[derive(Default)]
struct State {
    native_ready: bool,
    content_ready: bool,
    startup_complete: bool,
    startup_choice: Option<bool>,
    background: bool,
    hotkey_available: bool,
    tray_available: bool,
    visible: bool,
    closing: bool,
    recording: bool,
    generation: u64,
}

impl State {
    fn complete_startup(&mut self) -> Action {
        if !self.native_ready || !self.content_ready || self.startup_complete || self.closing {
            return Action::None;
        }
        self.startup_complete = true;
        if self
            .startup_choice
            .unwrap_or(!self.background || !self.hotkey_available)
        {
            Action::Show
        } else {
            Action::Hide
        }
    }

    fn transition(&mut self, visible: bool) {
        self.startup_choice = Some(visible);
        self.visible = visible;
        self.generation += 1;
        if !visible {
            self.recording = false;
        }
    }

    fn focus_allowed(&self, generation: u64) -> bool {
        self.visible && !self.closing && self.generation == generation
    }

    fn hotkey(&self, active: bool, fullscreen: bool, disable_fullscreen: bool) -> Action {
        if self.closing || !self.hotkey_available {
            Action::None
        } else if self.recording && self.visible && active {
            Action::Record
        } else if self.visible {
            Action::Hide
        } else if disable_fullscreen && fullscreen {
            Action::None
        } else {
            Action::Show
        }
    }

    fn blur(&mut self, owned_foreground: bool, importing: bool, native_dialog: bool) -> Action {
        if owned_foreground || native_dialog {
            return Action::None;
        }
        self.recording = false;
        if importing {
            return Action::None;
        }
        if !self.closing && self.visible && (self.hotkey_available || self.tray_available) {
            Action::Hide
        } else {
            Action::None
        }
    }
}

pub struct Controller {
    app: slint::Weak<AppWindow>,
    state: RefCell<State>,
    native: Cell<Option<NativeWindow>>,
    service: Option<Service>,
    desktop: Option<desktop::Service>,
    positioned: Cell<bool>,
    layout_queued: Cell<bool>,
    current: Cell<Option<Key>>,
    modifiers: Cell<ModifiersState>,
    observer: Option<Observer>,
    diagnostics: bool,
}

impl Controller {
    pub fn connect(
        app: &AppWindow,
        background: bool,
        key: Key,
        identity: &Identity,
        diagnostics: bool,
        observer: Option<Observer>,
    ) -> Rc<Self> {
        let weak = app.as_weak();
        let service = Service::start(move |event| {
            let text = match event {
                hotkey::Event::Pressed(key) => key.to_string(),
                hotkey::Event::Stopped => String::new(),
            };
            // One serialized event-loop queue; no task is spawned per key press.
            let _ = weak.upgrade_in_event_loop(move |app| app.invoke_global_hotkey(text.into()));
        });
        let (service, error) = match service {
            Ok(service) => (Some(service), None),
            Err(error) => (None, Some(error.to_string())),
        };
        let weak = app.as_weak();
        let desktop = desktop::Service::start(identity, move |event| {
            let _ = weak.upgrade_in_event_loop(move |app| app.invoke_desktop_event(event as i32));
        });
        let desktop = match desktop {
            Ok(service) => Some(service),
            Err(error) => {
                app.set_platform_error(format!("托盘及实例唤起不可用: {error}").into());
                None
            }
        };
        let controller = Rc::new(Self {
            app: app.as_weak(),
            state: RefCell::new(State {
                background,
                content_ready: true,
                ..Default::default()
            }),
            native: Cell::new(None),
            service,
            desktop,
            positioned: Cell::new(false),
            layout_queued: Cell::new(false),
            current: Cell::new(None),
            modifiers: Cell::new(ModifiersState::empty()),
            observer,
            diagnostics,
        });
        app.set_hotkey_draft(key.to_string().into());
        if let Some(error) = error {
            app.set_hotkey_error(format!("快捷键服务不可用: {error}").into());
        } else {
            controller.apply_hotkey(&key.to_string());
        }
        let weak = Rc::downgrade(&controller);
        app.on_global_hotkey(move |text| {
            if let Some(controller) = weak.upgrade() {
                controller.global_hotkey(&text);
            }
        });
        let weak = Rc::downgrade(&controller);
        app.on_desktop_event(move |event| {
            if let Some(controller) = weak.upgrade() {
                controller.desktop_event(event);
            }
        });
        let weak = Rc::downgrade(&controller);
        app.on_layout_changed(move || {
            if let Some(controller) = weak.upgrade() {
                controller.queue_layout("content");
            }
        });
        let weak = Rc::downgrade(&controller);
        app.on_drag_requested(move || {
            if let Some(controller) = weak.upgrade()
                && let Some(app) = controller.app.upgrade()
            {
                let success = app
                    .window()
                    .with_winit_window(|window| window.drag_window().is_ok())
                    .unwrap_or(false);
                controller.log("window_drag", &format!("started={success}"));
            }
        });
        let weak = Rc::downgrade(&controller);
        app.on_hide_requested(move || {
            if let Some(controller) = weak.upgrade() {
                controller.hide("escape");
            }
        });
        let weak = Rc::downgrade(&controller);
        app.on_hotkey_apply(move |text| {
            if let Some(controller) = weak.upgrade() {
                controller.apply_hotkey(&text);
            }
        });
        let weak = Rc::downgrade(&controller);
        app.on_hotkey_record(move |enabled| {
            if let Some(controller) = weak.upgrade() {
                controller.record(enabled);
            }
        });
        let weak = Rc::downgrade(&controller);
        app.on_native_interaction_finished(move || {
            if let Some(controller) = weak.upgrade() {
                controller.focused(controller.native.get().is_some_and(NativeWindow::active));
            }
        });
        let weak = Rc::downgrade(&controller);
        app.window().on_close_requested(move || {
            if let Some(controller) = weak.upgrade() {
                let state = controller.state.borrow();
                let recording = state.recording;
                let recoverable = state.hotkey_available || state.tray_available;
                drop(state);
                let modal = controller
                    .app
                    .upgrade()
                    .is_some_and(|app| app.get_native_dialog_active());
                if recording || modal {
                    return CloseRequestResponse::KeepWindowShown;
                }
                if recoverable {
                    controller.hide("close");
                } else {
                    controller.quit();
                }
            }
            CloseRequestResponse::KeepWindowShown
        });
        let weak = Rc::downgrade(&controller);
        app.on_quit_requested(move || {
            if let Some(controller) = weak.upgrade() {
                controller.quit();
            }
        });
        controller
    }

    fn log(&self, name: &str, fields: &str) {
        measurement::event(&self.observer, name, fields);
        if self.diagnostics {
            eprintln!("event={name} {fields}");
        }
    }

    pub fn native_ready(self: &Rc<Self>, hwnd: isize) {
        self.native.set(Some(NativeWindow::new(hwnd)));
        if let Some(desktop) = &self.desktop {
            desktop.set_owner(hwnd);
        }
        let action = {
            let mut state = self.state.borrow_mut();
            state.native_ready = true;
            state.complete_startup()
        };
        self.log(
            "lifecycle_ready",
            &format!(
                "native_visible={} content_ready=true background={} hotkey_available={}",
                self.native.get().unwrap().visible(),
                self.state.borrow().background,
                self.state.borrow().hotkey_available
            ),
        );
        match action {
            Action::Show => self.show("startup"),
            Action::Hide => self.hide("startup"),
            _ => {}
        }
    }

    fn show(self: &Rc<Self>, reason: &str) {
        if self
            .app
            .upgrade()
            .is_some_and(|app| app.get_native_dialog_active())
        {
            return;
        }
        {
            let mut state = self.state.borrow_mut();
            if state.closing {
                return;
            }
            state.startup_choice = Some(true);
            if !state.native_ready || !state.content_ready {
                return;
            }
            state.transition(true);
        }
        let Some(app) = self.app.upgrade() else {
            return;
        };
        self.update_layout("show");
        app.set_presented(true);
        if let Err(error) = app.show() {
            self.state.borrow_mut().transition(false);
            app.set_presented(false);
            app.set_hotkey_error(format!("窗口显示失败: {error}").into());
            self.log("lifecycle_error", "operation=show");
            return;
        }
        if let Some(native) = self.native.get() {
            native.activate();
            // The first activation can precede Slint's native event routing.
            // Synchronize from the real foreground HWND and UI-thread focus,
            // rather than treating visibility or component focus as activation.
            let focused = native.focused();
            app.set_native_focused(focused);
            app.window()
                .dispatch_event(slint::platform::WindowEvent::WindowActiveChanged(focused));
            self.log(
                "native_focus_sync",
                &format!("focused={focused} source=GetForegroundWindow+GetFocus"),
            );
        }
        if app.get_appearance_open() {
            app.invoke_show_appearance(true);
        } else {
            app.invoke_focus_input();
        }
        self.defer_focus();
        self.log(
            "lifecycle_shown",
            &format!(
                "reason={reason} generation={} native_active={} query_bytes={} appearance_open={}",
                self.state.borrow().generation,
                self.native.get().is_some_and(NativeWindow::active),
                app.get_input_text().len(),
                app.get_appearance_open()
            ),
        );
    }

    fn hide(&self, reason: &str) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        if app.get_native_dialog_active() {
            return;
        }
        let mut state = self.state.borrow_mut();
        // A failed hotkey and failed tray must never strand the only window.
        if !state.hotkey_available && !state.tray_available && reason != "startup" {
            return;
        }
        if state.closing {
            return;
        }
        state.transition(false);
        let generation = state.generation;
        drop(state);
        self.record(false);
        app.set_presented(false);
        app.invoke_show_import(false);
        app.set_drop_hovering(false);
        app.set_native_focused(false);
        app.set_native_composing(false);
        app.invoke_suspend_input();
        if app.hide().is_err() {
            self.log("lifecycle_error", "operation=hide");
        }
        self.log(
            "lifecycle_hidden",
            &format!("reason={reason} generation={generation}"),
        );
    }

    pub fn defer_focus(self: &Rc<Self>) {
        let generation = self.state.borrow().generation;
        let weak = Rc::downgrade(self);
        slint::Timer::single_shot(std::time::Duration::ZERO, move || {
            if let Some(controller) = weak.upgrade() {
                let state = controller.state.borrow();
                let allowed = state.focus_allowed(generation);
                drop(state);
                if allowed
                    && controller
                        .native
                        .get()
                        .is_some_and(|native| native.visible() && native.active())
                    && let Some(app) = controller.app.upgrade()
                {
                    app.invoke_focus_input();
                }
            }
        });
    }

    pub fn focused(self: &Rc<Self>, focused: bool) {
        if focused {
            self.defer_focus();
            return;
        }
        // Native activation/ownership settles after Focused(false). A single
        // queued check handles owned popups without blanket IME/blur immunity.
        let generation = self.state.borrow().generation;
        let weak = Rc::downgrade(self);
        slint::Timer::single_shot(std::time::Duration::from_millis(30), move || {
            let Some(controller) = weak.upgrade() else {
                return;
            };
            if controller.state.borrow().generation != generation {
                return;
            }
            let owned = controller.native.get().is_some_and(NativeWindow::active)
                || controller
                    .desktop
                    .as_ref()
                    .is_some_and(desktop::Service::menu_active);
            let importing = controller
                .app
                .upgrade()
                .is_some_and(|app| app.get_import_open());
            let modal = controller
                .app
                .upgrade()
                .is_some_and(|app| app.get_native_dialog_active());
            let action = controller.state.borrow_mut().blur(owned, importing, modal);
            controller.log(
                "lifecycle_blur_check",
                &format!("owned_foreground={owned} importing={importing} native_dialog={modal} action={action:?}"),
            );
            if !owned {
                controller.record(false);
            }
            if action == Action::Hide {
                controller.hide("blur");
            }
        });
    }

    fn global_hotkey(self: &Rc<Self>, text: &str) {
        if self.state.borrow().closing {
            return;
        }
        if text.is_empty() {
            self.state.borrow_mut().hotkey_available = false;
            self.current.set(None);
            if let Some(app) = self.app.upgrade() {
                app.set_hotkey_error("快捷键服务已停止，请重新启动。".into());
            }
            self.show("hotkey_failure");
            return;
        }
        if Key::parse(text).ok() != self.current.get() {
            return;
        }
        let native = self.native.get();
        let Some(app) = self.app.upgrade() else {
            return;
        };
        if app.get_native_dialog_active() {
            self.log("global_hotkey", "action=None native_dialog=true");
            return;
        }
        let action = self.state.borrow().hotkey(
            native.is_some_and(NativeWindow::active),
            native.is_some_and(NativeWindow::foreground_fullscreen),
            app.get_disable_hotkey_fullscreen(),
        );
        self.log("global_hotkey", &format!("action={action:?}"));
        match action {
            Action::Show => self.show("hotkey"),
            Action::Hide => self.hide("hotkey"),
            Action::Record => {
                app.set_hotkey_draft(text.into());
                self.record(false);
            }
            Action::None => {}
        }
    }

    fn desktop_event(self: &Rc<Self>, event: i32) {
        if self.state.borrow().closing {
            return;
        }
        if self
            .app
            .upgrade()
            .is_some_and(|app| app.get_native_dialog_active())
            && [
                desktop::Event::Open,
                desktop::Event::SecondInstance,
                desktop::Event::Settings,
                desktop::Event::Quit,
            ]
            .iter()
            .any(|action| *action as i32 == event)
        {
            self.log(
                "desktop_event_deferred",
                &format!("action={event} native_dialog=true"),
            );
            return;
        }
        self.log("desktop_event", &format!("action={event}"));
        match event {
            n if n == desktop::Event::Open as i32 => self.show("tray"),
            n if n == desktop::Event::SecondInstance as i32 => self.show("second_instance"),
            n if n == desktop::Event::Settings as i32 => {
                if let Some(app) = self.app.upgrade() {
                    app.invoke_show_appearance(true);
                }
                self.show("tray_settings");
            }
            n if n == desktop::Event::Quit as i32 => self.quit(),
            n if n == desktop::Event::TrayRestored as i32 => {
                self.state.borrow_mut().tray_available = true;
                if let Some(app) = self.app.upgrade() {
                    app.set_platform_error("".into());
                }
            }
            n if n == desktop::Event::TrayUnavailable as i32 => {
                self.state.borrow_mut().tray_available = false;
                if let Some(app) = self.app.upgrade() {
                    app.set_platform_error("托盘暂时不可用，请使用快捷键或重新启动。".into());
                }
                if !self.state.borrow().hotkey_available {
                    self.show("tray_failure");
                }
            }
            n if n == desktop::Event::EnvironmentChanged as i32 => {
                self.queue_layout("display_or_resume")
            }
            n if n == desktop::Event::MenuClosed as i32 && self.state.borrow().visible => {
                if self
                    .desktop
                    .as_ref()
                    .is_some_and(desktop::Service::foreground)
                {
                    if let Some(native) = self.native.get() {
                        native.activate();
                    }
                    self.defer_focus();
                } else {
                    self.focused(false);
                }
            }
            _ => {}
        }
    }

    pub fn queue_layout(self: &Rc<Self>, reason: &'static str) {
        if self.layout_queued.replace(true) {
            return;
        }
        let weak = Rc::downgrade(self);
        // Run after winit/Slint have consumed ScaleFactorChanged/Resized.
        let moving = self.native.get().is_some_and(NativeWindow::moving);
        let delay = if moving {
            std::time::Duration::from_millis(50)
        } else {
            std::time::Duration::ZERO
        };
        slint::Timer::single_shot(delay, move || {
            if let Some(controller) = weak.upgrade() {
                controller.layout_queued.set(false);
                // Do not clamp a window back to the old monitor mid-drag.
                if controller.native.get().is_some_and(NativeWindow::moving) {
                    controller.queue_layout("drag_completed");
                    return;
                }
                if controller.positioned.get() && !controller.state.borrow().closing {
                    controller.update_layout(reason);
                }
            }
        });
    }

    fn update_layout(&self, reason: &str) {
        let (Some(native), Some(app)) = (self.native.get(), self.app.upgrade()) else {
            return;
        };
        let height = geometry::content_height(
            app.get_is_home(),
            app.get_appearance_open(),
            app.get_import_open(),
            app.get_result_items().row_count(),
        );
        let Some(layout) = geometry::layout(native, height, self.positioned.get()) else {
            self.log("geometry_error", "operation=monitor_layout");
            return;
        };
        let bounds = layout.bounds;
        let scale = layout.dpi as f32 / 96.0;
        let width = bounds.width as f32 / scale;
        let height = bounds.height as f32 / scale;
        let changed = (app.get_viewport_width() - width).abs() > 0.1
            || (app.get_viewport_height() - height).abs() > 0.1;
        if changed {
            app.set_viewport_width(width);
            app.set_viewport_height(height);
            app.window().set_size(slint::PhysicalSize::new(
                bounds.width as u32,
                bounds.height as u32,
            ));
        }
        let applied = geometry::apply(native, bounds);
        if applied {
            self.positioned.set(true);
        }
        if changed || reason == "show" || reason == "display_or_resume" || reason == "dpi" {
            self.log("window_layout", &format!("reason={reason} applied={applied} dpi={} x={} y={} physical_width={} physical_height={} logical_width={width:.1} logical_height={height:.1} work_x={} work_y={} work_width={} work_height={}", layout.dpi, bounds.x, bounds.y, bounds.width, bounds.height, layout.work.x, layout.work.y, layout.work.width, layout.work.height));
        }
    }

    fn apply_hotkey(&self, text: &str) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        self.record(false);
        let outcome = Key::parse(text).and_then(|key| {
            self.service.as_ref().ok_or("快捷键服务不可用")?.bind(key)?;
            self.current.set(Some(key));
            self.state.borrow_mut().hotkey_available = true;
            app.set_hotkey_current(key.to_string().into());
            app.set_hotkey_draft(key.to_string().into());
            Ok(())
        });
        app.set_hotkey_error(outcome.as_ref().err().map_or("", String::as_str).into());
        self.log(
            "hotkey_bind",
            &format!(
                "success={} previous_retained={}",
                outcome.is_ok(),
                outcome.is_err() && self.current.get().is_some()
            ),
        );
    }

    fn record(&self, enabled: bool) {
        let enabled = enabled
            && self.state.borrow().visible
            && self.native.get().is_some_and(NativeWindow::active);
        let changed = self.state.borrow().recording != enabled;
        self.state.borrow_mut().recording = enabled;
        if let Some(app) = self.app.upgrade() {
            app.set_hotkey_recording(enabled);
        }
        if changed {
            self.log("hotkey_recording", &format!("enabled={enabled}"));
        }
    }

    pub fn modifiers(&self, modifiers: ModifiersState) {
        self.modifiers.set(modifiers);
    }

    pub fn record_key(&self, event: &KeyEvent) -> bool {
        if !self.state.borrow().recording {
            return false;
        }
        if !event.state.is_pressed() || event.repeat {
            return true;
        }
        let Some(app) = self.app.upgrade() else {
            return true;
        };
        if app.get_native_composing() || app.get_has_preedit() {
            return false;
        }
        let PhysicalKey::Code(code) = event.physical_key else {
            return true;
        };
        if code == KeyCode::Escape {
            self.record(false);
            return true;
        }
        let virtual_key = match code {
            KeyCode::Space => 0x20,
            KeyCode::Enter => 0x0d,
            KeyCode::Tab => 9,
            code if (KeyCode::KeyA as u32..=KeyCode::KeyZ as u32).contains(&(code as u32)) => {
                0x41 + code as u32 - KeyCode::KeyA as u32
            }
            code if (KeyCode::Digit0 as u32..=KeyCode::Digit9 as u32).contains(&(code as u32)) => {
                0x30 + code as u32 - KeyCode::Digit0 as u32
            }
            code if (KeyCode::F1 as u32..=KeyCode::F24 as u32).contains(&(code as u32)) => {
                0x70 + code as u32 - KeyCode::F1 as u32
            }
            _ => return true,
        };
        let modifiers = self.modifiers.get();
        let flags = u32::from(modifiers.alt_key())
            | u32::from(modifiers.control_key()) << 1
            | u32::from(modifiers.shift_key()) << 2
            | u32::from(modifiers.super_key()) << 3;
        if let Some(key) = Key::from_native(virtual_key, flags) {
            app.set_hotkey_draft(key.to_string().into());
            self.record(false);
        }
        true
    }

    pub fn quit(&self) {
        if self
            .app
            .upgrade()
            .is_some_and(|app| app.get_native_dialog_active())
        {
            self.log("lifecycle_quit_deferred", "native_dialog=true");
            return;
        }
        self.state.borrow_mut().closing = true;
        if let Some(app) = self.app.upgrade() {
            app.set_presented(false);
            app.invoke_suspend_input();
        }
        self.log("lifecycle_quit", "");
        let _ = slint::quit_event_loop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_waits_for_both_lifecycles_and_only_runs_once() {
        for native_first in [false, true] {
            let mut state = State::default();
            if native_first {
                state.native_ready = true;
            } else {
                state.content_ready = true;
            }
            assert_eq!(state.complete_startup(), Action::None);
            state.native_ready = true;
            state.content_ready = true;
            assert_eq!(state.complete_startup(), Action::Show);
            state.transition(false);
            assert_eq!(state.complete_startup(), Action::None);
            assert!(!state.visible);
        }
    }

    #[test]
    fn background_requires_recovery_and_respects_explicit_startup_actions() {
        for available in [false, true] {
            let mut state = State {
                background: true,
                native_ready: true,
                content_ready: true,
                hotkey_available: available,
                ..Default::default()
            };
            assert_eq!(
                state.complete_startup(),
                if available {
                    Action::Hide
                } else {
                    Action::Show
                }
            );
        }
        for choice in [false, true] {
            let mut state = State {
                background: true,
                startup_choice: Some(choice),
                native_ready: true,
                content_ready: true,
                ..Default::default()
            };
            assert_eq!(
                state.complete_startup(),
                if choice { Action::Show } else { Action::Hide }
            );
        }
    }

    #[test]
    fn hotkey_recording_fullscreen_order_and_blur_follow_the_window_contract() {
        let mut state = State {
            visible: true,
            recording: true,
            hotkey_available: true,
            ..Default::default()
        };
        assert_eq!(state.hotkey(true, true, true), Action::Record);
        assert_eq!(state.blur(true, false, false), Action::None);
        assert!(state.recording);
        assert_eq!(state.blur(false, false, false), Action::Hide);
        assert!(!state.recording);
        state.transition(false);
        let hidden_generation = state.generation;
        assert_eq!(state.hotkey(false, true, true), Action::None);
        assert_eq!(state.hotkey(false, true, false), Action::Show);
        for _ in 0..20 {
            assert_eq!(state.hotkey(false, false, true), Action::Show);
            state.transition(true);
            assert_eq!(state.hotkey(true, false, true), Action::Hide);
            state.transition(false);
        }
        assert!(!state.visible && state.generation > hidden_generation);
        state.hotkey_available = false;
        state.visible = true;
        assert_eq!(state.blur(false, false, false), Action::None);
        state.closing = true;
        assert_eq!(state.complete_startup(), Action::None);
        assert_eq!(state.hotkey(true, false, false), Action::None);
    }

    #[test]
    fn late_focus_and_startup_cannot_reactivate_a_dismissed_window() {
        let mut state = State {
            native_ready: true,
            content_ready: true,
            ..Default::default()
        };
        assert_eq!(state.complete_startup(), Action::Show);
        state.transition(true);
        let first_show = state.generation;
        assert!(state.focus_allowed(first_show));
        state.transition(false);
        assert!(!state.focus_allowed(first_show));
        assert_eq!(state.complete_startup(), Action::None);
        state.transition(true);
        assert!(!state.focus_allowed(first_show));
        assert!(state.focus_allowed(state.generation));
        state.closing = true;
        assert!(!state.focus_allowed(state.generation));
    }

    #[test]
    fn tray_is_a_recovery_route_after_hotkey_failure_but_not_after_both_fail() {
        let mut state = State {
            visible: true,
            tray_available: true,
            ..Default::default()
        };
        assert_eq!(state.blur(false, false, false), Action::Hide);
        state.tray_available = false;
        assert_eq!(state.blur(false, false, false), Action::None);
        state.tray_available = true;
        assert_eq!(state.blur(true, false, false), Action::None);
    }

    #[test]
    fn import_and_owned_modal_have_scoped_blur_protection() {
        let mut state = State {
            visible: true,
            hotkey_available: true,
            recording: true,
            ..Default::default()
        };
        assert_eq!(state.blur(false, true, false), Action::None);
        assert!(!state.recording);
        assert_eq!(state.blur(false, false, true), Action::None);
        assert_eq!(state.blur(true, false, false), Action::None);
        assert_eq!(state.blur(false, false, false), Action::Hide);
        state.transition(false);
        assert_eq!(state.blur(false, true, false), Action::None);
        assert!(!state.visible);
    }
}
