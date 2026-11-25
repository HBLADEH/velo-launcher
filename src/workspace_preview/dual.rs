use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::time::Instant;

use slint::{ModelRc, Timer, VecModel};
use velo_slint::core::service::WorkspaceUserRequest;
use velo_slint::core::workspace::{
    actions::{Template, WorkspaceAction},
    config::AppKey,
    directions::{self, Direction, Release},
    dual::{DualConfig, Matrix},
    user_actions::UserAction,
};
use velo_slint::platform::windows::{
    workspace::{self, Foreground, Invocation},
    workspace_actions::Application,
    workspace_input::Sample,
};

use crate::{WorkspaceTile, WorkspaceWindow};

use super::{Controller, Phase};

pub(super) const PANEL_HEIGHT: i32 = 348;
const PANEL_HUB: i32 = 172;
const PAD_WIDTH: i32 = 278;
const PAD_HEIGHT: i32 = 218;

#[derive(Default)]
pub(super) struct Dual {
    press: RefCell<Option<Press>>,
    pub scene: RefCell<Option<Rc<Scene>>>,
}

impl Dual {
    pub fn pending(&self) -> bool {
        self.press.borrow().is_some()
    }

    pub fn clear(&self) {
        self.press.borrow_mut().take();
        self.scene.borrow_mut().take();
    }
}

struct Press {
    trigger: Invocation,
    close_panel: bool,
    phase: directions::Phase,
    latest: Sample,
}

impl Press {
    fn motion(&self, sample: Sample) -> (std::time::Duration, f64, f64, f64) {
        let anchor = self
            .trigger
            .anchor
            .expect("sessions have a captured anchor");
        let scale = 96.0 / f64::from(anchor.dpi());
        (
            sample.at.saturating_duration_since(self.trigger.at),
            sample.distance * scale,
            (f64::from(sample.point.0) - f64::from(anchor.point.0)) * scale,
            (f64::from(sample.point.1) - f64::from(anchor.point.1)) * scale,
        )
    }
}

#[derive(Clone)]
struct Target {
    window: Option<Foreground>,
    application: Option<Application>,
    focus: Option<Foreground>,
}

impl Target {
    fn verified(&self) -> Option<(Foreground, Application)> {
        self.window.zip(self.application.clone())
    }

    fn name(&self) -> Option<String> {
        self.application
            .as_ref()
            .map(|app| super::editor::key_name(&app.key))
    }

    fn key(&self) -> Option<&AppKey> {
        self.application.as_ref().map(|app| &app.key)
    }
}

enum Action {
    Builtin(WorkspaceAction),
    User(Box<UserAction>),
}

struct Slot {
    action: Option<Action>,
    tile: WorkspaceTile,
}

impl Slot {
    fn new(
        config: &DualConfig,
        id: Option<&str>,
        direction: Option<Direction>,
        target: &Target,
        template: Option<Template>,
    ) -> Self {
        let (action, label, glyph, enabled) = if let Some(action) = id.and_then(DualConfig::builtin)
        {
            let enabled = action
                .template()
                .is_none_or(|required| template == Some(required) && target.verified().is_some());
            (
                Some(Action::Builtin(action)),
                action.label(),
                action.glyph(),
                enabled,
            )
        } else if let Some(action) = id.and_then(|id| config.actions.get(id)) {
            let enabled = action.action.ready()
                && (!action.action.requires_target() || target.verified().is_some());
            (
                Some(Action::User(Box::new(action.clone()))),
                action.label.as_str(),
                action.glyph(),
                enabled,
            )
        } else {
            (
                None,
                if id.is_some() { "动作不可用" } else { "" },
                "apps",
                false,
            )
        };
        Self {
            action,
            tile: WorkspaceTile {
                label: label.into(),
                glyph: glyph.into(),
                direction: direction.map_or("", Direction::icon_path).into(),
                enabled,
            },
        }
    }

    fn action(&self) -> Option<&Action> {
        self.action.as_ref().filter(|_| self.tile.enabled)
    }
}

enum View {
    Panel {
        global: Vec<Slot>,
        global_columns: usize,
        local_columns: usize,
    },
    Pad(Cell<directions::Phase>),
}

pub(super) struct Scene {
    view: View,
    slots: Vec<Slot>,
    target: Target,
    foreground: Target,
    title: String,
    glyph: &'static str,
    matched: bool,
    ready: Instant,
}

impl Scene {
    fn panel(dual: &DualConfig, target: Target, foreground: Target, ready: Instant) -> Self {
        let profile = dual.panel(target.key());
        let matrix = profile.map_or(&dual.default_panel, |profile| &profile.matrix);
        let template = profile.and_then(|profile| profile.template);
        let tiles = |matrix: &Matrix, target: &Target, template| {
            matrix
                .visible()
                .iter()
                .map(|slot| Slot::new(dual, slot.as_deref(), None, target, template))
                .collect()
        };
        Self {
            view: View::Panel {
                global: tiles(&dual.global, &foreground, None),
                global_columns: dual.global.columns,
                local_columns: matrix.columns,
            },
            slots: tiles(matrix, &target, template),
            title: profile
                .map(|profile| profile.name.clone())
                .or_else(|| target.name())
                .unwrap_or_else(|| "默认工作台".into()),
            glyph: template.map_or("apps", Template::glyph),
            matched: profile.is_some(),
            target,
            foreground,
            ready,
        }
    }

    fn pad(
        dual: &DualConfig,
        foreground: Target,
        phase: directions::Phase,
        ready: Instant,
    ) -> Self {
        let profile = dual.gestures(foreground.key());
        let directions = profile.map_or(&dual.default_gestures, |profile| &profile.directions);
        let template = profile.and_then(|profile| profile.template);
        Self {
            view: View::Pad(Cell::new(phase)),
            slots: Direction::GRID
                .into_iter()
                .map(|direction| {
                    Slot::new(
                        dual,
                        directions.get(direction),
                        Some(direction),
                        &foreground,
                        template,
                    )
                })
                .collect(),
            title: profile.map_or_else(|| "默认手势".into(), |profile| profile.name.clone()),
            glyph: template.map_or("apps", Template::glyph),
            matched: profile.is_some(),
            target: foreground.clone(),
            foreground,
            ready,
        }
    }

    pub fn is_pad(&self) -> bool {
        matches!(self.view, View::Pad(_))
    }

    pub fn center_only(&self) -> bool {
        matches!(&self.view, View::Pad(phase) if phase.get() == directions::Phase::Center)
    }

    fn size(&self) -> (i32, i32, i32) {
        match self.view {
            View::Panel {
                global_columns,
                local_columns,
                ..
            } => {
                let columns = global_columns.max(local_columns) as i32;
                (
                    24 + columns * 88 + (columns - 1) * 6,
                    PANEL_HEIGHT,
                    PANEL_HUB,
                )
            }
            View::Pad(_) => (PAD_WIDTH, PAD_HEIGHT, PAD_HEIGHT / 2),
        }
    }

    pub fn show(&self, window: &WorkspaceWindow) {
        let tiles = |slots: &[Slot]| {
            ModelRc::new(VecModel::from(
                slots
                    .iter()
                    .map(|slot| slot.tile.clone())
                    .collect::<Vec<_>>(),
            ))
        };
        let (width, height, _) = self.size();
        window.set_board_width(width);
        window.set_board_height(height);
        window.set_pad(self.is_pad());
        window.set_context_title(self.title.as_str().into());
        window.set_context_glyph(self.glyph.into());
        window.set_context_tiles(tiles(&self.slots));
        if let View::Panel {
            global,
            global_columns,
            local_columns,
        } = &self.view
        {
            window.set_global_tiles(tiles(global));
            window.set_global_columns(*global_columns as i32);
            window.set_context_columns(*local_columns as i32);
            let source = if self.matched {
                "应用专属"
            } else if self.target.application.is_none() {
                ""
            } else {
                "默认工作台"
            };
            window.set_context_source(source.into());
        } else {
            self.update_pad(window);
        }
    }

    fn selected(&self) -> Option<usize> {
        let View::Pad(phase) = &self.view else {
            return None;
        };
        let directions::Phase::Selecting(direction) = phase.get() else {
            return None;
        };
        let index = direction_index(direction);
        self.slots[index].tile.enabled.then_some(index)
    }

    fn update_pad(&self, window: &WorkspaceWindow) {
        window.set_pad_expanded(!self.center_only());
        window.set_pad_selection(self.selected().map_or(-1, |index| index as i32));
    }

    pub fn report_shown(&self, controller: &Controller) {
        let mode = if self.center_only() {
            "center"
        } else if self.is_pad() {
            "pad"
        } else {
            "panel"
        };
        controller.log(
            "workspace_dual_shown",
            &format!(
                "mode={mode} matched={} input_us={}",
                self.matched,
                self.ready.elapsed().as_micros()
            ),
        );
    }
}

fn direction_index(direction: Direction) -> usize {
    Direction::GRID
        .iter()
        .position(|value| *value == direction)
        .unwrap_or_default()
}

impl Controller {
    fn dual_target(&self, window: Option<Foreground>, focus: Option<Foreground>) -> Target {
        let window = window.filter(|window| window.external_application());
        let application = self
            .resolver
            .borrow()
            .as_ref()
            .and_then(|resolver| resolver.recent(window));
        Target {
            window,
            application,
            focus,
        }
    }

    fn dual_scene(
        &self,
        trigger: Invocation,
        phase: Option<directions::Phase>,
        at: Instant,
    ) -> Scene {
        let config = self.committed_config().unwrap_or_default();
        let dual = &config.dual;
        let foreground = self.dual_target(trigger.foreground, trigger.focus);
        if let Some(phase) = phase {
            Scene::pad(dual, foreground, phase, at)
        } else {
            let focus = trigger
                .focus
                .filter(|_| trigger.hovered == trigger.foreground);
            Scene::panel(
                dual,
                self.dual_target(trigger.hovered, focus),
                foreground,
                at,
            )
        }
    }

    pub(super) fn open_dual_panel(self: &Rc<Self>, trigger: Invocation, at: Instant) {
        self.open_dual(trigger, self.dual_scene(trigger, None, at));
    }

    fn open_dual(self: &Rc<Self>, trigger: Invocation, scene: Scene) {
        let Some(app) = self.launcher.upgrade() else {
            return;
        };
        app.set_workspace_action_ticket(app.get_workspace_action_ticket().wrapping_add(1));
        let (width, height, center_y) = scene.size();
        let Some(layout) = trigger
            .anchor
            .and_then(|anchor| anchor.layout(width, height, center_y))
        else {
            app.set_workspace_notice("当前显示器工作区放不下工作台".into());
            self.hide("layout", true);
            return;
        };
        self.dual.scene.replace(Some(Rc::new(scene)));
        if let Err(error) = self.open(trigger, layout, &app) {
            self.hide("open_error", false);
            self.log("workspace_error", &format!("operation=open error={error}"));
        }
    }

    pub(super) fn dual_press(self: &Rc<Self>, mut trigger: Invocation) {
        let close_panel = matches!(self.state.borrow().phase, Phase::Shown(_));
        self.hide("side_retrigger", true);
        if close_panel {
            trigger = trigger.after_panel_hides();
        }
        let Some(app) = self.launcher.upgrade() else {
            return;
        };
        if app.get_native_dialog_active() || app.get_hotkey_recording() {
            return;
        }
        let Some(anchor) = trigger.anchor else { return };
        self.request_context(trigger);
        self.dual.press.replace(Some(Press {
            trigger,
            close_panel,
            phase: directions::Phase::Pending,
            latest: Sample {
                at: trigger.at,
                point: anchor.point,
                distance: 0.0,
            },
        }));
        self.log("workspace_side_down", "");
        let weak = Rc::downgrade(self);
        Timer::single_shot(
            directions::HOLD_AFTER.saturating_sub(trigger.at.elapsed()),
            move || {
                if let Some(controller) = weak.upgrade() {
                    // The release can already be in the input queue when this wakes.
                    controller.drain_mouse();
                    let sample = controller
                        .dual
                        .press
                        .borrow()
                        .as_ref()
                        .filter(|press| press.trigger.at == trigger.at)
                        .map(|press| Sample {
                            at: press.latest.at.max(trigger.at + directions::HOLD_AFTER),
                            ..press.latest
                        });
                    if let Some(sample) = sample {
                        controller.dual_move(sample);
                    }
                }
            },
        );
    }

    pub(super) fn dual_move(self: &Rc<Self>, sample: Sample) {
        let (trigger, previous, phase) = {
            let mut press = self.dual.press.borrow_mut();
            let Some(press) = press.as_mut() else { return };
            let (age, peak, x, y) = press.motion(sample);
            press.latest = Sample {
                at: sample.at.max(press.latest.at),
                ..sample
            };
            let previous = press.phase;
            press.phase = previous.advance(age, peak, x, y);
            (press.trigger, previous, press.phase)
        };
        if phase == directions::Phase::Pending || phase == previous {
            return;
        }
        if Foreground::current() != trigger.foreground {
            self.hide("foreground_changed", false);
            return;
        }
        let scene = self.dual.scene.borrow().clone();
        let Some(scene) = scene else {
            let scene = self.dual_scene(trigger, Some(phase), sample.at);
            self.log(
                "workspace_dual_selection",
                &format!(
                    "phase={phase:?} effective={:?} input_us={}",
                    scene.selected(),
                    sample.at.elapsed().as_micros()
                ),
            );
            self.open_dual(trigger, scene);
            return;
        };
        let View::Pad(current) = &scene.view else {
            return;
        };
        let compact = scene.center_only();
        current.set(phase);
        if let Some(window) = self.window() {
            scene.update_pad(&window);
        }
        self.log(
            "workspace_dual_selection",
            &format!(
                "phase={phase:?} effective={:?} input_us={}",
                scene.selected(),
                sample.at.elapsed().as_micros()
            ),
        );
        if compact != scene.center_only()
            && self.state.borrow().visible_generation().is_some()
            && let Some(native) = self.native.get()
        {
            self.region.set(None);
            if let Err(error) = self.update_region(native) {
                self.hide("region_error", false);
                self.log(
                    "workspace_error",
                    &format!("operation=region error={error}"),
                );
            }
        }
    }

    pub(super) fn dual_release(self: &Rc<Self>, sample: Sample) {
        let Some(press) = self.dual.press.borrow_mut().take() else {
            return;
        };
        let trigger = press.trigger;
        if Foreground::current() != trigger.foreground || !trigger.display_unchanged() {
            self.hide("context_changed", false);
            return;
        }
        let (age, peak, x, y) = press.motion(sample);
        let result = press.phase.release(age, peak, x, y);
        self.log(
            "workspace_side_release",
            &format!(
                "result={result:?} input_us={}",
                sample.at.elapsed().as_micros()
            ),
        );
        match result {
            Release::Panel if !press.close_panel => self.open_dual_panel(trigger, sample.at),
            Release::Direction(direction) => {
                let scene = self.dual.scene.borrow().clone();
                if press.phase == directions::Phase::Selecting(direction)
                    && let Some(scene) = scene
                    && let Some(action) = scene.slots[direction_index(direction)].action()
                {
                    self.dispatch_dual(action, &scene.target, false, sample.at);
                } else {
                    self.hide("empty_direction", false);
                }
            }
            _ => self.hide("side_release_cancel", false),
        }
    }

    pub(super) fn dual_activate(&self, index: i32) {
        if !matches!(self.state.borrow().phase, Phase::Shown(_))
            || !self.native.get().is_some_and(|native| native.active())
        {
            return;
        }
        let Some(scene) = self.dual.scene.borrow().clone() else {
            return;
        };
        let View::Panel { global, .. } = &scene.view else {
            return;
        };
        match index {
            -1 => self.dispatch_dual(
                &Action::Builtin(WorkspaceAction::Launcher),
                &scene.foreground,
                true,
                Instant::now(),
            ),
            -4..=-2 => self.open_dual_settings(index, &scene),
            -5 => self.hide("panel_close", true),
            0..=9 => {
                if let Some(action) = global.get(index as usize).and_then(Slot::action) {
                    self.dispatch_dual(action, &scene.foreground, true, Instant::now());
                }
            }
            10..=19 => {
                if let Some(action) = scene
                    .slots
                    .get((index - 10) as usize)
                    .and_then(Slot::action)
                {
                    self.dispatch_dual(action, &scene.target, true, Instant::now());
                }
            }
            _ => {}
        }
    }

    fn dispatch_dual(&self, action: &Action, target: &Target, panel: bool, at: Instant) {
        let Some(app) = self.launcher.upgrade() else {
            return;
        };
        if !workspace::current_input_desktop() {
            self.hide("input_desktop_changed", false);
            return;
        }
        if matches!(action, Action::Builtin(WorkspaceAction::Launcher)) {
            self.hide("dual_action", false);
            app.invoke_desktop_event(velo_slint::platform::windows::desktop::Event::Open as i32);
            self.log(
                "workspace_action_dispatched",
                &format!("action=Launcher input_us={}", at.elapsed().as_micros()),
            );
            return;
        }
        let verified = target.verified();
        let needs_target = match action {
            Action::Builtin(action) => action.template().is_some(),
            Action::User(action) => action.action.requires_target(),
        };
        if needs_target
            && !verified.as_ref().is_some_and(|(window, _)| {
                if panel {
                    self.native
                        .get()
                        .is_some_and(|native| window.restore(native))
                } else {
                    Foreground::current() == Some(*window)
                }
            })
        {
            self.hide("target_invalid", panel);
            app.set_workspace_notice("无法切回动作所属的应用，已取消工作台动作".into());
            return;
        }
        let owner = super::native_window_from_app(&app).map_or(0, |window| window.hwnd() as isize);
        self.hide("dual_action", panel && !needs_target);
        let submitted = self
            .business
            .upgrade()
            .is_some_and(|business| match action {
                Action::Builtin(action) => business.workspace_action(*action, verified, owner),
                Action::User(action) => self.submit_user_action(
                    &app,
                    &business,
                    action,
                    verified.map(|(window, application)| (window, application, target.focus)),
                    owner,
                ),
            });
        if submitted {
            app.set_workspace_notice("正在执行工作台动作".into());
            let kind = match action {
                Action::Builtin(action) => action.id(),
                Action::User(action) => action.action.kind(),
            };
            self.log(
                "workspace_action_dispatched",
                &format!("action={kind} input_us={}", at.elapsed().as_micros()),
            );
        } else {
            app.set_workspace_notice("工作台动作暂时无法提交，请稍后重试".into());
        }
    }

    pub(super) fn submit_user_action(
        &self,
        app: &crate::AppWindow,
        business: &crate::business::Controller,
        action: &UserAction,
        target: Option<(Foreground, Application, Option<Foreground>)>,
        owner: isize,
    ) -> bool {
        let ticket = app.get_workspace_action_ticket().wrapping_add(1);
        app.set_workspace_action_ticket(ticket);
        business.workspace_user_action(WorkspaceUserRequest {
            action: action.clone(),
            target,
            owner,
            generation: self.execution_epoch.load(Ordering::Acquire),
            epoch: self.execution_epoch.clone(),
            ticket,
        })
    }

    fn open_dual_settings(&self, index: i32, scene: &Scene) {
        if let Some(app) = self.launcher.upgrade() {
            let (section, key) = match index {
                -4 => (super::editor::GESTURE, scene.foreground.key()),
                -3 => (super::editor::PANEL, scene.target.key()),
                _ => (super::editor::PANEL, None),
            };
            self.editor_at(&app, section, key, index == -3);
            self.editor.set_test_target(
                scene
                    .foreground
                    .verified()
                    .map(|(window, app)| (window, app, scene.foreground.focus)),
            );
            self.hide("dual_edit", false);
            app.set_settings_tab(7);
            app.invoke_desktop_event(
                velo_slint::platform::windows::desktop::Event::Settings as i32,
            );
        }
    }
}
