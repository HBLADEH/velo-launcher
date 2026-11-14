//! Immutable business snapshots and synchronous background data operations.

pub mod catalog;
pub mod config;
pub mod data;
pub mod history;
pub mod icons;
pub mod indexer;
pub mod launch;
pub mod library;
pub mod migration;
pub mod model;
pub mod normalize;
pub mod search;
pub mod service;
pub mod storage;
pub mod system_tools;
pub mod update;

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

pub type UnknownFields = BTreeMap<String, Value>;
pub type Result<T> = std::result::Result<T, DataError>;

pub struct DataError {
    pub operation: &'static str,
    source: Box<dyn Error + Send + Sync>,
}

impl fmt::Debug for DataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Rust's Result-returning main uses Debug. Sources such as tempfile's
        // PathError may contain a personal path; retain them only via source().
        f.debug_struct("DataError")
            .field("operation", &self.operation)
            .finish_non_exhaustive()
    }
}

impl DataError {
    pub(crate) fn new(
        operation: &'static str,
        source: impl Into<Box<dyn Error + Send + Sync>>,
    ) -> Self {
        Self {
            operation,
            source: source.into(),
        }
    }
}

impl fmt::Display for DataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Keep paths and user content out of the normal diagnostic message.
        write!(f, "{} failed", self.operation)
    }
}

impl Error for DataError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.source.as_ref())
    }
}

pub trait JsonDocument: Serialize + DeserializeOwned + Clone + Send + Sync + 'static {
    fn normalize(value: &mut Value) -> Result<()>;
    fn validate(&self) -> Result<()>;

    fn decode(bytes: &[u8]) -> Result<Self> {
        let mut value =
            serde_json::from_slice(bytes).map_err(|e| DataError::new("decode JSON", e))?;
        Self::normalize(&mut value)?;
        let document: Self = decode_fields(&value)?;
        document.validate()?;
        Ok(document)
    }
}

pub(crate) fn decode_fields<T: DeserializeOwned>(value: &Value) -> Result<T> {
    // Value emits u128 events for large unknown integers, which Serde's flatten
    // buffer cannot accept. JSON bytes preserve them through its number adapter.
    let bytes = serde_json::to_vec(value).map_err(|e| DataError::new("normalize JSON", e))?;
    serde_json::from_slice(&bytes).map_err(|e| DataError::new("decode fields", e))
}

// Go overlays structs onto defaults: scalar/struct null is ignored, whereas
// slice/map null remains null. Its JSON field matching also ignores ASCII case.
pub(crate) fn overlay(defaults: Value, input: &mut Value) {
    if input.is_null() && !defaults.is_array() && !defaults.is_null() {
        *input = defaults;
        return;
    }
    if let (Value::Object(defaults), Value::Object(fields)) = (defaults, input) {
        for (name, default) in defaults {
            let key = if fields.contains_key(&name) {
                Some(name.clone())
            } else {
                fields
                    .keys()
                    .find(|key| key.eq_ignore_ascii_case(&name))
                    .cloned()
            };
            if let Some(key) = key {
                if let Some(mut value) = fields.remove(&key) {
                    overlay(default, &mut value);
                    fields.insert(name, value);
                }
            } else {
                fields.insert(name, default);
            }
        }
    }
}

pub(crate) fn defaults<T: Serialize>(value: &T) -> Result<Value> {
    serde_json::to_value(value).map_err(|e| DataError::new("encode defaults", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_error_formatting_keeps_source_details_out_of_logs() {
        let detail = r"C:\synthetic-personal-directory\query-content";
        let error = DataError::new("read data file", std::io::Error::other(detail));
        assert!(!format!("{error:?}").contains(detail));
        assert_eq!(error.to_string(), "read data file failed");
        assert_eq!(error.source().unwrap().to_string(), detail);
    }
}
