//! UI-thread controller for the opt-in Workspace prototype.

use std::cell::{Cell, RefCell};
use std::io;
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::winit_030::winit::{
    event::{MouseButton, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};
use slint::winit_030::{EventResult, WinitWindowAccessor};
use slint::{CloseRequestResponse, ComponentHandle, PhysicalPosition, PhysicalSize, Timer};
use velo_slint::core::workspace::{
    actions::WorkspaceAction,
    config::{Layouts, WorkspaceConfig},
};
use velo_slint::platform::windows::{
    geometry::Layout,
    hotkey::Key,
    preferences,
    window::NativeWindow,
    workspace::{self, Foreground, Invocation, Region, Shape},
    workspace_actions::Resolver,
    workspace_input::Button,
};

use crate::{AppWindow, WorkspaceWindow, measurement::Observer};

#[path = "workspace_preview/radial.rs"]
mod radial;
use radial::{Display, Scene};
#[path = "workspace_preview/gestures.rs"]
mod gestures;
use gestures::Hold;
#[path = "workspace_preview/bindings.rs"]
mod bindings;
use bindings::{Bindings, Keyboard, Message, Mouse, Prepared};
#[path = "workspace_preview/editor.rs"]
mod editor;
use editor::Editor;

pub const DEFAULT_HOTKEY: Key = Key {
    modifiers: 3,
    code: 0x20,
};
const INTRO_DURATION: Duration = Duration::from_millis(48);

thread_local! {
    // Slint's hook runs when this component asks for its adapter, before an
    // HWND exists or the backend reads the component's title.
    static CREATING_WINDOW: Cell<bool> = const { Cell::new(false) };
}

pub fn creating_window() -> bool {
    CREATING_WINDOW.get()
}

pub struct Preview {
    pub shape: Shape,
    pub key: Key,
    pub button: Option<Button>,
}

#[derive(Clone, Copy, Default)]
enum Phase {
    #[default]
    Hidden,
    Opening,
    Shown(Option<Foreground>),
}

impl Phase {
    fn swipe_can_submit<T: Eq>(
        self,
        original: Option<T>,
        current: Option<T>,
        owns_focus: bool,
    ) -> bool {
        original.is_some()
            && match self {
                Self::Shown(_) => owns_focus,
                Self::Opening => current == original,
                Self::Hidden => false,
            }
    }
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
        matches!(self.phase, Phase::Shown(_)).then_some(self.generation)
    }

    fn hide(&mut self) -> Phase {
        std::mem::take(&mut self.phase)
    }
}

pub struct Controller {
    bindings: RefCell<Bindings>,
    pending: RefCell<Option<Prepared>>,
    editor: Editor,
    hold: RefCell<Option<Hold>>,
    launcher: slint::Weak<AppWindow>,
    business: Weak<crate::business::Controller>,
    window: RefCell<Option<WorkspaceWindow>>,
    state: RefCell<State>,
    native: Cell<Option<NativeWindow>>,
    region: Cell<Option<Region>>,
    radial: RefCell<Option<Rc<Scene>>>,
    resolver: RefCell<Option<Resolver>>,
    shape: Shape,
    observer: Option<Observer>,
    diagnostics: bool,
    force_reduce_motion: bool,
}

impl Controller {
    pub fn connect(
        launcher: &AppWindow,
        business: Weak<crate::business::Controller>,
        preview: Option<Preview>,
        observer: Option<Observer>,
        diagnostics: bool,
        force_reduce_motion: bool,
    ) -> Rc<Self> {
        let controller = Rc::new(Self {
            bindings: RefCell::default(),
            pending: RefCell::new(None),
            editor: Editor::default(),
            hold: RefCell::new(None),
            launcher: launcher.as_weak(),
            business,
            window: RefCell::new(None),
            state: RefCell::default(),
            native: Cell::new(None),
            region: Cell::new(None),
            radial: RefCell::new(None),
            resolver: RefCell::new(None),
            shape: preview
                .as_ref()
                .map_or(Shape::Radial, |preview| preview.shape),
            observer,
            diagnostics,
            force_reduce_motion,
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
            if let Some(controller) = weak.upgrade()
                && controller.shape == Shape::Radial
            {
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
                self.log("workspace_hotkey_ready", &format!("shape={:?}", self.shape));
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

    fn prepare_bindings(&self, config: &WorkspaceConfig) -> Result<Prepared, String> {
        self.bindings
            .borrow()
            .prepare(config, self.launcher.clone())
    }

    fn apply_bindings(&self, prepared: Prepared) {
        let enabled = prepared.enabled;
        self.hide("settings_changed", true);
        let old = self.bindings.borrow_mut().commit(prepared);
        drop(old);
        self.suspend_input(
            self.launcher
                .upgrade()
                .is_some_and(|app| app.get_native_dialog_active() || app.get_hotkey_recording()),
        );
        if !enabled {
            self.window.borrow_mut().take();
            self.native.set(None);
            self.region.set(None);
            self.radial.borrow_mut().take();
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
        if suspended && self.hold.borrow().is_some() {
            self.hide("launcher_modal", false);
        }
    }

    fn save_settings(&self, mut config: WorkspaceConfig) -> Result<(), String> {
        if self.pending.borrow().is_some() {
            return Err("工作台设置正在保存".into());
        }
        config.validate().map_err(|_| "应用配置或槽位数量有误")?;
        config.hotkey = Key::parse(&config.hotkey)?.to_string();
        let prepared = self.prepare_bindings(&config)?;
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
        if self.hold.borrow().is_some() || !matches!(self.state.borrow().phase, Phase::Hidden) {
            self.hide("hotkey", true);
            return;
        }
        let Some(launcher) = self.launcher.upgrade() else {
            return;
        };
        if launcher.get_native_dialog_active() || launcher.get_hotkey_recording() {
            return;
        }
        let Some((layout, _)) = self.invocation_layout(trigger, &launcher) else {
            return;
        };
        if self.shape == Shape::Radial {
            self.request_context(trigger);
        }
        if let Err(error) = self.open(trigger, layout, &launcher) {
            self.hide("open_error", false);
            self.log("workspace_error", &format!("operation=open error={error}"));
        }
    }

    fn invocation_layout(
        &self,
        trigger: Invocation,
        launcher: &AppWindow,
    ) -> Option<(Layout, (i32, i32))> {
        let result = trigger.anchor.and_then(|anchor| {
            anchor
                .layout(self.shape)
                .map(|layout| (layout, anchor.point))
        });
        if result.is_none() {
            self.log("workspace_error", "operation=layout");
            launcher.set_workspace_notice("工作台无法在当前显示器工作区内打开".into());
        }
        result
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
        window.set_reduce_motion(
            self.shape != Shape::Radial
                || self.force_reduce_motion
                || launcher.get_reduce_motion()
                || preferences::reduced_motion(),
        );
        window.set_highlighted(false);
        if self.shape == Shape::Radial {
            window.set_radial(true);
            self.region.set(None);
        }
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
        if self.shape == Shape::Radial && !workspace::current_input_desktop() {
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
            if self.shape == Shape::Radial {
                workspace::prepare_radial_frame(native)?;
            }
            self.native.set(Some(native));
        }
        workspace::apply_layout(native, layout)?;
        let scene = (self.shape == Shape::Radial).then(|| self.prepare_scene(trigger));
        let intro = !view.get_reduce_motion()
            && scene
                .as_ref()
                .is_some_and(|scene| scene.display() == Display::Full);
        if let Some(scene) = &scene {
            if intro {
                scene.set_display(Display::Intro);
            }
            scene.show(view);
            if let Some(hold) = self.hold.borrow().as_ref() {
                scene.highlight(view, hold.highlight());
            }
        }
        self.update_region(native)?;
        if Foreground::current() != trigger.foreground {
            self.hide("foreground_changed", false);
            return Ok(());
        }
        // Record the original target before any show or activation event.
        self.state.borrow_mut().phase = Phase::Shown(trigger.foreground);
        view.show()?;
        native.activate();
        if !native.active() {
            self.hide("activation_failed", false);
            return Ok(());
        }
        self.log(
            "workspace_shown",
            &format!(
                "shape={:?} cold={cold} dpi={} invoke_us={} captured={}",
                self.shape,
                layout.dpi,
                trigger.at.elapsed().as_micros(),
                trigger.foreground.is_some()
            ),
        );
        if let Some(scene) = scene {
            self.log(
                "workspace_radial_display",
                &format!("mode={:?}", scene.display()),
            );
        }
        if intro {
            let weak = Rc::downgrade(self);
            let generation = self.state.borrow().generation;
            // Only the hub is interactive during the opening beat. Install
            // the full region once when its tiles are ready to appear.
            Timer::single_shot(INTRO_DURATION, move || {
                if let Some(controller) = weak.upgrade()
                    && controller.state.borrow().visible_generation() == Some(generation)
                    && let (Some(scene), Some(view)) = (controller.scene(), controller.window())
                {
                    controller.finish_intro(&scene, &view);
                }
            });
        }
        Ok(())
    }

    fn finish_intro(&self, scene: &Scene, view: &WorkspaceWindow) -> bool {
        if scene.display() != Display::Intro {
            return scene.display() == Display::Full;
        }
        let Some(native) = self.native.get().filter(|native| native.active()) else {
            self.hide("blur", false);
            return false;
        };
        scene.set_display(Display::Full);
        scene.render(view);
        self.region.set(None);
        if let Err(error) = self.update_region(native) {
            self.hide("region_error", false);
            self.log(
                "workspace_error",
                &format!("operation=region error={error}"),
            );
            return false;
        }
        self.log("workspace_radial_display", "mode=Full");
        true
    }

    fn update_region(&self, native: NativeWindow) -> io::Result<()> {
        let region = Region::read(native)?;
        if self.region.get() != Some(region) {
            if let Some(scene) = self.scene() {
                region.apply_radial(native, scene.outlines())?;
                if let Some(window) = self.window() {
                    window.set_radial_compact(region.scale() < 0.97);
                }
            } else {
                region.apply(native, self.shape)?;
            }
            self.region.set(Some(region));
        }
        Ok(())
    }

    fn connect_window(self: &Rc<Self>, view: &WorkspaceWindow) {
        let weak = Rc::downgrade(self);
        view.on_dismiss(move || {
            if let Some(controller) = weak.upgrade() {
                controller.hide("button", true);
            }
        });
        let weak = Rc::downgrade(self);
        let weak_view = view.as_weak();
        view.on_toggle_highlight(move || {
            if let Some(view) = weak_view.upgrade() {
                let checked = !view.get_highlighted();
                view.set_highlighted(checked);
                if let Some(controller) = weak.upgrade() {
                    controller.log("workspace_highlight", &format!("checked={checked}"));
                }
            }
        });
        let weak = Rc::downgrade(self);
        view.window().on_close_requested(move || {
            if let Some(controller) = weak.upgrade() {
                controller.hide("close", true);
            }
            CloseRequestResponse::KeepWindowShown
        });
        let weak = Rc::downgrade(self);
        view.on_radial_activate(move |item| {
            if let Some(controller) = weak.upgrade() {
                controller.radial_activate(item);
            }
        });
        let weak = Rc::downgrade(self);
        view.window().on_winit_window_event(move |_, event| {
            let Some(controller) = weak.upgrade() else {
                return EventResult::Propagate;
            };
            if let (Some(scene), Some(region), Some(window)) = (
                controller.scene(),
                controller.region.get(),
                controller.window(),
            ) && controller.state.borrow().visible_generation().is_some()
                && matches!(scene.display(), Display::Full | Display::Intro)
            {
                match event {
                    WindowEvent::CursorMoved { position, .. } => {
                        scene.move_pointer(
                            &window,
                            Some(region.board_point(position.x, position.y)),
                        );
                    }
                    WindowEvent::CursorLeft { .. } => scene.move_pointer(&window, None),
                    WindowEvent::MouseInput {
                        state,
                        button: MouseButton::Left,
                        ..
                    } => {
                        scene.move_pointer(
                            &window,
                            workspace::cursor().map(|point| region.screen_to_board(point)),
                        );
                        if let Some(item) = scene.click(state.is_pressed()) {
                            controller.radial_activate(item);
                        }
                        return EventResult::PreventDefault;
                    }
                    WindowEvent::ModifiersChanged(modifiers) => {
                        scene.reverse_tab.set(modifiers.state().shift_key());
                    }
                    WindowEvent::KeyboardInput { event, .. } if event.state.is_pressed() => {
                        match event.physical_key {
                            PhysicalKey::Code(KeyCode::Tab) => {
                                if controller.finish_intro(&scene, &window) {
                                    scene.tab(&window);
                                }
                                return EventResult::PreventDefault;
                            }
                            PhysicalKey::Code(
                                KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Space,
                            ) => {
                                if !event.repeat {
                                    controller.radial_activate(window.get_radial_focus());
                                }
                                return EventResult::PreventDefault;
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
            match event {
                WindowEvent::KeyboardInput { event, .. }
                    if event.physical_key == PhysicalKey::Code(KeyCode::Escape)
                        && event.state.is_pressed() =>
                {
                    controller.hide("escape", true);
                    return EventResult::PreventDefault;
                }
                WindowEvent::Focused(false) => controller.after_window_event(true),
                WindowEvent::ScaleFactorChanged { .. }
                    if controller.hold.borrow().is_some()
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

    fn radial_activate(&self, item: i32) {
        if self.state.borrow().visible_generation().is_none()
            || !self.scene().is_some_and(|scene| {
                scene.display() == Display::Full
                    || (scene.display() == Display::Intro && item == radial::HUB)
            })
        {
            return;
        }
        if item == radial::HUB {
            self.hide("hub", true);
        } else if item == radial::EDIT
            && let Some(scene) = self.scene()
            && scene.can_edit()
            && let Some(app) = self.launcher.upgrade()
        {
            self.open_editor(
                scene
                    .application
                    .as_ref()
                    .map(|application| &application.key),
            );
            self.hide("edit", false);
            app.set_settings_tab(7);
            app.invoke_desktop_event(
                velo_slint::platform::windows::desktop::Event::Settings as i32,
            );
        } else {
            self.dispatch_radial(item);
        }
    }

    fn dispatch_radial(&self, item: i32) {
        let original = match self.state.borrow().phase {
            Phase::Shown(original) if self.native.get().is_some_and(NativeWindow::active) => {
                original
            }
            _ => return,
        };
        self.run_radial_action(item, original);
    }

    fn run_radial_action(&self, item: i32, original: Option<Foreground>) {
        if !workspace::current_input_desktop() {
            self.hide("input_desktop_changed", false);
            return;
        }
        if original.is_some_and(|target| !target.exists()) {
            self.hide("target_invalid", false);
            return;
        }
        let Some(scene) = self.scene() else { return };
        let Some(action) = scene.action(item) else {
            return;
        };
        let Some(launcher) = self.launcher.upgrade() else {
            return;
        };
        if !scene.item_available(item) {
            launcher.set_workspace_notice("请在工作台设置中为该应用选择匹配的动作".into());
            if let Some(window) = self.window() {
                window.set_radial_scope("当前应用不可用".into());
            }
            return;
        }
        self.log(
            "workspace_radial_selected",
            &format!("slot={item} action={action:?}"),
        );
        if action == WorkspaceAction::Launcher {
            self.hide("radial_tile", false);
            launcher
                .invoke_desktop_event(velo_slint::platform::windows::desktop::Event::Open as i32);
            self.log("workspace_action_dispatched", "action=Launcher");
            return;
        }
        let target = original.zip(scene.application.clone());
        let owner = native_window_from_app(&launcher).map_or(0, |window| window.hwnd() as isize);
        self.hide("radial_tile", true);
        if self
            .business
            .upgrade()
            .is_some_and(|business| business.workspace_action(action, target, owner))
        {
            launcher.set_workspace_notice("正在执行工作台动作".into());
            self.log("workspace_action_dispatched", &format!("action={action:?}"));
        } else {
            launcher.set_workspace_notice("工作台动作暂时无法提交，请稍后重试".into());
        }
    }

    fn scene(&self) -> Option<Rc<Scene>> {
        self.radial.borrow().clone()
    }

    fn request_context(&self, trigger: Invocation) {
        self.radial.borrow_mut().take();
        if trigger.foreground.is_none() {
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

    fn prepare_scene(&self, trigger: Invocation) -> Rc<Scene> {
        if let Some(view) = self.scene() {
            return view;
        }
        let application = self
            .resolver
            .borrow()
            .as_ref()
            .and_then(|resolver| resolver.recent(trigger.foreground));
        let config = self.committed_config().unwrap_or_default();
        let layouts = Layouts::resolve(&config, application.as_ref().map(|app| &app.key));
        let view = Rc::new(Scene::new(layouts, application, true));
        self.radial.replace(Some(view.clone()));
        view
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
        self.hold.borrow_mut().take();
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

    #[test]
    fn swipe_release_needs_the_original_target_or_an_active_workspace() {
        assert!(!Phase::Opening.swipe_can_submit::<u32>(None, None, false));
        assert!(!Phase::Opening.swipe_can_submit(Some(1), Some(2), false));
        assert!(Phase::Opening.swipe_can_submit(Some(1), Some(1), false));
        assert!(Phase::Shown(None).swipe_can_submit(Some(1), Some(2), true));
        assert!(!Phase::Shown(None).swipe_can_submit(Some(1), Some(2), false));
        assert!(!Phase::Hidden.swipe_can_submit(Some(1), Some(1), true));
    }
}
