use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::config::Config;
use super::model::AppItem;
use super::storage::{self, Store};
use super::{DataError, JsonDocument, Result, UnknownFields, decode_fields, defaults, overlay};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ApplicationLibrary {
    pub version: i64,
    pub applications: Option<Vec<AppItem>>,
    pub pinned: Option<Vec<AppItem>>,
    #[serde(flatten)]
    pub unknown: UnknownFields,
}

impl ApplicationLibrary {
    pub fn empty() -> Self {
        Self {
            version: 1,
            ..Self::default()
        }
    }

    pub fn migrate(legacy: Option<Vec<AppItem>>) -> Self {
        let mut applications = Vec::new();
        let pinned = legacy.map(|mut items| {
            for item in &mut items {
                if item.source == "Quick Launch" {
                    let mut custom = item.clone();
                    custom.source = "Manual".into();
                    custom.pinned = false;
                    applications.push(custom);
                    item.source = "Manual".into();
                }
            }
            items
        });
        Self {
            version: 1,
            applications: (!applications.is_empty()).then_some(applications),
            pinned,
            unknown: UnknownFields::new(),
        }
    }
}

fn normalize_items(value: &mut Value) -> Result<()> {
    if let Some(items) = value.as_array_mut() {
        for item in items {
            overlay(defaults(&AppItem::default())?, item);
        }
    }
    Ok(())
}

impl JsonDocument for ApplicationLibrary {
    fn normalize(value: &mut Value) -> Result<()> {
        overlay(defaults(&Self::default())?, value);
        for key in ["applications", "pinned"] {
            if let Some(items) = value.get_mut(key) {
                normalize_items(items)?;
            }
        }
        Ok(())
    }

    fn validate(&self) -> Result<()> {
        if self.version != 1 {
            return Err(DataError::new(
                "validate application library",
                "unsupported version",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IndexedFile {
    pub modified: i64,
    pub size: i64,
    pub item: AppItem,
    #[serde(flatten)]
    pub unknown: UnknownFields,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IndexCache {
    pub version: i64,
    pub files: Option<BTreeMap<String, IndexedFile>>,
    pub apps: Option<Vec<AppItem>>,
    #[serde(flatten)]
    pub unknown: UnknownFields,
}

impl JsonDocument for IndexCache {
    fn normalize(value: &mut Value) -> Result<()> {
        overlay(defaults(&Self::default())?, value);
        if let Some(files) = value.get_mut("files").and_then(Value::as_object_mut) {
            for file in files.values_mut() {
                overlay(defaults(&IndexedFile::default())?, file);
            }
        }
        if let Some(items) = value.get_mut("apps") {
            normalize_items(items)?;
        }
        Ok(())
    }

    fn validate(&self) -> Result<()> {
        if self.version != 1 {
            return Err(DataError::new(
                "validate index cache",
                "unsupported version",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HistoryEntry {
    pub launch_count: i64,
    // Keep the original RFC3339 string: conversion to UTC or milliseconds would
    // lose Go's timezone spelling and nanosecond precision on a round trip.
    pub last_launch_time: String,
    pub query_history: Option<BTreeMap<String, i64>>,
    #[serde(flatten)]
    pub unknown: UnknownFields,
}

impl Default for HistoryEntry {
    fn default() -> Self {
        Self {
            launch_count: 0,
            last_launch_time: "0001-01-01T00:00:00Z".into(),
            query_history: None,
            unknown: UnknownFields::new(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct History(pub BTreeMap<String, HistoryEntry>);

impl JsonDocument for History {
    fn normalize(value: &mut Value) -> Result<()> {
        if value.is_null() {
            *value = serde_json::json!({});
        }
        if let Some(entries) = value.as_object_mut() {
            for entry in entries.values_mut() {
                overlay(defaults(&HistoryEntry::default())?, entry);
                if let Some(queries) = entry
                    .get_mut("query_history")
                    .and_then(Value::as_object_mut)
                {
                    for count in queries.values_mut() {
                        if count.is_null() {
                            *count = Value::from(0);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn validate(&self) -> Result<()> {
        for entry in self.0.values() {
            chrono::DateTime::parse_from_rfc3339(&entry.last_launch_time)
                .map_err(|e| DataError::new("validate history timestamp", e))?;
        }
        Ok(())
    }
}

pub enum IndexLoad {
    Missing,
    Ready(IndexCache),
    RebuildRequired(DataError),
}

pub enum ConfigLoad {
    Existing,
    Created,
    Recovered { original: std::path::PathBuf },
}

/// The caller must hold the instance/data-directory guard. This layer does not
/// choose a product directory, launch a worker, scan files, or touch the UI.
pub struct DataStore {
    pub config: Store<Config>,
    pub history: Store<History>,
    pub library: Store<ApplicationLibrary>,
    pub config_load: ConfigLoad,
    pub index: IndexLoad,
}

fn optional_bytes(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(DataError::new("read data file", error)),
    }
}

impl DataStore {
    pub fn open(directory: &Path) -> Result<Self> {
        let config_path = directory.join("config.json");
        let (config, config_load) = match optional_bytes(&config_path)? {
            None => {
                let config = Config::default();
                storage::write(&config_path, &config)?;
                (config, ConfigLoad::Created)
            }
            Some(bytes) => match Config::decode(&bytes) {
                Ok(config) => (config, ConfigLoad::Existing),
                Err(error) => {
                    let future = serde_json::from_slice::<Value>(&bytes)
                        .ok()
                        .and_then(|value| {
                            value
                                .as_object()?
                                .iter()
                                .find(|(key, _)| key.eq_ignore_ascii_case("version"))
                                .and_then(|(_, version)| version.as_f64())
                        })
                        .is_some_and(|version| version > 1.0);
                    if future {
                        return Err(error);
                    }
                    // A durable copy precedes recovery; a failed replacement leaves
                    // both the original path and the preserved bytes available.
                    let original =
                        storage::preserve_bytes(directory, "config.json.invalid", &bytes)?;
                    let config = Config::default();
                    storage::write(&config_path, &config)?;
                    (config, ConfigLoad::Recovered { original })
                }
            },
        };
        let history_path = directory.join("history.json");
        let history = optional_bytes(&history_path)?
            .map(|bytes| History::decode(&bytes))
            .transpose()?
            .unwrap_or_default();
        let library_path = directory.join("application-library.json");
        let library = match optional_bytes(&library_path)? {
            Some(bytes) => ApplicationLibrary::decode(&bytes)?,
            None => match optional_bytes(&directory.join("quick-launch.json"))? {
                None => ApplicationLibrary::empty(),
                Some(bytes) => {
                    let mut value: Value = serde_json::from_slice(&bytes)
                        .map_err(|e| DataError::new("decode legacy library", e))?;
                    normalize_items(&mut value)?;
                    let items = decode_fields(&value)?;
                    let library = ApplicationLibrary::migrate(items);
                    storage::write(&library_path, &library)?;
                    library
                }
            },
        };
        let index = match optional_bytes(&directory.join("index.json")) {
            Ok(None) => IndexLoad::Missing,
            Ok(Some(bytes)) => match IndexCache::decode(&bytes) {
                Ok(cache) => IndexLoad::Ready(cache),
                Err(error) => IndexLoad::RebuildRequired(error),
            },
            Err(error) => IndexLoad::RebuildRequired(error),
        };
        Ok(Self {
            config: Store::from_loaded(config_path, config),
            history: Store::from_loaded(history_path, history),
            library: Store::from_loaded(library_path, library),
            config_load,
            index,
        })
    }
}
