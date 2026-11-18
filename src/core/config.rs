use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{DataError, JsonDocument, Result, UnknownFields, defaults, overlay};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Search {
    pub fuzzy: bool,
    pub history_weight: f64,
    #[serde(flatten)]
    pub unknown: UnknownFields,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Everything {
    pub enabled: bool,
    pub executable: String,
    #[serde(flatten)]
    pub unknown: UnknownFields,
}

impl Default for Search {
    fn default() -> Self {
        Self {
            fuzzy: true,
            history_weight: 1.0,
            unknown: UnknownFields::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub version: i64,
    pub hotkey: String,
    pub disable_hotkey_fullscreen: bool,
    pub max_results: i64,
    pub theme: String,
    pub result_layout: String,
    pub launch_at_startup: bool,
    pub space_launch: bool,
    pub filter_noise: bool,
    pub search: Search,
    pub everything: Everything,
    pub custom_directories: Option<Vec<String>>,
    pub scan_program_files: bool,
    pub refresh_minutes: i64,
    pub auto_check_updates: bool,
    #[serde(flatten)]
    pub unknown: UnknownFields,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: 1,
            hotkey: "Alt+Space".into(),
            disable_hotkey_fullscreen: false,
            max_results: 8,
            theme: "system".into(),
            result_layout: "list".into(),
            launch_at_startup: false,
            space_launch: true,
            filter_noise: true,
            search: Search::default(),
            everything: Everything::default(),
            custom_directories: Some(Vec::new()),
            scan_program_files: true,
            refresh_minutes: 30,
            auto_check_updates: true,
            unknown: UnknownFields::new(),
        }
    }
}

impl JsonDocument for Config {
    fn normalize(value: &mut Value) -> Result<()> {
        overlay(defaults(&Self::default())?, value);
        Ok(())
    }

    fn validate(&self) -> Result<()> {
        let message = if self.version != 1 {
            Some("unsupported config version")
        } else if self.hotkey.trim().is_empty() {
            Some("empty hotkey")
        } else if !(1..=20).contains(&self.max_results) {
            Some("max_results outside 1..20")
        } else if !["system", "light", "dark"].contains(&self.theme.as_str()) {
            Some("invalid theme")
        } else if !["list", "grid"].contains(&self.result_layout.as_str()) {
            Some("invalid result_layout")
        } else if !(0.0..=5.0).contains(&self.search.history_weight) {
            Some("history_weight outside 0..5")
        } else if !(1..=1440).contains(&self.refresh_minutes) {
            Some("refresh_minutes outside 1..1440")
        } else if self
            .custom_directories
            .iter()
            .flatten()
            .any(|dir| !Path::new(dir).is_absolute())
        {
            Some("custom directory is not absolute")
        } else if !self.everything.executable.is_empty()
            && (!Path::new(&self.everything.executable).is_absolute()
                || !Path::new(&self.everything.executable)
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("exe")))
        {
            Some("Everything executable path must be an absolute .exe")
        } else {
            None
        };
        match message {
            Some(message) => Err(DataError::new("validate config", message)),
            None => Ok(()),
        }
    }
}
