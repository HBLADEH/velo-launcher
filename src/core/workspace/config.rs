use std::path::Path;

use serde::{Deserialize, Serialize};

use super::actions::{Template, WorkspaceAction};
use super::radial::{SlotId, Zone};
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

    fn normalize(self) -> Option<Self> {
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Layout {
    pub count: u8,
    pub slots: [Option<WorkspaceAction>; 8],
    #[serde(flatten)]
    pub unknown: UnknownFields,
}

impl Default for Layout {
    fn default() -> Self {
        use WorkspaceAction::*;
        Self::from_actions(4, [Some(Downloads), Some(Documents), Some(Pictures), None])
    }
}

impl Layout {
    pub fn upper() -> Self {
        use WorkspaceAction::*;
        Self::from_actions(
            6,
            [
                Some(Explorer),
                Some(Calculator),
                Some(TaskManager),
                None,
                Some(Launcher),
                Some(ControlPanel),
            ],
        )
    }

    pub fn template(template: Template) -> Self {
        use WorkspaceAction::*;
        let actions = match template {
            Template::Explorer => [Some(Downloads), Some(Documents), Some(ThisPc), None],
            Template::Code => [Some(CodeTerminal), Some(CodeCommands), Some(CodeFind), None],
            Template::Edge => [
                Some(EdgeNewTab),
                Some(EdgeAddress),
                Some(EdgeBookmarks),
                None,
            ],
        };
        Self::from_actions(4, actions)
    }

    fn from_actions<const N: usize>(count: u8, actions: [Option<WorkspaceAction>; N]) -> Self {
        let mut slots = [None; 8];
        slots[..N].copy_from_slice(&actions);
        Self {
            count,
            slots,
            unknown: UnknownFields::new(),
        }
    }

    fn valid(&self) -> bool {
        matches!(self.count, 4 | 6 | 8)
    }

    pub fn action(&self, index: u8) -> Option<WorkspaceAction> {
        (index < self.count)
            .then(|| self.slots.get(usize::from(index)).copied().flatten())
            .flatten()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub app: AppKey,
    pub name: String,
    pub glyph: String,
    #[serde(default)]
    pub template: Option<Template>,
    pub layout: Layout,
    #[serde(flatten)]
    pub unknown: UnknownFields,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkspaceConfig {
    pub version: u32,
    pub enabled: bool,
    pub hotkey: String,
    pub mouse: Option<MouseButton>,
    pub upper: Layout,
    pub default: Layout,
    pub profiles: Vec<Profile>,
    pub excluded: Vec<AppKey>,
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
            upper: Layout::upper(),
            default: Layout::default(),
            profiles: Vec::new(),
            excluded: Vec::new(),
            unknown: UnknownFields::new(),
        }
    }
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
        // Nested missing fields use the appropriate zone defaults.
        if let Some(upper) = raw.get("upper") {
            let mut upper = upper.clone();
            crate::core::overlay(crate::core::defaults(&Layout::upper())?, &mut upper);
            value.upper = crate::core::decode_fields(&upper)?;
        }
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&mut self) -> Result<()> {
        use std::collections::HashSet;
        let invalid = |reason| DataError::new("validate Workspace configuration", reason);
        if self.version != 1 || !self.upper.valid() || !self.default.valid() {
            return Err(invalid("unsupported version or slot preset"));
        }
        self.hotkey = Key::parse(&self.hotkey)
            .map_err(|_| invalid("invalid Workspace hotkey"))?
            .to_string();
        let mut seen = HashSet::new();
        for profile in &mut self.profiles {
            profile.app = profile
                .app
                .clone()
                .normalize()
                .ok_or_else(|| invalid("invalid application identity"))?;
            if profile.name.trim().is_empty()
                || !profile.layout.valid()
                || !seen.insert(profile.app.clone())
            {
                return Err(invalid("invalid or duplicate application profile"));
            }
        }
        for app in &mut self.excluded {
            *app = app
                .clone()
                .normalize()
                .ok_or_else(|| invalid("invalid excluded application"))?;
        }
        Ok(())
    }

    pub fn profile(&self, app: Option<&AppKey>) -> Option<&Profile> {
        let app = app?;
        self.profiles.iter().find(|profile| &profile.app == app)
    }
}

#[derive(Clone)]
pub struct Layouts {
    pub upper: Layout,
    pub lower: Layout,
    pub context: Option<AppKey>,
    pub scope: String,
    pub glyph: String,
    pub template: Option<Template>,
}

impl Layouts {
    pub fn resolve(config: &WorkspaceConfig, app: Option<&AppKey>) -> Self {
        let profile = config.profile(app);
        Self {
            upper: config.upper.clone(),
            lower: profile.map_or_else(|| config.default.clone(), |profile| profile.layout.clone()),
            context: app.cloned(),
            scope: profile.map_or("默认工作台", |profile| &profile.name).into(),
            glyph: profile.map_or("apps", |profile| &profile.glyph).into(),
            template: profile.and_then(|profile| profile.template),
        }
    }

    pub fn action(&self, slot: SlotId) -> Option<WorkspaceAction> {
        match slot.zone {
            Zone::Upper => self.upper.action(slot.index),
            Zone::Lower => self.lower.action(slot.index),
        }
    }

    pub fn available(&self, slot: SlotId) -> bool {
        self.action(slot).is_some_and(|action| {
            action.template().is_none_or(|template| {
                slot.zone == Zone::Lower
                    && self.context.is_some()
                    && self.template == Some(template)
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::Config;

    #[test]
    fn exact_full_path_matches_and_unsaved_presets_keep_slots() {
        let mut config = WorkspaceConfig::default();
        let matched = AppKey::executable(Path::new(r"C:\Apps\Editor\Code.exe")).unwrap();
        let different = AppKey::executable(Path::new(r"C:\Other\Code.exe")).unwrap();
        config.profiles.push(Profile {
            app: matched.clone(),
            name: "VS Code".into(),
            glyph: "code".into(),
            template: Some(Template::Code),
            layout: Layout::template(Template::Code),
            unknown: UnknownFields::new(),
        });
        assert_eq!(Layouts::resolve(&config, Some(&matched)).scope, "VS Code");
        assert_eq!(
            Layouts::resolve(&config, Some(&different)).scope,
            "默认工作台"
        );
        config.upper.count = 4;
        assert!(
            Layouts::resolve(&config, None)
                .action(SlotId {
                    zone: Zone::Upper,
                    index: 4
                })
                .is_none()
        );
        config.upper.count = 6;
        assert_eq!(
            Layouts::resolve(&config, None).action(SlotId {
                zone: Zone::Upper,
                index: 4
            }),
            Some(WorkspaceAction::Launcher)
        );
    }

    #[test]
    fn absent_invalid_and_future_objects_leave_launcher_data_readable() {
        let mut config = Config::default();
        assert!(!WorkspaceConfig::read(&config).unwrap().enabled);
        assert!(WorkspaceConfig::read_enabled(&config).is_none());
        for raw in [
            serde_json::json!(false),
            serde_json::json!({"enabled": true, "hotkey": "space"}),
            serde_json::json!({"version": 22, "other": 8}),
        ] {
            config.unknown.insert(KEY.into(), raw.clone());
            assert!(WorkspaceConfig::read(&config).is_err());
            assert!(WorkspaceConfig::read_enabled(&config).is_none());
            assert_eq!(config.unknown[KEY], raw);
        }
        config
            .unknown
            .insert(KEY.into(), serde_json::json!({"upper":{"future":42}}));
        let parsed = WorkspaceConfig::read(&config).unwrap();
        assert_eq!(parsed.upper.count, 6);
        assert_eq!(parsed.upper.unknown["future"], 42);
        assert!(WorkspaceConfig::read_enabled(&config).is_none());

        config.unknown.insert(
            KEY.into(),
            serde_json::json!({"enabled": true, "mouse": "xbutton2"}),
        );
        assert_eq!(
            WorkspaceConfig::read_enabled(&config).unwrap().mouse,
            Some(MouseButton::XButton2),
        );
    }
}
