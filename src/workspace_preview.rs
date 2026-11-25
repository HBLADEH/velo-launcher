//! UI-thread controller for the Workspace windows.

use std::cell::{Cell, RefCell};
use std::io;
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::winit_030::winit::{
    event::WindowEvent,
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};
use slint::winit_030::{EventResult, WinitWindowAccessor};
use slint::{CloseRequestResponse, ComponentHandle, PhysicalPosition, PhysicalSize, Timer};
use velo_slint::core::workspace::config::WorkspaceConfig;
use velo_slint::platform::windows::{
    geometry::Layout,
    hotkey::Key,
    window::NativeWindow,
    workspace::{self, Foreground, Invocation, Region},
    workspace_actions::Resolver,
    workspace_input::{Button, Event},
};

use crate::{AppWindow, WorkspaceWindow, measurement::Observer};

#[path = "workspace_preview/bindings.rs"]
mod bindings;
#[path = "workspace_preview/dual.rs"]
mod dual;
use bindings::{Bindings, Keyboard, Message, Mouse, Prepared};
#[path = "workspace_preview/editor.rs"]
mod editor;
use editor::Editor;

pub const DEFAULT_HOTKEY: Key = Key {
    modifiers: 3,
    code: 0x20,
};

thread_local! {
    // Slint's hook runs when this component asks for its adapter, before an
    // HWND exists or the backend reads the component's title.
    static CREATING_WINDOW: Cell<bool> = const { Cell::new(false) };
}

pub fn creating_window() -> bool {
    CREATING_WINDOW.get()
}

pub struct Preview {
    pub key: Key,
    pub button: Option<Button>,
}

#[derive(Clone, Copy, Default)]
enum Phase {
    #[default]
    Hidden,
    Opening,
    Shown(Option<Foreground>),
    Pad,
}

#[derive(Default)]
struct State {
    generation: u64,
    phase: Phase,
}

impl State {
    fn begin(&mut self) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.phase = Phase::Opening;
        self.generation
    }

    fn is_opening(&self, generation: u64) -> bool {
        self.generation == generation && matches!(self.phase, Phase::Opening)
    }

    fn visible_generation(&self) -> Option<u64> {
        matches!(self.phase, Phase::Shown(_) | Phase::Pad).then_some(self.generation)
    }

    fn hide(&mut self) -> Phase {
        std::mem::take(&mut self.phase)
    }
}

pub struct Controller {
    bindings: RefCell<Bindings>,
    pending: RefCell<Option<Prepared>>,
    editor: Editor,
    dual: dual::Dual,
    execution_epoch: Arc<AtomicU64>,
    launcher: slint::Weak<AppWindow>,
    business: Weak<crate::business::Controller>,
    window: RefCell<Option<WorkspaceWindow>>,
    state: RefCell<State>,
    native: Cell<Option<NativeWindow>>,
    region: Cell<Option<Region>>,
    resolver: RefCell<Option<Resolver>>,
    preview: bool,
    observer: Option<Observer>,
    diagnostics: bool,
}

impl Controller {
    pub fn connect(
        launcher: &AppWindow,
        business: Weak<crate::business::Controller>,
        preview: Option<Preview>,
        observer: Option<Observer>,
        diagnostics: bool,
    ) -> Rc<Self> {
        let controller = Rc::new(Self {
            bindings: RefCell::default(),
            pending: RefCell::new(None),
            editor: Editor::default(),
            dual: dual::Dual::default(),
            execution_epoch: Arc::default(),
            launcher: launcher.as_weak(),
            business,
            window: RefCell::new(None),
            state: RefCell::default(),
            native: Cell::new(None),
            region: Cell::new(None),
            resolver: RefCell::new(None),
            preview: preview.is_some(),
            observer,
            diagnostics,
        });
        let weak = Rc::downgrade(&controller);
        launcher.on_workspace_event(move || {
            let Some(controller) = weak.upgrade() else {
                return;
            };
            let message = controller
                .bindings
                .borrow()
                .keyboard
                .as_ref()
                .and_then(Keyboard::next);
            match message {
                Some(Message::Invoke(trigger)) => controller.toggle(trigger),
                Some(Message::Stopped) => {
                    controller.bindings.borrow_mut().keyboard.take();
                    controller.hide("hotkey_stopped", false);
                    controller.report_hotkey_error("快捷键服务已停止");
                }
                None => {}
            }
        });
        let weak = Rc::downgrade(&controller);
        launcher.on_workspace_mouse_event(move || {
            if let Some(controller) = weak.upgrade() {
                controller.drain_mouse();
            }
        });
        let weak = Rc::downgrade(&controller);
        // Let the launcher acquire its stored chord before attempting ours.
        launcher.on_lifecycle_started(move || {
            if let Some(controller) = weak.upgrade() {
                controller.start_bindings(preview.as_ref());
            }
        });
        let weak = Rc::downgrade(&controller);
        launcher.on_workspace_saved(move |saved, message| {
            if let Some(controller) = weak.upgrade() {
                controller.finish_settings(saved);
                controller.editor_saved(saved);
                if let Some(app) = controller.launcher.upgrade() {
                    app.set_workspace_notice(message);
                }
            }
        });
        let weak = Rc::downgrade(&controller);
        launcher.on_workspace_input_suspended(move |suspended| {
            if let Some(controller) = weak.upgrade() {
                controller.suspend_input(suspended);
            }
        });
        let weak = Rc::downgrade(&controller);
        launcher.on_workspace_environment_changed(move || {
            if let Some(controller) = weak.upgrade() {
                controller.hide("display_changed", true);
            }
        });
        controller.connect_editor(launcher);
        controller
    }

    fn start_bindings(&self, preview: Option<&Preview>) {
        let config = if preview.is_some() {
            self.committed_config()
        } else {
            self.enabled_config()
        };
        let (key, button) = if let Some(preview) = preview {
            (Ok(preview.key), preview.button)
        } else if let Some(config) = &config {
            (
                Key::parse(&config.hotkey),
                config.mouse.map(Button::from_preference),
            )
        } else {
            return;
        };
        match key.and_then(|key| Keyboard::reserve(key, self.launcher.clone())) {
            Ok(keyboard) => {
                keyboard.active.store(true, Ordering::Release);
                self.bindings.borrow_mut().keyboard = Some(keyboard);
                self.log("workspace_hotkey_ready", "");
            }
            Err(error) => self.report_hotkey_error(&error),
        }
        if let Some(button) = button {
            match Mouse::reserve(button, self.launcher.clone()) {
                Ok(mouse) => {
                    if let Some(config) = config {
                        mouse.admission.set_excluded(config.excluded.clone());
                    }
                    mouse.admission.enabled.store(true, Ordering::Release);
                    self.bindings.borrow_mut().mouse = Some(mouse);
                    self.log("workspace_mouse_ready", &format!("button={button:?}"));
                }
                Err(error) => {
                    self.log("workspace_mouse_error", &format!("error={error}"));
                    if let Some(launcher) = self.launcher.upgrade() {
                        launcher.set_workspace_notice(
                            "工作台侧键不可用，请检查设置或使用备用快捷键".into(),
                        );
                    }
                }
            }
        }
    }

    fn committed_config(&self) -> Option<Arc<WorkspaceConfig>> {
        self.business.upgrade()?.workspace_config()
    }

    fn enabled_config(&self) -> Option<Arc<WorkspaceConfig>> {
        self.business.upgrade()?.enabled_workspace()
    }

    fn apply_bindings(&self, prepared: Prepared) {
        self.execution_epoch.fetch_add(1, Ordering::AcqRel);
        let enabled = prepared.enabled;
        self.hide("settings_changed", true);
        let old = self.bindings.borrow_mut().commit(prepared);
        drop(old);
        self.suspend_input(self.launcher.upgrade().is_some_and(|app| {
            app.get_native_dialog_active()
                || app.get_hotkey_recording()
                || app.get_workspace_recording()
        }));
        if !enabled {
            self.window.borrow_mut().take();
            self.native.set(None);
            self.region.set(None);
            self.resolver.borrow_mut().take();
        }
    }

    fn finish_settings(&self, saved: bool) {
        let prepared = self.pending.borrow_mut().take();
        if let (true, Some(prepared)) = (saved, prepared) {
            self.apply_bindings(prepared);
        }
    }

    fn suspend_input(&self, suspended: bool) {
        if let Some(mouse) = &self.bindings.borrow().mouse {
            mouse
                .admission
                .suspended
                .store(suspended, Ordering::Release);
        }
        if suspended && self.dual.pending() {
            self.hide("launcher_modal", false);
        }
    }

    fn save_settings(&self, mut config: WorkspaceConfig) -> Result<(), String> {
        if self.pending.borrow().is_some() {
            return Err("工作台设置正在保存".into());
        }
        config.validate().map_err(|_| "应用配置或槽位数量有误")?;
        let prepared = self
            .bindings
            .borrow()
            .prepare(&config, self.launcher.clone())?;
        self.execution_epoch.fetch_add(1, Ordering::AcqRel);
        if self
            .business
            .upgrade()
            .is_some_and(|business| business.save_workspace(config))
        {
            self.pending.replace(Some(prepared));
            Ok(())
        } else {
            Err("Velo 正在处理其他任务，请稍候".into())
        }
    }

    fn window(&self) -> Option<WorkspaceWindow> {
        self.window
            .borrow()
            .as_ref()
            .map(ComponentHandle::clone_strong)
    }

    fn report_hotkey_error(&self, error: &str) {
        let message = format!("工作台快捷键不可用：{error}");
        if let Some(launcher) = self.launcher.upgrade() {
            launcher.set_workspace_notice(message.as_str().into());
        }
        self.log("workspace_hotkey_error", &format!("error={error}"));
        if !self.diagnostics {
            eprintln!("{message}");
        }
    }

    fn log(&self, event: &str, fields: &str) {
        if let Some(observer) = &self.observer {
            observer.event(event, fields);
        }
        if self.diagnostics {
            eprintln!("event={event} {fields}");
        }
    }

    fn toggle(self: &Rc<Self>, trigger: Invocation) {
        if let Some(app) = self.launcher.upgrade()
            && app.get_workspace_recording()
        {
            if let Some(keyboard) = &self.bindings.borrow().keyboard {
                self.append_chord(&app, &keyboard.key.to_string());
            }
            return;
        }
        if self.dual.pending() || !matches!(self.state.borrow().phase, Phase::Hidden) {
            self.hide("hotkey", true);
            return;
        }
        let Some(launcher) = self.launcher.upgrade() else {
            return;
        };
        if launcher.get_native_dialog_active() || launcher.get_hotkey_recording() {
            return;
        }
        self.request_context(trigger);
        self.open_dual_panel(trigger, trigger.at);
    }

    fn drain_mouse(self: &Rc<Self>) {
        let events = self
            .bindings
            .borrow()
            .mouse
            .as_ref()
            .map(|mouse| mouse.service.drain())
            .unwrap_or_default();
        for event in events {
            match event {
                Event::Pressed(invocation) => self.dual_press(invocation),
                Event::Moved(sample) => self.dual_move(sample),
                Event::Released(sample) => self.dual_release(sample),
                Event::Cancelled(reason) => self.cancel_side_input(reason),
                Event::Stopped => {
                    self.cancel_side_input("mouse_stopped");
                    self.bindings.borrow_mut().mouse.take();
                    self.log("workspace_mouse_error", "reason=worker_stopped");
                }
            }
        }
    }

    fn cancel_side_input(&self, reason: &'static str) {
        if self.dual.pending() {
            self.hide(reason, false);
        }
        self.log("workspace_mouse_cancelled", &format!("reason={reason}"));
    }

    fn open(
        self: &Rc<Self>,
        trigger: Invocation,
        layout: Layout,
        launcher: &AppWindow,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let cold = self.window.borrow().is_none();
        if cold {
            let window = CREATING_WINDOW.with(|flag| {
                flag.set(true);
                let window = WorkspaceWindow::new().inspect(|window| {
                    window.set_reduce_motion(true);
                    let _ = window.window();
                });
                flag.set(false);
                window
            })?;
            self.connect_window(&window);
            self.window.replace(Some(window));
        }
        let Some(window) = self.window() else {
            return Ok(());
        };
        window.set_dark_theme(launcher.get_dark_theme());
        let scene = self.dual.scene.borrow().clone().ok_or("工作台视图不可用")?;
        scene.show(&window);
        self.region.set(None);
        if !window.window().has_winit_window() {
            let bounds = layout.bounds;
            window
                .window()
                .set_position(PhysicalPosition::new(bounds.x, bounds.y));
            window
                .window()
                .set_size(PhysicalSize::new(bounds.width as u32, bounds.height as u32));
        }
        let generation = self.state.borrow_mut().begin();
        // Reused windows can present in this input turn without scheduling the
        // adapter creation future and another zero-duration timer.
        if let Some(native) = self.native.get() {
            return self.reveal(&window, Some(native), layout, trigger, false);
        }
        let view = window.as_weak();
        let weak = Rc::downgrade(self);
        slint::spawn_local(async move {
            let Some(view) = view.upgrade() else { return };
            let native = view
                .window()
                .winit_window()
                .await
                .as_deref()
                .ok()
                .and_then(native_window);
            // Winit can still be registering the new HWND when this future resolves.
            Timer::single_shot(Duration::ZERO, move || {
                let Some(controller) = weak.upgrade() else {
                    return;
                };
                if !controller.state.borrow().is_opening(generation) {
                    return;
                }
                if let Err(error) = controller.reveal(&view, native, layout, trigger, cold) {
                    controller.hide("reveal_error", false);
                    controller.log(
                        "workspace_error",
                        &format!("operation=reveal error={error}"),
                    );
                }
            });
        })?;
        Ok(())
    }

    fn reveal(
        self: &Rc<Self>,
        view: &WorkspaceWindow,
        native: Option<NativeWindow>,
        layout: Layout,
        trigger: Invocation,
        cold: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if !workspace::current_input_desktop() {
            self.hide("input_desktop_changed", false);
            return Ok(());
        }
        if !trigger.display_unchanged() {
            self.hide("display_changed", false);
            return Ok(());
        }
        if Foreground::current() != trigger.foreground {
            self.hide("foreground_changed", false);
            return Ok(());
        }
        let native = native.ok_or("窗口句柄不可用")?;
        if !self
            .native
            .get()
            .is_some_and(|old| old.hwnd() == native.hwnd())
        {
            self.region.set(None);
            workspace::prepare_frame(native)?;
            self.native.set(Some(native));
        }
        let scene = self.dual.scene.borrow().clone().ok_or("工作台视图不可用")?;
        let pad = scene.is_pad();
        workspace::set_passive(native, pad)?;
        workspace::apply_layout(native, layout)?;
        self.update_region(native)?;
        if Foreground::current() != trigger.foreground {
            self.hide("foreground_changed", false);
            return Ok(());
        }
        // Record the original target before any show or activation event.
        self.state.borrow_mut().phase = if pad {
            Phase::Pad
        } else {
            Phase::Shown(trigger.foreground)
        };
        view.show()?;
        if pad {
            if Foreground::current() != trigger.foreground {
                self.hide("passive_activation_failed", false);
                return Ok(());
            }
        } else {
            native.activate();
            if !native.active() {
                self.hide("activation_failed", false);
                return Ok(());
            }
        }
        scene.report_shown(self);
        self.log(
            "workspace_shown",
            &format!(
                "cold={cold} dpi={} invoke_us={} captured={}",
                layout.dpi,
                trigger.at.elapsed().as_micros(),
                trigger.foreground.is_some()
            ),
        );
        Ok(())
    }

    fn update_region(&self, native: NativeWindow) -> io::Result<()> {
        let region = Region::read(native)?;
        if self.region.get() != Some(region)
            && let Some(scene) = self.dual.scene.borrow().as_ref()
        {
            region.apply(native, scene.center_only())?;
            self.region.set(Some(region));
        }
        Ok(())
    }

    fn connect_window(self: &Rc<Self>, view: &WorkspaceWindow) {
        let weak = Rc::downgrade(self);
        view.window().on_close_requested(move || {
            if let Some(controller) = weak.upgrade() {
                controller.hide("close", true);
            }
            CloseRequestResponse::KeepWindowShown
        });
        let weak = Rc::downgrade(self);
        view.on_panel_activate(move |index| {
            if let Some(controller) = weak.upgrade() {
                controller.dual_activate(index);
            }
        });
        let weak = Rc::downgrade(self);
        view.window().on_winit_window_event(move |_, event| {
            let Some(controller) = weak.upgrade() else {
                return EventResult::Propagate;
            };
            match event {
                WindowEvent::KeyboardInput { event, .. }
                    if event.physical_key == PhysicalKey::Code(KeyCode::Escape)
                        && event.state.is_pressed() =>
                {
                    controller.hide("escape", true);
                    return EventResult::PreventDefault;
                }
                WindowEvent::Focused(false)
                    if !matches!(controller.state.borrow().phase, Phase::Pad) =>
                {
                    controller.after_window_event(true)
                }
                WindowEvent::ScaleFactorChanged { .. }
                    if controller.dual.pending()
                        && controller.state.borrow().visible_generation().is_some() =>
                {
                    controller.hide("dpi_changed", true);
                }
                WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                    controller.after_window_event(false);
                }
                _ => {}
            }
            EventResult::Propagate
        });
    }

    fn request_context(&self, trigger: Invocation) {
        if trigger.foreground.is_none() && trigger.hovered.is_none() {
            return;
        }
        if self.resolver.borrow().is_none() {
            match Resolver::start() {
                Ok(resolver) => {
                    self.resolver.replace(Some(resolver));
                }
                Err(error) => self.log("workspace_context_error", &format!("error={error}")),
            }
        }
        if let Some(resolver) = self.resolver.borrow().as_ref() {
            resolver.request(trigger);
        }
    }

    fn after_window_event(self: &Rc<Self>, blur: bool) {
        let Some(generation) = self.state.borrow().visible_generation() else {
            return;
        };
        let weak = Rc::downgrade(self);
        Timer::single_shot(Duration::ZERO, move || {
            let Some(controller) = weak.upgrade() else {
                return;
            };
            if controller.state.borrow().visible_generation() != Some(generation) {
                return;
            }
            let Some(native) = controller.native.get() else {
                return;
            };
            if blur {
                if !native.active() {
                    controller.hide("blur", false);
                }
            } else if let Err(error) = controller.update_region(native) {
                controller.hide("region_error", false);
                controller.log(
                    "workspace_error",
                    &format!("operation=region error={error}"),
                );
            }
        });
    }

    fn hide(&self, reason: &str, restore_focus: bool) {
        self.dual.clear();
        let phase = self.state.borrow_mut().hide();
        if matches!(phase, Phase::Hidden) {
            return;
        }
        // Transfer while Workspace still owns foreground permission. A blur
        // from an outside click stays with the window the user picked.
        let restored = match (restore_focus, phase, self.native.get()) {
            (true, Phase::Shown(Some(target)), Some(native)) if native.active() => {
                target.restore(native)
            }
            _ => false,
        };
        if let Some(view) = self.window()
            && let Err(error) = view.hide()
        {
            self.log("workspace_error", &format!("operation=hide error={error}"));
        }
        self.log(
            "workspace_hidden",
            &format!("reason={reason} restored={restored}"),
        );
    }
}

impl Drop for Controller {
    fn drop(&mut self) {
        self.execution_epoch.fetch_add(1, Ordering::AcqRel);
    }
}

fn native_window(window: &Window) -> Option<NativeWindow> {
    let RawWindowHandle::Win32(handle) = window.window_handle().ok()?.as_raw() else {
        return None;
    };
    Some(NativeWindow::new(handle.hwnd.get()))
}

fn native_window_from_app(app: &AppWindow) -> Option<NativeWindow> {
    app.window().with_winit_window(native_window).flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_open_and_blur_expire_across_rapid_hide_and_reopen() {
        let mut state = State::default();
        let first = state.begin();
        assert!(state.is_opening(first));
        state.hide();
        assert!(!state.is_opening(first));
        let second = state.begin();
        assert!(!state.is_opening(first));
        state.phase = Phase::Shown(None);
        assert_eq!(state.visible_generation(), Some(second));
        state.hide();
        let third = state.begin();
        assert!(!state.is_opening(second));
        state.phase = Phase::Shown(None);
        assert_eq!(state.visible_generation(), Some(third));
    }
}
