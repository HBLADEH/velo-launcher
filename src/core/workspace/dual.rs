use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};

use super::actions::{Template, WorkspaceAction};
use super::config::AppKey;
use super::directions::Direction;
use super::user_actions::UserAction;
use crate::core::{DataError, Result, UnknownFields};

pub const MAX_COLUMNS: usize = 5;
pub const ROWS: usize = 2;
const SLOTS: usize = MAX_COLUMNS * ROWS;

pub(super) const GLOBAL_ACTIONS: [Option<WorkspaceAction>; 6] = [
    Some(WorkspaceAction::Explorer),
    Some(WorkspaceAction::Calculator),
    Some(WorkspaceAction::TaskManager),
    None,
    Some(WorkspaceAction::Launcher),
    Some(WorkspaceAction::ControlPanel),
];

pub(super) const CONTEXT_ACTIONS: [Option<WorkspaceAction>; 3] = [
    Some(WorkspaceAction::Downloads),
    Some(WorkspaceAction::Documents),
    Some(WorkspaceAction::Pictures),
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Matrix {
    pub columns: usize,
    pub slots: Vec<Option<String>>,
    #[serde(flatten)]
    pub unknown: UnknownFields,
}

impl Default for Matrix {
    fn default() -> Self {
        Self {
            columns: 3,
            slots: vec![None; SLOTS],
            unknown: UnknownFields::new(),
        }
    }
}

impl Matrix {
    pub(super) fn from_actions(
        columns: usize,
        actions: impl IntoIterator<Item = Option<WorkspaceAction>>,
    ) -> Self {
        let mut matrix = Self {
            columns,
            ..Self::default()
        };
        for (slot, action) in matrix.slots.iter_mut().zip(actions) {
            *slot = action.map(|action| action.id().into());
        }
        matrix
    }

    fn template(template: Template) -> Self {
        use WorkspaceAction::*;
        let actions = match template {
            Template::Explorer => [Downloads, Documents, ThisPc],
            Template::Code => [CodeTerminal, CodeCommands, CodeFind],
            Template::Edge => [EdgeNewTab, EdgeAddress, EdgeBookmarks],
        };
        Self::from_actions(3, actions.map(Some))
    }

    pub fn visible(&self) -> &[Option<String>] {
        &self.slots[..self.columns * ROWS]
    }

    fn validate(&mut self) -> bool {
        if !(3..=MAX_COLUMNS).contains(&self.columns) || self.slots.len() > SLOTS {
            return false;
        }
        self.slots.resize(SLOTS, None);
        true
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PanelProfile {
    pub app: AppKey,
    pub name: String,
    #[serde(default)]
    pub template: Option<Template>,
    pub matrix: Matrix,
    #[serde(flatten)]
    pub unknown: UnknownFields,
}

impl PanelProfile {
    pub fn new(app: AppKey, name: String, template: Option<Template>, default: &Matrix) -> Self {
        Self {
            app,
            name,
            template,
            matrix: template.map_or_else(|| default.clone(), Matrix::template),
            unknown: UnknownFields::new(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GestureMap {
    pub slots: [Option<String>; 8],
    #[serde(flatten)]
    pub unknown: UnknownFields,
}

impl GestureMap {
    pub fn get(&self, direction: Direction) -> Option<&str> {
        self.slots[direction.index()].as_deref()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GestureProfile {
    pub app: AppKey,
    pub name: String,
    #[serde(default)]
    pub template: Option<Template>,
    pub directions: GestureMap,
    #[serde(flatten)]
    pub unknown: UnknownFields,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DualConfig {
    pub global: Matrix,
    pub default_panel: Matrix,
    pub panel_profiles: Vec<PanelProfile>,
    pub default_gestures: GestureMap,
    pub gesture_profiles: Vec<GestureProfile>,
    pub actions: BTreeMap<String, UserAction>,
    #[serde(flatten)]
    pub unknown: UnknownFields,
}

impl DualConfig {
    pub fn initial() -> Self {
        Self {
            global: Matrix::from_actions(3, GLOBAL_ACTIONS),
            default_panel: Matrix::from_actions(3, CONTEXT_ACTIONS),
            ..Self::default()
        }
    }

    pub fn panel(&self, app: Option<&AppKey>) -> Option<&PanelProfile> {
        self.panel_profiles
            .iter()
            .find(|profile| Some(&profile.app) == app)
    }

    pub fn gestures(&self, app: Option<&AppKey>) -> Option<&GestureProfile> {
        self.gesture_profiles
            .iter()
            .find(|profile| Some(&profile.app) == app)
    }

    pub fn builtin(id: &str) -> Option<WorkspaceAction> {
        WorkspaceAction::ALL
            .into_iter()
            .find(|action| action.id() == id)
    }

    pub fn validate(&mut self) -> Result<()> {
        let invalid = |reason| DataError::new("validate dual Workspace", reason);
        if !self.global.validate() || !self.default_panel.validate() {
            return Err(invalid("invalid matrix size"));
        }
        if self.actions.len() > 128
            || self
                .actions
                .iter()
                .any(|(id, action)| !id.starts_with("action:") || id.len() > 64 || !action.valid())
        {
            return Err(invalid("invalid action library"));
        }
        let mut seen = HashSet::new();
        for profile in &mut self.panel_profiles {
            profile.app = profile
                .app
                .clone()
                .normalize()
                .ok_or_else(|| invalid("invalid panel application"))?;
            if profile.name.trim().is_empty()
                || !profile.matrix.validate()
                || !seen.insert(profile.app.clone())
            {
                return Err(invalid("invalid or duplicate panel profile"));
            }
        }
        let mut seen = HashSet::new();
        for profile in &mut self.gesture_profiles {
            profile.app = profile
                .app
                .clone()
                .normalize()
                .ok_or_else(|| invalid("invalid gesture application"))?;
            if profile.name.trim().is_empty() || !seen.insert(profile.app.clone()) {
                return Err(invalid("invalid or duplicate gesture profile"));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_app_gestures_override_default_without_affecting_panel() {
        let app = AppKey::package("example!app").unwrap();
        let mut dual = DualConfig::default();
        dual.default_gestures.slots[Direction::North.index()] = Some("launcher".into());
        dual.gesture_profiles.push(GestureProfile {
            app: app.clone(),
            name: "Example".into(),
            template: None,
            directions: GestureMap::default(),
            unknown: UnknownFields::new(),
        });
        assert!(dual.panel(Some(&app)).is_none());
        assert_eq!(
            dual.gestures(Some(&app))
                .unwrap()
                .directions
                .get(Direction::North),
            None
        );
        assert_eq!(
            dual.default_gestures.get(Direction::North),
            Some("launcher")
        );
    }
}
