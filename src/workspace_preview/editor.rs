use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;

use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use velo_slint::core::workspace::{
    actions::{Template, WorkspaceAction},
    config::{AppKey, Layout, MouseButton, Profile, WorkspaceConfig},
    radial::{Board, Point, Zone},
};

use crate::{AppWindow, RadialTile, WorkspaceChoice, WorkspaceDraft, WorkspaceEditorState};

use super::Controller;

#[derive(Default)]
pub(super) struct Editor {
    draft: RefCell<Option<WorkspaceConfig>>,
    scope: Cell<i32>,
    board: RefCell<Option<Board>>,
    updating: Cell<bool>,
}

impl Controller {
    pub(super) fn connect_editor(self: &Rc<Self>, app: &AppWindow) {
        let model = app.global::<WorkspaceEditorState>();
        let weak = Rc::downgrade(self);
        model.on_open(move || {
            if let Some(controller) = weak.upgrade() {
                controller.open_editor(None);
            }
        });
        let weak = Rc::downgrade(self);
        model.on_close(move || {
            if let Some(controller) = weak.upgrade() {
                controller.editor.draft.borrow_mut().take();
                controller.editor.board.borrow_mut().take();
                if let Some(app) = controller.launcher.upgrade() {
                    let model = app.global::<WorkspaceEditorState>();
                    model.set_draft(WorkspaceDraft::default());
                    model.set_scopes(ModelRc::default());
                    model.set_slots(ModelRc::default());
                    model.set_tiles(ModelRc::default());
                    model.set_choices(ModelRc::default());
                }
            }
        });
        let weak = Rc::downgrade(self);
        model.on_save(move || {
            if let Some(controller) = weak.upgrade() {
                controller.save_editor();
            }
        });
        let weak = Rc::downgrade(self);
        model.on_changed_selection(move || {
            if let Some(controller) = weak.upgrade() {
                controller.editor_selection();
            }
        });
        let weak = Rc::downgrade(self);
        model.on_identity_changed(move || {
            if let Some(controller) = weak.upgrade() {
                controller.editor_exclusion();
            }
        });
        let weak = Rc::downgrade(self);
        model.on_command(move |command, arg| {
            if let Some(controller) = weak.upgrade() {
                controller.editor_command(&command, arg);
            }
        });
        let weak = Rc::downgrade(self);
        model.on_select_point(move |x, y| {
            if let Some(controller) = weak.upgrade()
                && let Some(index) = controller.editor.hit(Point { x, y })
                && let Some(app) = controller.launcher.upgrade()
            {
                app.global::<WorkspaceEditorState>().set_slot(index);
            }
        });
        let weak = Rc::downgrade(self);
        model.on_drop_action(move |id, x, y| {
            if let Some(controller) = weak.upgrade()
                && let Some(index) = controller.editor.hit(Point { x, y })
                && let Some(app) = controller.launcher.upgrade()
            {
                app.global::<WorkspaceEditorState>().set_slot(index);
                controller.editor_command("assign", id);
            }
        });
    }

    pub(super) fn open_editor(&self, key: Option<&AppKey>) {
        let Some(app) = self.launcher.upgrade() else {
            return;
        };
        let model = app.global::<WorkspaceEditorState>();
        if self.editor.draft.borrow().is_none() {
            let Some(config) = self.committed_config() else {
                app.set_workspace_notice("工作台配置不可读，原文件已保留".into());
                return;
            };
            self.editor.draft.replace(Some((*config).clone()));
            self.editor.updating.set(true);
            model.set_draft(WorkspaceDraft {
                enabled: config.enabled,
                hotkey: config.hotkey.as_str().into(),
                mouse: match config.mouse {
                    None => 0,
                    Some(MouseButton::XButton2) => 1,
                    Some(MouseButton::XButton1) => 2,
                },
                ..WorkspaceDraft::default()
            });
            self.editor.updating.set(false);
            self.editor.select_scope(&app, 0);
        }
        if let Some(key) = key {
            let scope = self
                .editor
                .draft
                .borrow()
                .as_ref()
                .and_then(|config| {
                    config
                        .profiles
                        .iter()
                        .position(|profile| &profile.app == key)
                })
                .map_or(1, |index| index as i32 + 2);
            self.editor.select_scope(&app, scope);
            if scope == 1 {
                let mut draft = model.get_draft();
                draft.application = key_text(key).into();
                model.set_draft(draft);
            }
        }
        self.editor.refresh(&app);
    }

    fn save_editor(&self) {
        let Some(app) = self.launcher.upgrade() else {
            return;
        };
        let model = app.global::<WorkspaceEditorState>();
        let Some(mut config) = self.editor.draft.borrow().clone() else {
            return;
        };
        let fields = model.get_draft();
        config.enabled = fields.enabled;
        config.hotkey = fields.hotkey.to_string();
        config.mouse = match fields.mouse {
            1 => Some(MouseButton::XButton2),
            2 => Some(MouseButton::XButton1),
            _ => None,
        };
        match self.save_settings(config) {
            Ok(()) => {
                model.set_busy(true);
                app.set_workspace_notice("正在保存工作台设置".into());
                self.log("workspace_settings_submitted", "");
            }
            Err(message) => app.set_workspace_notice(message.into()),
        }
    }

    pub(super) fn editor_saved(&self, saved: bool) {
        self.log("workspace_settings_finished", &format!("saved={saved}"));
        if let Some(app) = self.launcher.upgrade() {
            app.global::<WorkspaceEditorState>().set_busy(false);
            if saved && self.editor.draft.borrow().is_some() {
                self.editor.draft.borrow_mut().take();
                self.open_editor(None);
            }
        }
    }

    fn editor_selection(&self) {
        if self.editor.updating.get() {
            return;
        }
        let Some(app) = self.launcher.upgrade() else {
            return;
        };
        let model = app.global::<WorkspaceEditorState>();
        let scope = model.get_scope();
        if scope != self.editor.scope.get() {
            self.editor.select_scope(&app, scope);
        } else if let Some(config) = self.editor.draft.borrow_mut().as_mut()
            && let Some(layout) = layout_mut(config, scope)
            && let Some(&count) = [4, 6, 8].get(model.get_preset() as usize)
        {
            layout.count = count;
        }
        self.editor.refresh(&app);
    }

    fn editor_exclusion(&self) {
        let Some(app) = self.launcher.upgrade() else {
            return;
        };
        let model = app.global::<WorkspaceEditorState>();
        let key = app_key(&model.get_draft().application);
        let excluded = key.is_some_and(|key| {
            self.editor
                .draft
                .borrow()
                .as_ref()
                .is_some_and(|config| config.excluded.contains(&key))
        });
        model.set_excluded(excluded);
    }

    fn editor_command(&self, command: &str, arg: i32) {
        let Some(app) = self.launcher.upgrade() else {
            return;
        };
        let model = app.global::<WorkspaceEditorState>();
        if model.get_busy() {
            return;
        }
        let scope = self.editor.scope.get();
        let selected = model.get_slot() as usize;
        let mut next_scope = scope;
        {
            let mut draft = self.editor.draft.borrow_mut();
            let Some(config) = draft.as_mut() else { return };
            match command {
                "profile" | "exclude" => {
                    let fields = model.get_draft();
                    let Some(key) = app_key(&fields.application) else {
                        app.set_workspace_notice("请输入应用的完整可执行文件路径或包标识".into());
                        return;
                    };
                    if command == "exclude" {
                        if config.excluded.contains(&key) {
                            config.excluded.retain(|excluded| excluded != &key);
                        } else {
                            config.excluded.push(key);
                        }
                    } else if let Some(index) = config
                        .profiles
                        .iter()
                        .position(|profile| profile.app == key)
                    {
                        next_scope = index as i32 + 2;
                    } else {
                        let template = fields
                            .template
                            .checked_sub(1)
                            .and_then(|index| Template::ALL.get(index as usize).copied());
                        let name = template
                            .map_or_else(|| key_name(&key), |template| template.name().into());
                        config.profiles.push(Profile {
                            app: key,
                            name,
                            glyph: template.map_or("apps", Template::glyph).into(),
                            template,
                            layout: template
                                .map_or_else(|| config.default.clone(), Layout::template),
                            unknown: Default::default(),
                        });
                        next_scope = config.profiles.len() as i32 + 1;
                    }
                }
                "remove" if scope >= 2 => {
                    if (scope - 2) as usize >= config.profiles.len() {
                        return;
                    }
                    config.profiles.remove((scope - 2) as usize);
                    next_scope = 1;
                }
                "reset" => {
                    let restored = match scope {
                        0 => Layout::upper(),
                        1 => Layout::default(),
                        _ => config
                            .profiles
                            .get((scope - 2) as usize)
                            .and_then(|profile| profile.template)
                            .map_or_else(|| config.default.clone(), Layout::template),
                    };
                    if let Some(layout) = layout_mut(config, scope) {
                        let unknown = std::mem::take(&mut layout.unknown);
                        *layout = Layout {
                            unknown,
                            ..restored
                        };
                    }
                }
                "assign" | "clear" | "move" => {
                    let template = scope
                        .checked_sub(2)
                        .and_then(|i| config.profiles.get(i as usize))
                        .and_then(|profile| profile.template);
                    let Some(layout) = layout_mut(config, scope) else {
                        return;
                    };
                    if selected >= usize::from(layout.count) {
                        return;
                    }
                    match command {
                        "assign" => {
                            let Some(&action) = WorkspaceAction::ALL.get(arg as usize) else {
                                return;
                            };
                            if action
                                .template()
                                .is_some_and(|required| Some(required) != template)
                            {
                                return;
                            }
                            layout.slots[selected] = Some(action);
                        }
                        "clear" => layout.slots[selected] = None,
                        _ if (0..i32::from(layout.count)).contains(&arg) => {
                            layout.slots.swap(selected, arg as usize);
                            model.set_slot(arg);
                        }
                        _ => return,
                    }
                }
                _ => return,
            }
        }
        app.set_workspace_notice("".into());
        if next_scope != scope || command == "reset" {
            self.editor.select_scope(&app, next_scope);
        }
        self.editor.refresh(&app);
        self.editor_exclusion();
    }
}

impl Editor {
    fn hit(&self, point: Point) -> Option<i32> {
        let board = self.board.borrow();
        let zone = if self.scope.get() == 0 {
            Zone::Upper
        } else {
            Zone::Lower
        };
        board
            .as_ref()?
            .slots
            .iter()
            .find(|slot| slot.id.zone == zone && slot.outline.contains(point))
            .map(|slot| i32::from(slot.id.index))
    }

    fn select_scope(&self, app: &AppWindow, scope: i32) {
        let model = app.global::<WorkspaceEditorState>();
        let draft = self.draft.borrow();
        let Some(config) = draft.as_ref() else { return };
        let Some(layout) = layout(config, scope) else {
            return;
        };
        self.updating.set(true);
        self.scope.set(scope);
        model.set_scope(scope);
        model.set_preset(i32::from(layout.count / 2 - 2));
        model.set_slot(0);
        if let Some(profile) = scope
            .checked_sub(2)
            .and_then(|index| config.profiles.get(index as usize))
        {
            let mut fields = model.get_draft();
            fields.application = key_text(&profile.app).into();
            fields.template = profile
                .template
                .and_then(|template| Template::ALL.iter().position(|&item| item == template))
                .map_or(0, |index| index as i32 + 1);
            model.set_draft(fields);
        }
        self.updating.set(false);
    }

    fn refresh(&self, app: &AppWindow) {
        let model = app.global::<WorkspaceEditorState>();
        let draft = self.draft.borrow();
        let Some(config) = draft.as_ref() else { return };
        let scope = self.scope.get();
        let profile = scope
            .checked_sub(2)
            .and_then(|index| config.profiles.get(index as usize));
        let lower = profile.map_or(&config.default, |profile| &profile.layout);
        let zone = if scope == 0 { Zone::Upper } else { Zone::Lower };
        let active = if zone == Zone::Upper {
            &config.upper
        } else {
            lower
        };
        let board = Board::new(config.upper.count, lower.count);
        self.updating.set(true);
        model.set_scopes(rows(
            ["全局动作".into(), "默认工作台".into()].into_iter().chain(
                config
                    .profiles
                    .iter()
                    .map(|profile| SharedString::from(profile.name.as_str())),
            ),
        ));
        model.set_slots(rows((0..active.count).map(|index| {
            let label = active.action(index).map_or("空槽", WorkspaceAction::label);
            SharedString::from(format!("{} · {label}", index + 1))
        })));
        if model.get_slot() >= i32::from(active.count) {
            model.set_slot(0);
        }
        model.set_tiles(rows(board.slots.iter().filter_map(|slot| {
            let current = slot.id.zone == zone;
            let source = if slot.id.zone == Zone::Upper {
                &config.upper
            } else {
                lower
            };
            let action = source.action(slot.id.index);
            if !current && action.is_none() {
                return None;
            }
            Some(RadialTile {
                slot: if current {
                    i32::from(slot.id.index)
                } else {
                    -1
                },
                path: slot.outline.commands().into(),
                label: action.map_or("添加动作", WorkspaceAction::label).into(),
                glyph: action.map_or("plus", WorkspaceAction::glyph).into(),
                guide: if action.is_none() {
                    slot.outline.dashed_commands().into()
                } else {
                    "".into()
                },
                center_x: slot.anchor.x,
                center_y: slot.anchor.y,
                enabled: action.is_some(),
                ..Default::default()
            })
        })));
        model.set_choices(rows(
            WorkspaceAction::ALL
                .iter()
                .copied()
                .enumerate()
                .filter(|(_, action)| {
                    action.template().is_none_or(|template| {
                        profile.is_some_and(|profile| profile.template == Some(template))
                    })
                })
                .map(|(index, action)| WorkspaceChoice {
                    id: index as i32,
                    label: action.label().into(),
                    glyph: action.glyph().into(),
                }),
        ));
        self.board.replace(Some(board));
        self.updating.set(false);
    }
}

fn rows<T: Clone + 'static>(values: impl IntoIterator<Item = T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(values.into_iter().collect::<Vec<_>>()))
}

fn layout(config: &WorkspaceConfig, scope: i32) -> Option<&Layout> {
    match scope {
        0 => Some(&config.upper),
        1 => Some(&config.default),
        _ => config
            .profiles
            .get((scope - 2) as usize)
            .map(|profile| &profile.layout),
    }
}

fn layout_mut(config: &mut WorkspaceConfig, scope: i32) -> Option<&mut Layout> {
    match scope {
        0 => Some(&mut config.upper),
        1 => Some(&mut config.default),
        _ => config
            .profiles
            .get_mut((scope - 2) as usize)
            .map(|profile| &mut profile.layout),
    }
}

fn app_key(text: &str) -> Option<AppKey> {
    let text = text.trim();
    if text.contains('!') {
        AppKey::package(text)
    } else {
        let path = Path::new(text);
        if !path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
        {
            return None;
        }
        AppKey::executable(path)
    }
}

fn key_text(key: &AppKey) -> &str {
    match key {
        AppKey::Executable(path) | AppKey::Package(path) => path,
    }
}

fn key_name(key: &AppKey) -> String {
    match key {
        AppKey::Executable(path) => Path::new(path)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("应用")
            .into(),
        AppKey::Package(id) => id.split('!').next().unwrap_or(id).into(),
    }
}
