use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;

use slint::winit_030::winit::{
    event::KeyEvent,
    keyboard::{KeyCode, ModifiersState, PhysicalKey},
};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use velo_slint::core::workspace::{
    actions::{Template, WorkspaceAction},
    config::{AppKey, MouseButton, WorkspaceConfig},
    directions::Direction,
    dual::{DualConfig, GestureMap, GestureProfile, PanelProfile},
    user_actions::{Action, KeyStep, Script, UserAction},
};
use velo_slint::platform::windows::{
    hotkey::Key, interaction, workspace::Foreground, workspace_actions::Application,
};

use crate::{
    AppWindow, WorkspaceActionDraft, WorkspaceChoice, WorkspaceDraft, WorkspaceEditorState,
    WorkspaceTile,
};

use super::{Controller, native_window_from_app};

const GENERAL: i32 = 0;
pub(super) const PANEL: i32 = 1;
pub(super) const GESTURE: i32 = 2;
const LIBRARY: i32 = 3;
const ICONS: [&str; 6] = ["", "document", "keyboard", "terminal", "folder", "star"];
type TestTarget = (Foreground, Application, Option<Foreground>);

#[derive(Default)]
pub(super) struct Editor {
    draft: RefCell<Option<WorkspaceConfig>>,
    updating: Cell<bool>,
    position: Cell<(i32, i32)>,
    library_index: Cell<i32>,
    form: RefCell<WorkspaceActionDraft>,
    choices: RefCell<Vec<String>>,
    confirm_delete: RefCell<Option<String>>,
    test_target: RefCell<Option<TestTarget>>,
}

impl Editor {
    pub fn set_test_target(&self, target: Option<TestTarget>) {
        self.test_target.replace(target);
    }
}

impl Controller {
    pub(super) fn connect_editor(self: &Rc<Self>, app: &AppWindow) {
        let model = app.global::<WorkspaceEditorState>();
        let weak = Rc::downgrade(self);
        model.on_open(move || {
            if let Some(controller) = weak.upgrade()
                && let Some(app) = controller.launcher.upgrade()
            {
                controller.open_editor(&app);
            }
        });
        let weak = Rc::downgrade(self);
        model.on_close(move || {
            if let Some(controller) = weak.upgrade()
                && let Some(app) = controller.launcher.upgrade()
            {
                controller.close_editor(&app);
            }
        });
        let weak = Rc::downgrade(self);
        model.on_save(move || {
            if let Some(controller) = weak.upgrade()
                && let Some(app) = controller.launcher.upgrade()
            {
                controller.save_editor(&app);
            }
        });
        let weak = Rc::downgrade(self);
        model.on_identity_changed(move || {
            if let Some(controller) = weak.upgrade()
                && let Some(app) = controller.launcher.upgrade()
            {
                controller.refresh_exclusion(&app);
            }
        });
        let weak = Rc::downgrade(self);
        app.on_workspace_recorded_chord(move |chord| {
            if let Some(controller) = weak.upgrade()
                && let Some(app) = controller.launcher.upgrade()
            {
                controller.append_chord(&app, &chord);
            }
        });
        let weak = Rc::downgrade(self);
        model.on_change(move || {
            if let Some(controller) = weak.upgrade() {
                controller.dual_selection();
            }
        });
        let weak = Rc::downgrade(self);
        model.on_command(move |command, arg| {
            if let Some(controller) = weak.upgrade() {
                controller.dual_command(&command, arg);
            }
        });
        let weak = Rc::downgrade(self);
        model.on_drop_action(move |id, x, y| {
            if let Some(controller) = weak.upgrade()
                && let Some(app) = controller.launcher.upgrade()
            {
                let model = app.global::<WorkspaceEditorState>();
                let section = model.get_section();
                let columns = if section == PANEL {
                    model.get_columns() + 3
                } else {
                    3
                };
                if !(0.0..1.0).contains(&x) || y < 0.0 || y % 60.0 >= 54.0 {
                    return;
                }
                let cell = (y / 60.0) as i32 * columns + (x * columns as f32) as i32;
                let slot = match (section, cell) {
                    (PANEL, value) if value < columns * 2 => value,
                    (GESTURE, 0..=3) => cell,
                    (GESTURE, 5..=8) => cell - 1,
                    _ => return,
                };
                model.set_selected(slot);
                controller.dual_command("assign", id);
            }
        });
    }

    fn open_editor(&self, app: &AppWindow) {
        if self.editor.draft.borrow().is_none() {
            self.load_editor(app, true);
        }
    }

    fn load_editor(&self, app: &AppWindow, first_open: bool) {
        let model = app.global::<WorkspaceEditorState>();
        let Some(config) = self.committed_config() else {
            model.set_draft_open(false);
            app.set_workspace_notice("工作台配置不可读，原文件已保留".into());
            return;
        };
        let mut fields = model.get_draft();
        fields.enabled = config.enabled;
        fields.hotkey = config.hotkey.as_str().into();
        fields.mouse = match config.mouse {
            None => 0,
            Some(MouseButton::XButton2) => 1,
            Some(MouseButton::XButton1) => 2,
        };
        if self.preview {
            let bindings = self.bindings.borrow();
            fields.enabled |= bindings.keyboard.is_some() || bindings.mouse.is_some();
            if let Some(keyboard) = &bindings.keyboard {
                fields.hotkey = keyboard.key.to_string().into();
            }
            if let Some(mouse) = &bindings.mouse {
                fields.mouse = match mouse.button {
                    super::Button::XButton1 => 2,
                    super::Button::XButton2 => 1,
                };
            }
        }
        self.editor.draft.replace(Some((*config).clone()));
        self.editor.updating.set(true);
        if first_open {
            model.set_section(GENERAL);
            model.set_profile(0);
            model.set_library_index(0);
            self.editor.library_index.set(0);
            self.editor.position.set((GENERAL, 0));
        }
        model.set_draft(fields);
        model.set_draft_open(true);
        self.load_action_form(app, self.editor.library_index.get());
        self.editor.updating.set(false);
        self.refresh_exclusion(app);
        self.refresh_editor(app, first_open);
    }

    fn save_editor(&self, app: &AppWindow) {
        let model = app.global::<WorkspaceEditorState>();
        if model.get_busy() || model.get_recording() || !self.apply_action_form(app) {
            return;
        }
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
                self.load_editor(&app, false);
            }
        }
    }

    fn refresh_exclusion(&self, app: &AppWindow) {
        let model = app.global::<WorkspaceEditorState>();
        let key = app_key(&model.get_draft().application);
        model.set_excluded(key.is_some_and(|key| {
            self.editor
                .draft
                .borrow()
                .as_ref()
                .is_some_and(|config| config.excluded.contains(&key))
        }));
    }

    fn toggle_exclusion(&self, app: &AppWindow) {
        let model = app.global::<WorkspaceEditorState>();
        let Some(key) = app_key(&model.get_draft().application) else {
            app.set_workspace_notice("请输入应用的完整可执行文件路径或包标识".into());
            return;
        };
        if let Some(config) = self.editor.draft.borrow_mut().as_mut() {
            if let Some(index) = config.excluded.iter().position(|excluded| excluded == &key) {
                config.excluded.remove(index);
            } else {
                config.excluded.push(key);
            }
        }
        self.refresh_exclusion(app);
        app.set_workspace_notice("".into());
    }

    pub(super) fn editor_at(
        &self,
        app: &AppWindow,
        section: i32,
        key: Option<&AppKey>,
        local: bool,
    ) {
        self.open_editor(app);
        if self.editor.draft.borrow().is_none() || !self.apply_action_form(app) {
            return;
        }
        let model = app.global::<WorkspaceEditorState>();
        let profile = if let Some(config) = self
            .editor
            .draft
            .borrow()
            .as_ref()
            .map(|config| &config.dual)
        {
            if section == GESTURE {
                config
                    .gesture_profiles
                    .iter()
                    .position(|profile| Some(&profile.app) == key)
                    .map_or(0, |index| index as i32 + 1)
            } else if local {
                config
                    .panel_profiles
                    .iter()
                    .position(|profile| Some(&profile.app) == key)
                    .map_or(1, |index| index as i32 + 2)
            } else {
                0
            }
        } else {
            0
        };
        self.editor.updating.set(true);
        model.set_section(section);
        model.set_profile(profile);
        self.editor.position.set((section, profile));
        self.editor.updating.set(false);
        self.refresh_editor(app, true);
        if let Some(key) = key {
            set_identity(app, key_text(key));
        }
    }

    fn close_editor(&self, app: &AppWindow) {
        self.editor.draft.borrow_mut().take();
        self.stop_action_recording(app);
        let model = app.global::<WorkspaceEditorState>();
        model.set_draft_open(false);
        model.set_draft(WorkspaceDraft::default());
        model.set_action(WorkspaceActionDraft::default());
        model.set_tiles(Default::default());
        model.set_choices(Default::default());
        model.set_library(Default::default());
        model.set_profiles(Default::default());
        model.set_slots(Default::default());
        model.set_action_references("".into());
        self.editor.choices.borrow_mut().clear();
        self.editor.set_test_target(None);
        self.editor.confirm_delete.borrow_mut().take();
        self.editor.form.replace(WorkspaceActionDraft::default());
    }

    fn dual_selection(&self) {
        if self.editor.updating.get() {
            return;
        }
        let Some(app) = self.launcher.upgrade() else {
            return;
        };
        let model = app.global::<WorkspaceEditorState>();
        if !model.get_draft_open() {
            return;
        }
        let previous = self.editor.position.get();
        let old_library = self.editor.library_index.get();
        let section = model.get_section();
        let library = model.get_library_index();
        if previous.0 == LIBRARY
            && (section != LIBRARY || library != old_library)
            && !self.apply_action_form(&app)
        {
            self.editor.updating.set(true);
            model.set_section(LIBRARY);
            model.set_library_index(old_library);
            self.editor.updating.set(false);
            return;
        }
        let profile = if section == previous.0 {
            model.get_profile()
        } else {
            0
        };
        let changed = (section, profile) != previous;
        self.editor.updating.set(true);
        model.set_profile(profile);
        self.editor.position.set((section, profile));
        if !changed
            && section == PANEL
            && let Some(config) = self
                .editor
                .draft
                .borrow_mut()
                .as_mut()
                .map(|config| &mut config.dual)
            && let Some((columns, _, _)) = columns_slots(config, section, profile)
            && let Ok(value @ 0..=2) = usize::try_from(model.get_columns())
        {
            *columns = value + 3;
        }
        if section == LIBRARY && library != old_library {
            self.editor.library_index.set(library);
            self.load_action_form(&app, library);
        }
        self.editor.updating.set(false);
        self.refresh_editor(&app, changed);
    }

    fn dual_command(self: &Rc<Self>, command: &str, arg: i32) {
        let Some(app) = self.launcher.upgrade() else {
            return;
        };
        let model = app.global::<WorkspaceEditorState>();
        if !model.get_draft_open() || model.get_busy() || app.get_native_dialog_active() {
            return;
        }
        match command {
            "exclude" => self.toggle_exclusion(&app),
            "browse-app" => self.browse_dual(&app, interaction::BROWSE_EXECUTABLE),
            "browse-script" => self.browse_dual(&app, interaction::BROWSE_BATCH),
            "commit-action" => {
                self.apply_action_form(&app);
            }
            "new-action" => {
                self.select_library(&app, 0);
                app.set_workspace_notice("".into());
            }
            "copy-action" => {
                let mut fields = model.get_action();
                self.select_library(&app, 0);
                fields.label = format!("{} 副本", fields.label).into();
                fields.trust = false;
                fields.trusted = false;
                model.set_action(fields);
            }
            "delete-action" => self.delete_action(&app),
            "disable-script" => self.disable_script(&app),
            "test-action" => self.test_action(&app),
            "record" => {
                if model.get_recording() {
                    self.stop_action_recording(&app);
                } else if model.get_action().kind == 1 {
                    model.set_recording(true);
                    app.set_workspace_recording(true);
                    app.invoke_suspend_input();
                }
            }
            _ => self.edit_slots(&app, command, arg),
        }
    }

    fn edit_slots(&self, app: &AppWindow, command: &str, arg: i32) {
        let model = app.global::<WorkspaceEditorState>();
        let (section, mut profile) = self.editor.position.get();
        if !matches!(section, PANEL | GESTURE) {
            return;
        }
        let fields = app.global::<WorkspaceEditorState>().get_draft();
        let selected = model.get_selected() as usize;
        let changed = {
            let mut draft = self.editor.draft.borrow_mut();
            let Some(dual) = draft.as_mut().map(|config| &mut config.dual) else {
                return;
            };
            match command {
                "profile" | "copy-profile" => {
                    let Some(key) = app_key(&fields.application) else {
                        app.set_workspace_notice("请输入完整 .exe 路径或包标识".into());
                        return;
                    };
                    let copy = command == "copy-profile";
                    if let Some(next) = create_profile(
                        dual,
                        section,
                        profile,
                        key,
                        template_at(fields.template),
                        copy,
                    ) {
                        profile = next;
                    } else {
                        app.set_workspace_notice("该应用已有配置，请先移除已有配置".into());
                        return;
                    }
                    true
                }
                "remove" if section == PANEL && profile >= 2 => {
                    if (profile - 2) as usize >= dual.panel_profiles.len() {
                        return;
                    }
                    dual.panel_profiles.remove((profile - 2) as usize);
                    profile = 1;
                    true
                }
                "remove" if section == GESTURE && profile >= 1 => {
                    if (profile - 1) as usize >= dual.gesture_profiles.len() {
                        return;
                    }
                    dual.gesture_profiles.remove((profile - 1) as usize);
                    profile = 0;
                    true
                }
                "reset" => {
                    reset_layout(dual, section, profile);
                    true
                }
                "assign" | "clear" | "move" => {
                    let Some((_, slots, count)) = columns_slots(dual, section, profile) else {
                        return;
                    };
                    if selected >= count {
                        return;
                    }
                    let selected = slot_storage(section, selected);
                    match command {
                        "assign" => {
                            let Some(id) = self.editor.choices.borrow().get(arg as usize).cloned()
                            else {
                                return;
                            };
                            slots[selected] = Some(id);
                        }
                        "clear" => slots[selected] = None,
                        _ if (0..count as i32).contains(&arg) => {
                            slots.swap(selected, slot_storage(section, arg as usize));
                            model.set_selected(arg);
                        }
                        _ => return,
                    }
                    false
                }
                _ => return,
            }
        };
        self.editor.updating.set(true);
        model.set_profile(profile);
        self.editor.position.set((section, profile));
        self.editor.updating.set(false);
        app.set_workspace_notice("".into());
        self.refresh_editor(app, changed);
    }

    fn refresh_editor(&self, app: &AppWindow, load_selection: bool) {
        let model = app.global::<WorkspaceEditorState>();
        let draft = self.editor.draft.borrow();
        let Some(dual) = draft.as_ref().map(|config| &config.dual) else {
            return;
        };
        let (section, profile) = self.editor.position.get();
        self.editor.updating.set(true);
        match section {
            PANEL | GESTURE => {
                let mut columns = 3;
                let (slots, template, app_key, profiles): (_, _, _, Vec<SharedString>) = if section
                    == PANEL
                {
                    let profile_view = profile
                        .checked_sub(2)
                        .and_then(|index| dual.panel_profiles.get(index as usize));
                    let matrix = if profile == 0 {
                        &dual.global
                    } else {
                        profile_view.map_or(&dual.default_panel, |profile| &profile.matrix)
                    };
                    columns = matrix.columns;
                    (
                        matrix.visible(),
                        profile_view.and_then(|p| p.template),
                        profile_view.map(|p| &p.app),
                        ["全局动作".into(), "默认工作台".into()]
                            .into_iter()
                            .chain(dual.panel_profiles.iter().map(|p| p.name.as_str().into()))
                            .collect(),
                    )
                } else {
                    let profile_view = profile
                        .checked_sub(1)
                        .and_then(|index| dual.gesture_profiles.get(index as usize));
                    let directions = profile_view.map_or(&dual.default_gestures, |p| &p.directions);
                    (
                        directions.slots.as_slice(),
                        profile_view.and_then(|p| p.template),
                        profile_view.map(|p| &p.app),
                        std::iter::once("默认手势".into())
                            .chain(dual.gesture_profiles.iter().map(|p| p.name.as_str().into()))
                            .collect(),
                    )
                };
                model.set_profiles(rows(profiles));
                model.set_columns((columns - 3) as i32);
                let indices = 0..slots.len();
                model.set_tiles(rows(indices.clone().map(|index| {
                    let id = slots[slot_storage(section, index)].as_deref();
                    let (label, glyph) = descriptor(dual, id);
                    WorkspaceTile {
                        label: label.into(),
                        glyph: glyph.into(),
                        direction: if section == GESTURE {
                            Direction::GRID[index].icon_path().into()
                        } else {
                            "".into()
                        },
                        enabled: id.is_some(),
                    }
                })));
                model.set_slots(rows(indices.map(|index| {
                    let label = descriptor(dual, slots[slot_storage(section, index)].as_deref()).0;
                    let label = if label.is_empty() { "空槽" } else { label };
                    if section == GESTURE {
                        format!("{} · {label}", Direction::GRID[index].label()).into()
                    } else {
                        format!("{} · {label}", index + 1).into()
                    }
                })));
                let choices = WorkspaceAction::ALL
                    .into_iter()
                    .filter(|action| {
                        action
                            .template()
                            .is_none_or(|required| template == Some(required))
                    })
                    .map(|action| (action.id(), action.label(), action.glyph()))
                    .chain(
                        dual.actions
                            .iter()
                            .map(|(id, spec)| (id.as_str(), spec.label.as_str(), spec.glyph())),
                    );
                let mut ids = self.editor.choices.borrow_mut();
                ids.clear();
                model.set_choices(rows(choices.enumerate().map(
                    |(index, (id, label, glyph))| {
                        ids.push(id.to_owned());
                        WorkspaceChoice {
                            id: index as i32,
                            label: label.into(),
                            glyph: glyph.into(),
                        }
                    },
                )));
                if load_selection || model.get_selected() as usize >= slots.len() {
                    model.set_selected(0);
                }
                if load_selection && let Some(key) = app_key {
                    set_identity(app, key_text(key));
                }
            }
            LIBRARY => model.set_library(rows(
                std::iter::once("新建动作".into()).chain(
                    dual.actions
                        .values()
                        .map(|action| action.label.as_str().into()),
                ),
            )),
            _ => {}
        }
        self.editor.updating.set(false);
    }

    fn select_library(&self, app: &AppWindow, index: i32) {
        self.editor.updating.set(true);
        app.global::<WorkspaceEditorState>()
            .set_library_index(index);
        self.editor.library_index.set(index);
        self.load_action_form(app, index);
        self.editor.updating.set(false);
    }

    fn load_action_form(&self, app: &AppWindow, index: i32) {
        let draft = self.editor.draft.borrow();
        let selected = draft.as_ref().map(|config| &config.dual).and_then(|dual| {
            let (id, action) = library_item(dual, index)?;
            Some((form_for(action), references(dual, id)))
        });
        let (form, refs) = selected.unwrap_or_else(|| {
            (
                WorkspaceActionDraft {
                    timeout: "15".into(),
                    ..Default::default()
                },
                String::new(),
            )
        });
        let model = app.global::<WorkspaceEditorState>();
        model.set_action(form.clone());
        model.set_action_references(refs.into());
        self.editor.form.replace(form);
        self.editor.confirm_delete.borrow_mut().take();
    }

    pub(super) fn apply_action_form(&self, app: &AppWindow) -> bool {
        if self.editor.position.get().0 != LIBRARY {
            return true;
        }
        let model = app.global::<WorkspaceEditorState>();
        let form = model.get_action();
        if form == *self.editor.form.borrow() {
            return true;
        }
        let selected = self.editor.library_index.get();
        let index = {
            let mut draft = self.editor.draft.borrow_mut();
            let Some(dual) = draft.as_mut().map(|config| &mut config.dual) else {
                return false;
            };
            let old = library_item(dual, selected);
            let spec = match parse_form(&form, old.map(|(_, item)| item)) {
                Ok(spec) => spec,
                Err(message) => {
                    app.set_workspace_notice(message.into());
                    return false;
                }
            };
            let id = if let Some((id, _)) = old {
                id.to_owned()
            } else {
                if dual.actions.len() == 128 {
                    app.set_workspace_notice("动作库已达到 128 项上限".into());
                    return false;
                }
                let Some(id) = interaction::new_group_id() else {
                    app.set_workspace_notice("暂时无法新建动作，请重试".into());
                    return false;
                };
                id.replacen("group:", "action:", 1)
            };
            dual.actions.insert(id.clone(), spec);
            dual.actions
                .keys()
                .position(|key| *key == id)
                .map_or(0, |index| index as i32 + 1)
        };
        self.refresh_editor(app, false);
        self.select_library(app, index);
        app.set_workspace_notice("动作已加入草稿，保存设置后生效".into());
        true
    }

    fn delete_action(&self, app: &AppWindow) {
        {
            let mut draft = self.editor.draft.borrow_mut();
            let Some(dual) = draft.as_mut().map(|config| &mut config.dual) else {
                return;
            };
            let Some((id, _)) = library_item(dual, self.editor.library_index.get()) else {
                return;
            };
            let refs = references(dual, id);
            if !refs.is_empty() && self.editor.confirm_delete.borrow().as_deref() != Some(id) {
                self.editor.confirm_delete.replace(Some(id.into()));
                app.set_workspace_notice(
                    format!("{refs}。再次点击删除后，引用将显示为不可用。").into(),
                );
                return;
            }
            let id = id.to_owned();
            dual.actions.remove(&id);
        }
        self.select_library(app, 0);
        self.refresh_editor(app, false);
        app.set_workspace_notice("动作已从草稿移除".into());
    }

    fn disable_script(&self, app: &AppWindow) {
        let selected = self.editor.library_index.get();
        {
            let mut draft = self.editor.draft.borrow_mut();
            let Some(dual) = draft.as_mut().map(|config| &mut config.dual) else {
                return;
            };
            let Some(action) = dual
                .actions
                .values_mut()
                .nth(selected.saturating_sub(1) as usize)
            else {
                return;
            };
            if let Action::RunScript(script) = &mut action.action {
                script.digest = None;
                script.review = false;
            }
        }
        self.select_library(app, selected);
        app.set_workspace_notice("脚本已在草稿中停用，保存设置后生效".into());
    }

    fn test_action(&self, app: &AppWindow) {
        let model = app.global::<WorkspaceEditorState>();
        if model.get_recording() {
            return;
        }
        let draft = self.editor.draft.borrow();
        let old = draft
            .as_ref()
            .map(|config| &config.dual)
            .and_then(|dual| library_item(dual, self.editor.library_index.get()))
            .map(|(_, action)| action);
        let action = match parse_form(&model.get_action(), old) {
            Ok(action) => action,
            Err(error) => {
                app.set_workspace_notice(error.into());
                return;
            }
        };
        drop(draft);
        if matches!(&action.action, Action::RunScript(script) if script.review || script.digest.is_none())
        {
            app.set_workspace_notice("请保存并确认信任后再测试脚本".into());
            return;
        }
        let Some(native) = native_window_from_app(app) else {
            return;
        };
        let target = self.editor.test_target.borrow().clone();
        if action.action.requires_target()
            && !target
                .as_ref()
                .is_some_and(|(window, _, _)| native.active() && window.restore(native))
        {
            app.set_workspace_notice("请在目标应用中打开工作台，从控制带进入设置后测试输入".into());
            return;
        }
        if self.business.upgrade().is_some_and(|business| {
            self.submit_user_action(app, &business, &action, target, native.hwnd() as isize)
        }) {
            app.set_workspace_notice("正在测试动作".into());
        } else {
            app.set_workspace_notice("另一个工作台动作正在执行，请稍候".into());
        }
    }

    fn browse_dual(&self, app: &AppWindow, request: i32) {
        let Some(owner) = native_window_from_app(app) else {
            return;
        };
        let owner = owner.hwnd() as isize;
        app.set_native_dialog_active(true);
        app.invoke_suspend_input();
        let weak = app.as_weak();
        if std::thread::Builder::new()
            .name("velo-workspace-picker".into())
            .spawn(move || {
                let result = interaction::run(owner, request);
                let _ = weak.upgrade_in_event_loop(move |app| {
                    app.set_native_dialog_active(false);
                    if let Some(path) = result.paths.first()
                        && let Some(path) = path.to_str()
                    {
                        if request == interaction::BROWSE_BATCH {
                            let model = app.global::<WorkspaceEditorState>();
                            let mut fields = model.get_action();
                            fields.content = path.into();
                            fields.trust = false;
                            fields.trusted = false;
                            model.set_action(fields);
                        } else {
                            set_identity(&app, path);
                        }
                    }
                    if !result.error.is_empty() {
                        app.set_workspace_notice(result.error.into());
                    }
                    app.invoke_native_interaction_finished();
                });
            })
            .is_err()
        {
            app.set_native_dialog_active(false);
            app.set_workspace_notice("无法打开文件选择器，请输入完整路径".into());
            app.invoke_native_interaction_finished();
        }
    }

    pub fn stop_action_recording(&self, app: &AppWindow) {
        app.global::<WorkspaceEditorState>().set_recording(false);
        app.set_workspace_recording(false);
    }

    pub fn record_action_key(
        &self,
        app: &AppWindow,
        event: &KeyEvent,
        modifiers: ModifiersState,
    ) -> bool {
        if !app.get_workspace_recording() {
            return false;
        }
        if event.repeat || !event.state.is_pressed() {
            return true;
        }
        let PhysicalKey::Code(code) = event.physical_key else {
            return true;
        };
        if code == KeyCode::Escape {
            self.stop_action_recording(app);
        } else if let Some(code) = crate::lifecycle::virtual_key(code) {
            let modifiers = u32::from(modifiers.alt_key())
                | u32::from(modifiers.control_key()) << 1
                | u32::from(modifiers.shift_key()) << 2
                | u32::from(modifiers.super_key()) << 3;
            self.append_chord(app, &Key { code, modifiers }.to_string());
        }
        true
    }

    pub(super) fn append_chord(&self, app: &AppWindow, chord: &str) {
        if !app.get_workspace_recording() {
            return;
        }
        let model = app.global::<WorkspaceEditorState>();
        let mut fields = model.get_action();
        fields.content = if fields.content.is_empty() {
            chord.into()
        } else {
            format!("{}\n{chord}", fields.content.trim_end()).into()
        };
        model.set_action(fields);
        self.stop_action_recording(app);
    }
}

fn set_identity(app: &AppWindow, identity: &str) {
    let model = app.global::<WorkspaceEditorState>();
    let mut fields = model.get_draft();
    fields.application = identity.into();
    model.set_draft(fields);
}

fn template_at(index: i32) -> Option<Template> {
    Template::ALL.get(index.checked_sub(1)? as usize).copied()
}

fn columns_slots(
    dual: &mut DualConfig,
    section: i32,
    profile: i32,
) -> Option<(&mut usize, &mut [Option<String>], usize)> {
    if section == PANEL {
        let matrix = match profile {
            0 => &mut dual.global,
            1 => &mut dual.default_panel,
            _ => &mut dual.panel_profiles.get_mut((profile - 2) as usize)?.matrix,
        };
        let count = matrix.visible().len();
        return Some((&mut matrix.columns, &mut matrix.slots, count));
    }
    let directions = if profile == 0 {
        &mut dual.default_gestures
    } else {
        &mut dual
            .gesture_profiles
            .get_mut((profile - 1) as usize)?
            .directions
    };
    Some((&mut dual.global.columns, &mut directions.slots, 8))
}

fn slot_storage(section: i32, index: usize) -> usize {
    if section == GESTURE {
        Direction::GRID[index].index()
    } else {
        index
    }
}

fn descriptor<'a>(dual: &'a DualConfig, id: Option<&str>) -> (&'a str, &'a str) {
    let Some(id) = id else { return ("", "plus") };
    if let Some(action) = DualConfig::builtin(id) {
        return (action.label(), action.glyph());
    }
    dual.actions
        .get(id)
        .map_or(("动作不可用", "apps"), |action| {
            (action.label.as_str(), action.glyph())
        })
}

fn create_profile(
    dual: &mut DualConfig,
    section: i32,
    source: i32,
    app: AppKey,
    template: Option<Template>,
    copy: bool,
) -> Option<i32> {
    if section == PANEL {
        if let Some(index) = dual
            .panel_profiles
            .iter()
            .position(|profile| profile.app == app)
        {
            return (!copy).then_some(index as i32 + 2);
        }
        let mut profile =
            PanelProfile::new(app.clone(), key_name(&app), template, &dual.default_panel);
        if copy {
            let (matrix, template) = match source {
                0 => (&dual.global, None),
                1 => (&dual.default_panel, None),
                _ => {
                    let source = dual.panel_profiles.get((source - 2) as usize)?;
                    (&source.matrix, source.template)
                }
            };
            profile.matrix = matrix.clone();
            profile.template = template;
        }
        dual.panel_profiles.push(profile);
        return Some(dual.panel_profiles.len() as i32 + 1);
    }
    if let Some(index) = dual
        .gesture_profiles
        .iter()
        .position(|profile| profile.app == app)
    {
        return (!copy).then_some(index as i32 + 1);
    }
    let mut directions = GestureMap::default();
    let mut match_template = template;
    if copy {
        if let Some(source) = source
            .checked_sub(1)
            .and_then(|index| dual.gesture_profiles.get(index as usize))
        {
            directions = source.directions.clone();
            match_template = source.template;
        } else {
            directions = dual.default_gestures.clone();
        }
    }
    dual.gesture_profiles.push(GestureProfile {
        name: key_name(&app),
        app,
        template: match_template,
        directions,
        unknown: Default::default(),
    });
    Some(dual.gesture_profiles.len() as i32)
}

fn reset_layout(dual: &mut DualConfig, section: i32, profile: i32) {
    if section == GESTURE {
        if let Some((_, slots, _)) = columns_slots(dual, section, profile) {
            slots.fill(None);
        }
        return;
    }
    let fresh = DualConfig::initial();
    let reset = match profile {
        0 => fresh.global,
        1 => fresh.default_panel,
        _ => {
            let Some(saved) = dual.panel_profiles.get((profile - 2) as usize) else {
                return;
            };
            PanelProfile::new(
                saved.app.clone(),
                saved.name.clone(),
                saved.template,
                &dual.default_panel,
            )
            .matrix
        }
    };
    if let Some((width, slots, _)) = columns_slots(dual, section, profile) {
        *width = reset.columns;
        slots.clone_from_slice(&reset.slots);
    }
}

fn library_item(dual: &DualConfig, selected: i32) -> Option<(&str, &UserAction)> {
    dual.actions
        .iter()
        .nth(selected.checked_sub(1)? as usize)
        .map(|(id, action)| (id.as_str(), action))
}

fn references(dual: &DualConfig, id: &str) -> String {
    let panel = [
        ("全局动作", &dual.global),
        ("默认工作台", &dual.default_panel),
    ]
    .into_iter()
    .chain(
        dual.panel_profiles
            .iter()
            .map(|profile| (profile.name.as_str(), &profile.matrix)),
    );
    let gestures = std::iter::once(("默认手势", &dual.default_gestures)).chain(
        dual.gesture_profiles
            .iter()
            .map(|profile| (profile.name.as_str(), &profile.directions)),
    );
    let names = panel
        .filter_map(|(name, matrix)| {
            matrix
                .slots
                .iter()
                .any(|slot| slot.as_deref() == Some(id))
                .then_some(name.to_owned())
        })
        .chain(
            gestures
                .filter(|(_, directions)| {
                    directions
                        .slots
                        .iter()
                        .any(|slot| slot.as_deref() == Some(id))
                })
                .map(|(name, _)| format!("{name}手势")),
        )
        .collect::<Vec<_>>();
    if names.is_empty() {
        String::new()
    } else {
        format!(
            "引用此动作的配置（{} 组）  {}",
            names.len(),
            names.join("、")
        )
    }
}

fn form_for(spec: &UserAction) -> WorkspaceActionDraft {
    let mut form = WorkspaceActionDraft {
        label: spec.label.as_str().into(),
        description: spec.description.as_str().into(),
        icon: ICONS
            .iter()
            .position(|&icon| icon == spec.icon)
            .unwrap_or_default() as i32,
        timeout: "15".into(),
        ..Default::default()
    };
    match &spec.action {
        Action::InsertText { text } => form.content = text.as_str().into(),
        Action::SendKeys { steps } => {
            form.kind = 1;
            form.content = steps
                .iter()
                .map(|step| {
                    if step.wait_ms == 0 {
                        step.chord.clone()
                    } else {
                        format!("{} @{}", step.chord, step.wait_ms)
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
                .into();
        }
        Action::RunScript(script) => {
            form.kind = 2;
            form.content = script.path.as_str().into();
            form.arguments = script.args.join("\n").into();
            form.directory = script.directory.as_str().into();
            form.timeout = script.timeout_secs.to_string().into();
            form.console = script.show_console;
            form.trust = script.review;
            form.trusted = script.digest.is_some();
        }
        Action::OpenTarget { target } => {
            form.kind = 3;
            form.content = target.as_str().into();
        }
    }
    form
}

fn parse_form(
    form: &WorkspaceActionDraft,
    old: Option<&UserAction>,
) -> Result<UserAction, &'static str> {
    let action = match form.kind {
        0 => Action::InsertText {
            text: form.content.to_string(),
        },
        1 => {
            let old_steps = old.and_then(|spec| match &spec.action {
                Action::SendKeys { steps } => Some(steps),
                _ => None,
            });
            let steps = form
                .content
                .lines()
                .filter(|line| !line.trim().is_empty())
                .enumerate()
                .map(|(index, line)| {
                    let (chord, wait_ms) = match line.rsplit_once('@') {
                        Some((chord, wait)) => (
                            chord,
                            wait.trim()
                                .parse()
                                .map_err(|_| "请输入有效的步骤等待毫秒数")?,
                        ),
                        None => (line, 0),
                    };
                    Ok(KeyStep {
                        chord: chord.trim().into(),
                        wait_ms,
                        unknown: old_steps
                            .and_then(|steps| steps.get(index))
                            .map(|step| step.unknown.clone())
                            .unwrap_or_default(),
                    })
                })
                .collect::<Result<Vec<_>, &str>>()?;
            Action::SendKeys { steps }
        }
        2 => {
            let previous = old.and_then(|spec| match &spec.action {
                Action::RunScript(script) => Some(script),
                _ => None,
            });
            let mut script = Script {
                path: form.content.trim().into(),
                args: form.arguments.lines().map(str::to_owned).collect(),
                directory: form.directory.trim().into(),
                timeout_secs: form
                    .timeout
                    .trim()
                    .parse()
                    .map_err(|_| "请输入 1 至 120 秒的脚本超时")?,
                show_console: form.console,
                digest: None,
                review: false,
                unknown: previous
                    .map(|script| script.unknown.clone())
                    .unwrap_or_default(),
            };
            if let Some(previous) = previous
                && script.path == previous.path
                && script.args == previous.args
                && script.directory == previous.directory
                && script.timeout_secs == previous.timeout_secs
                && script.show_console == previous.show_console
            {
                script.digest = previous.digest.clone();
                script.review = previous.review;
            }
            script.review |= form.trust;
            if !script.valid() {
                return Err("请输入有效的本地批处理路径、参数与超时设置");
            }
            Action::RunScript(script)
        }
        _ => Action::OpenTarget {
            target: form.content.trim().into(),
        },
    };
    let spec = UserAction {
        label: form.label.trim().into(),
        description: form.description.trim().into(),
        icon: ICONS.get(form.icon as usize).unwrap_or(&"").to_string(),
        action,
        unknown: old.map(|old| old.unknown.clone()).unwrap_or_default(),
    };
    spec.valid()
        .then_some(spec)
        .ok_or("请填写动作名称与有效参数")
}

fn rows<T: Clone + 'static>(values: impl IntoIterator<Item = T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(values.into_iter().collect::<Vec<_>>()))
}

pub(super) fn app_key(text: &str) -> Option<AppKey> {
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

pub(super) fn key_text(key: &AppKey) -> &str {
    match key {
        AppKey::Executable(path) | AppKey::Package(path) => path,
    }
}

pub(super) fn key_name(key: &AppKey) -> String {
    match key {
        AppKey::Executable(path) => Path::new(path)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("应用")
            .into(),
        AppKey::Package(id) => id.split('!').next().unwrap_or(id).into(),
    }
}
