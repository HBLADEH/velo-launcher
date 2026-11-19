use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::UnknownFields;
use std::path::{Path, PathBuf};

pub const FOLDER_SOURCE: &str = "Folder";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppItem {
    pub pinned: bool,
    pub id: String,
    pub name: String,
    pub path: String,
    pub exec_path: String,
    pub arguments: String,
    pub working_directory: String,
    pub icon_path: String,
    pub icon_url: String,
    pub description: String,
    pub source: String,
    pub keywords: Option<Vec<String>>,
    #[serde(flatten)]
    pub unknown: UnknownFields,
}

impl AppItem {
    pub fn is_folder(&self) -> bool {
        self.source == FOLDER_SOURCE
    }
}

pub fn identity(path: &str, arguments: &str) -> String {
    // Go's strings.ToLower uses one code point per input character. Applying
    // Rust's whole-string lowercasing changes final sigma and expands U+0130.
    let path: String = path
        .chars()
        .map(|c| c.to_lowercase().next().unwrap_or(c))
        .collect();
    let mut hash = Sha256::new();
    hash.update(path.as_bytes());
    hash.update([0]);
    hash.update(arguments.as_bytes());
    hash.finalize()[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Resolve only the old generated cache names, without a local HTTP server.
/// Existence/decoding belongs to the later background icon loader.
pub fn cached_icon_path(directory: &Path, icon_url: &str) -> Option<PathBuf> {
    let name = icon_url.strip_prefix("/icons/")?;
    let hash = name.strip_suffix(".png")?;
    if hash.len() != 32
        || !hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return None;
    }
    Some(directory.join("cache/icons").join(name))
}
