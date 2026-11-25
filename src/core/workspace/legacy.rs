//! Read-only conversion of the former Workspace layout fields.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::actions::{Template, WorkspaceAction};
use super::config::AppKey;
use super::dual::{CONTEXT_ACTIONS, DualConfig, GLOBAL_ACTIONS, Matrix, PanelProfile, ROWS};
use crate::core::{DataError, Result, UnknownFields, decode_fields, defaults, overlay};

#[derive(Deserialize, Serialize)]
#[serde(default)]
struct LegacyLayout {
    count: u8,
    slots: [Option<WorkspaceAction>; 8],
    #[serde(flatten)]
    unknown: UnknownFields,
}

impl Default for LegacyLayout {
    fn default() -> Self {
        Self::new(4, &CONTEXT_ACTIONS)
    }
}

impl LegacyLayout {
    fn new(count: u8, actions: &[Option<WorkspaceAction>]) -> Self {
        let mut slots = [None; 8];
        slots[..actions.len()].copy_from_slice(actions);
        Self {
            count,
            slots,
            unknown: UnknownFields::new(),
        }
    }

    fn into_matrix(self) -> Result<Matrix> {
        if !matches!(self.count, 4 | 6 | 8) {
            return Err(DataError::new(
                "migrate Workspace layout",
                "invalid slot preset",
            ));
        }
        // Keep slot order and retain any assignments beyond the visible preset.
        let columns = usize::from(self.count).div_ceil(ROWS).max(3);
        let mut matrix = Matrix::from_actions(columns, self.slots);
        matrix.unknown = self.unknown;
        // A legacy extension may use a field now owned by the matrix schema.
        matrix.unknown.remove("columns");
        Ok(matrix)
    }
}

#[derive(Deserialize)]
struct LegacyProfile {
    app: AppKey,
    name: String,
    template: Option<Template>,
    layout: LegacyLayout,
    #[serde(flatten)]
    unknown: UnknownFields,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct LegacyConfig {
    upper: Option<Value>,
    default: LegacyLayout,
    profiles: Vec<LegacyProfile>,
}

pub(super) fn migrate(raw: &Value) -> Result<DualConfig> {
    let legacy: LegacyConfig = decode_fields(raw)?;
    let defaults_upper = LegacyLayout::new(6, &GLOBAL_ACTIONS);
    let global = if let Some(mut upper) = legacy.upper {
        overlay(defaults(&defaults_upper)?, &mut upper);
        decode_fields::<LegacyLayout>(&upper)?
    } else {
        defaults_upper
    };
    let panel_profiles = legacy
        .profiles
        .into_iter()
        .map(|mut profile| {
            profile.unknown.remove("matrix");
            Ok(PanelProfile {
                app: profile.app,
                name: profile.name,
                template: profile.template,
                matrix: profile.layout.into_matrix()?,
                unknown: profile.unknown,
            })
        })
        .collect::<Result<_>>()?;
    // A direction needs an explicit assignment before it can dispatch an action.
    Ok(DualConfig {
        global: global.into_matrix()?,
        default_panel: legacy.default.into_matrix()?,
        panel_profiles,
        ..DualConfig::default()
    })
}
