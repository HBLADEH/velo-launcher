use std::path::Path;

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use super::dual::DualConfig;
use super::legacy;
use crate::core::{DataError, Result, UnknownFields};
use crate::platform::windows::hotkey::Key;

pub const KEY: &str = "workspace";
pub const DEFAULT_HOTKEY: &str = "Ctrl+Alt+Space";

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "kebab-case")]
pub enum AppKey {
    Executable(String),
    Package(String),
}

impl AppKey {
    pub fn executable(path: &Path) -> Option<Self> {
        if !path.is_absolute() {
            return None;
        }
        let text = path.to_str()?.replace('/', "\\");
        let path = if let Some(unc) = text.strip_prefix("\\\\?\\UNC\\") {
            format!("\\\\{unc}")
        } else {
            text.trim_start_matches("\\\\?\\").to_owned()
        };
        (!path.contains('\0')).then(|| Self::Executable(path.to_lowercase()))
    }

    pub fn package(value: &str) -> Option<Self> {
        (!value.trim().is_empty() && !value.contains('\0'))
            .then(|| Self::Package(value.to_lowercase()))
    }

    pub(super) fn normalize(self) -> Option<Self> {
        match self {
            Self::Executable(path) => Self::executable(Path::new(&path)),
            Self::Package(id) => Self::package(&id),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MouseButton {
    XButton1,
    XButton2,
}

const OBSOLETE_FIELDS: [&str; 4] = ["upper", "default", "profiles", "dual_enabled"];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkspaceConfig {
    pub version: u32,
    pub enabled: bool,
    pub hotkey: String,
    pub mouse: Option<MouseButton>,
    pub excluded: Vec<AppKey>,
    #[serde(deserialize_with = "read_layouts")]
    pub dual: DualConfig,
    #[serde(flatten)]
    pub unknown: UnknownFields,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            version: 1,
            enabled: false,
            hotkey: DEFAULT_HOTKEY.into(),
            mouse: None,
            excluded: Vec::new(),
            dual: DualConfig::initial(),
            unknown: UnknownFields::new(),
        }
    }
}

fn read_layouts<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<DualConfig, D::Error> {
    Ok(Option::deserialize(deserializer)?.unwrap_or_else(DualConfig::initial))
}

impl WorkspaceConfig {
    pub(crate) fn read_enabled(config: &crate::core::config::Config) -> Option<Self> {
        if config.unknown.get(KEY)?.get("enabled")?.as_bool()? {
            Self::read(config).ok()
        } else {
            None
        }
    }

    pub fn read(config: &crate::core::config::Config) -> Result<Self> {
        let Some(raw) = config.unknown.get(KEY) else {
            return Ok(Self::default());
        };
        let mut value: Self = crate::core::decode_fields(raw)?;
        if value.version != 1 {
            return Err(DataError::new(
                "validate Workspace configuration",
                "unsupported version",
            ));
        }
        // A saved matrix takes precedence even if the former mode switch was off.
        if raw.get("dual").is_none_or(Value::is_null) {
            value.dual = legacy::migrate(raw)?;
        }
        for field in OBSOLETE_FIELDS {
            value.unknown.remove(field);
        }
        value.validate()?;
        Ok(value)
    }

    pub(crate) fn needs_layout_backup(raw: &Value) -> bool {
        raw.get("dual").is_none_or(Value::is_null)
            || OBSOLETE_FIELDS
                .into_iter()
                .any(|field| raw.get(field).is_some())
    }

    pub fn validate(&mut self) -> Result<()> {
        let invalid = |reason| DataError::new("validate Workspace configuration", reason);
        if self.version != 1 {
            return Err(invalid("unsupported version"));
        }
        self.hotkey = Key::parse(&self.hotkey)
            .map_err(|_| invalid("invalid Workspace hotkey"))?
            .to_string();
        for app in &mut self.excluded {
            *app = app
                .clone()
                .normalize()
                .ok_or_else(|| invalid("invalid excluded application"))?;
        }
        self.dual.validate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::Config;
    use crate::core::workspace::directions::Direction;
    use serde_json::json;

    #[test]
    fn old_layout_reads_as_matrix_with_stable_slots_and_never_binds_gestures() {
        let mut config = Config::default();
        let old = json!({
            "enabled": true, "mouse": "xbutton2", "future_workspace_field": 14,
            "upper": {"future": 42},
            "default": {"count": 8, "slots": ["documents", null, null, null, null, null, null, "this-pc"]},
            "profiles": [{
                "app": {"kind": "executable", "value": "C:/Apps/Editor/Code.exe"},
                "name": "VS Code", "glyph": "code", "template": "code",
                "layout": {"count": 4, "slots": ["code-terminal", null, null, null, "documents", null, null, null]},
                "extension": true
            }]
        });
        config.unknown.insert(KEY.into(), old.clone());
        let parsed = WorkspaceConfig::read_enabled(&config).unwrap();
        assert_eq!(parsed.mouse, Some(MouseButton::XButton2));
        assert_eq!(parsed.unknown["future_workspace_field"], 14);
        assert_eq!(parsed.dual.global.visible()[4].as_deref(), Some("launcher"));
        assert_eq!(parsed.dual.global.unknown["future"], 42);
        assert_eq!(parsed.dual.default_panel.columns, 4);
        assert_eq!(
            parsed.dual.default_panel.visible()[7].as_deref(),
            Some("this-pc")
        );
        assert!(
            parsed
                .dual
                .default_gestures
                .slots
                .iter()
                .all(Option::is_none)
        );
        assert!(parsed.dual.gesture_profiles.is_empty());
        let matched = AppKey::executable(Path::new(r"C:\Apps\Editor\Code.exe")).unwrap();
        let different = AppKey::executable(Path::new(r"C:\Other\Code.exe")).unwrap();
        let profile = parsed.dual.panel(Some(&matched)).unwrap();
        assert_eq!(profile.name, "VS Code");
        assert_eq!(profile.unknown["extension"], true);
        assert_eq!(profile.matrix.slots[4].as_deref(), Some("documents"));
        assert!(parsed.dual.panel(Some(&different)).is_none());
        assert_eq!(config.unknown[KEY], old);
    }

    #[test]
    fn saved_matrices_and_gestures_take_precedence_over_old_mode_switch() {
        let mut config = Config::default();
        let saved = json!({
            "enabled": true, "dual_enabled": false, "upper": {"count": 8}, "profiles": [],
            "dual": {
                "global": {"columns": 3, "slots": ["calculator"]},
                "default_gestures": {"slots": [null, null, null, null, null, null, "launcher", null]}
            }
        });
        config.unknown.insert(KEY.into(), saved.clone());
        let parsed = WorkspaceConfig::read_enabled(&config).unwrap();
        assert_eq!(parsed.dual.global.columns, 3);
        assert_eq!(
            parsed.dual.global.visible()[0].as_deref(),
            Some("calculator")
        );
        assert_eq!(
            parsed.dual.default_gestures.get(Direction::North),
            Some("launcher")
        );
        let persisted = crate::core::defaults(&parsed).unwrap();
        assert!(
            OBSOLETE_FIELDS
                .iter()
                .all(|field| persisted.get(field).is_none())
        );
        assert_eq!(config.unknown[KEY], saved);
    }

    #[test]
    fn absent_invalid_and_future_objects_leave_launcher_data_readable() {
        let mut config = Config::default();
        assert!(!WorkspaceConfig::read(&config).unwrap().enabled);
        assert!(WorkspaceConfig::read_enabled(&config).is_none());
        config.unknown.insert(KEY.into(), json!({"dual": {}}));
        assert!(!WorkspaceConfig::read(&config).unwrap().enabled);
        assert!(WorkspaceConfig::read_enabled(&config).is_none());
        for raw in [
            json!(false),
            json!({"enabled": true, "hotkey": "space"}),
            json!({"version": 22, "other": 8}),
            json!({"enabled": true, "upper": {"count": 3}}),
            json!({"enabled": true, "dual": {"global": {"columns": 6}}}),
        ] {
            config.unknown.insert(KEY.into(), raw.clone());
            assert!(WorkspaceConfig::read(&config).is_err());
            assert!(WorkspaceConfig::read_enabled(&config).is_none());
            assert_eq!(config.unknown[KEY], raw);
        }
    }
}
