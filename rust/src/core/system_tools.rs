//! Catalogue data is checked against Go's private definitions by its regression
//! test. Only locally available commands enter the searchable snapshot.
use super::{DataError, Result, model::AppItem};
use serde::Deserialize;
use std::path::Path;

#[derive(Deserialize)]
struct Definition {
    id: String,
    name: String,
    target: String,
    args: String,
    description: String,
    keywords: Vec<String>,
    required: String,
    min_build: u32,
}
fn definitions() -> Result<Vec<Definition>> {
    serde_json::from_slice(include_bytes!("system_tools.json"))
        .map_err(|e| DataError::new("read system catalogue", e))
}
pub struct Environment<'a> {
    pub windows: &'a Path,
    pub build: u32,
    pub settings: bool,
    pub policies: &'a [String],
}
pub fn discover(env: &Environment<'_>, exists: impl Fn(&Path) -> bool) -> Result<Vec<AppItem>> {
    let mut items = Vec::new();
    for d in definitions()? {
        let mut args = d.args;
        let path = if d.target.starts_with("velo-action:") {
            if !exists(&env.windows.join("System32/shell32.dll")) {
                continue;
            }
            d.target
        } else if let Some(page) = d.target.strip_prefix("ms-settings:") {
            if !env.settings
                || env.build < d.min_build
                || (d.id == "shared" && env.build >= 22000)
                || !page_visible(page, env.policies)
            {
                continue;
            }
            d.target
        } else {
            let path = env.windows.join(d.target.replace('/', "\\"));
            if !exists(&path) {
                continue;
            }
            if !d.required.is_empty() {
                let required = env.windows.join(d.required.replace('/', "\\"));
                if !exists(&required) {
                    continue;
                }
                if required.extension().is_some_and(|e| e == "msc") {
                    args = format!("\"{}\"", required.display());
                }
            }
            path.to_string_lossy().into_owned()
        };
        items.push(AppItem {
            id: format!("system:{}", d.id),
            name: d.name,
            exec_path: path.clone(),
            path,
            arguments: args,
            working_directory: env.windows.join("System32").to_string_lossy().into_owned(),
            source: "System".into(),
            description: d.description,
            keywords: Some(d.keywords),
            ..AppItem::default()
        });
    }
    Ok(items)
}
pub fn page_visible(page: &str, policies: &[String]) -> bool {
    policies.iter().all(|policy| {
        let policy = policy.trim().to_lowercase();
        let Some((mode, list)) = policy.split_once(':') else {
            return true;
        };
        let found = list.split(';').any(|p| p.trim() == page);
        !((mode == "hide" && found) || (mode == "showonly" && !found))
    })
}
pub fn special(item: &AppItem) -> bool {
    item.source == "System"
        && item.arguments.is_empty()
        && item.path == item.exec_path
        && definitions().is_ok_and(|defs| {
            defs.iter().any(|d| {
                item.id == format!("system:{}", d.id)
                    && item.path == d.target
                    && (d.target.starts_with("ms-settings:")
                        || d.target.starts_with("velo-action:"))
            })
        })
}
