//! Launch-group data and editor drafts. Members reference the application
//! catalogue; dragging never launches an app or creates a separate entry.

use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::indexer;
use super::model::AppItem;
use super::normalize::normalize;
use super::storage::Store;
use super::{DataError, JsonDocument, Result, UnknownFields};

pub const MAX_GROUPS: usize = 50;
pub const MAX_MEMBERS: usize = 20;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Detection {
    #[default]
    Auto,
    Executable,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RunningRule {
    pub kind: Detection,
    pub path: String,
    #[serde(flatten)]
    pub unknown: UnknownFields,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Member {
    pub app_id: String,
    pub display_name: String,
    pub running_rule: RunningRule,
    #[serde(flatten)]
    pub unknown: UnknownFields,
}

impl Member {
    pub fn from_app(item: &AppItem) -> Result<Self> {
        validate_entry(item)?;
        let member = Self {
            app_id: item.id.clone(),
            display_name: item.name.clone(),
            ..Self::default()
        };
        member.validate()?;
        Ok(member)
    }

    fn validate(&self) -> Result<()> {
        if self.app_id.is_empty() || self.display_name.trim().is_empty() {
            return Err(invalid("member identity and name required"));
        }
        if self.running_rule.kind == Detection::Executable
            && !executable_path(&self.running_rule.path)
        {
            return Err(invalid("absolute detection executable required"));
        }
        Ok(())
    }

    /// Returns the actual process to check, never the Shell host of a command.
    /// Parameterized entries need an explicit rule: a running host cannot prove
    /// that a particular game, document or profile has already opened.
    pub fn running_program(&self, item: &AppItem) -> Option<PathBuf> {
        if item.id != self.app_id {
            return None;
        }
        let path = match self.running_rule.kind {
            Detection::Executable => &self.running_rule.path,
            Detection::Auto if item.arguments.is_empty() => target(item),
            Detection::Auto => return None,
        };
        if !executable_path(path) || generic_host(path) {
            return None;
        }
        Some(PathBuf::from(path))
    }
}

fn target(item: &AppItem) -> &str {
    if item.exec_path.is_empty() {
        &item.path
    } else {
        &item.exec_path
    }
}

fn executable_path(path: &str) -> bool {
    Path::new(path).is_absolute() && indexer::extension(path, "exe") && !path.contains('\0')
}

fn generic_host(path: &str) -> bool {
    Path::new(path).file_name().is_some_and(|name| {
        [
            "explorer.exe",
            "cmd.exe",
            "powershell.exe",
            "pwsh.exe",
            "rundll32.exe",
            "wscript.exe",
            "cscript.exe",
            "mshta.exe",
        ]
        .iter()
        .any(|host| name.to_string_lossy().eq_ignore_ascii_case(host))
    })
}

fn validate_entry(item: &AppItem) -> Result<()> {
    if item.id.is_empty()
        || item.source == "System"
        || item.is_folder()
        || !Path::new(&item.path).is_absolute()
        || !executable_path(target(item))
        || !(indexer::extension(&item.path, "exe") || indexer::extension(&item.path, "lnk"))
        || item.path.contains('\0')
    {
        return Err(invalid("unsupported local application"));
    }
    Ok(())
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub aliases: Vec<String>,
    pub members: Vec<Member>,
    #[serde(flatten)]
    pub unknown: UnknownFields,
}

impl Group {
    fn validate(&self) -> Result<()> {
        if !self.id.starts_with("group:")
            || self.id.len() <= 6
            || self.id.len() > 80
            || !self.id[6..]
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err(invalid("invalid group identity"));
        }
        if self.name.trim().is_empty()
            || self.name.chars().count() > 32
            || self.name.contains('\0')
            || self.aliases.len() > 8
            || self.aliases.iter().any(|alias| {
                alias.trim().is_empty() || alias.chars().count() > 32 || alias.contains('\0')
            })
        {
            return Err(invalid("invalid group name or aliases"));
        }
        if !(2..=MAX_MEMBERS).contains(&self.members.len()) {
            return Err(invalid("groups require two to twenty members"));
        }
        let mut ids = BTreeSet::new();
        for member in &self.members {
            member.validate()?;
            if !ids.insert(&member.app_id) {
                return Err(invalid("duplicate group member"));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Collection {
    pub version: u32,
    pub groups: Vec<Group>,
    #[serde(flatten)]
    pub unknown: UnknownFields,
}

impl Default for Collection {
    fn default() -> Self {
        Self {
            version: 1,
            groups: Vec::new(),
            unknown: UnknownFields::new(),
        }
    }
}

impl JsonDocument for Collection {
    fn normalize(_value: &mut Value) -> Result<()> {
        // This is a new format, not a Go contract. Serde defaults only fill
        // absent fields; explicit nulls and invalid types must stay errors.
        Ok(())
    }

    fn validate(&self) -> Result<()> {
        if self.version != 1 || self.groups.len() > MAX_GROUPS {
            return Err(invalid("unsupported version or group limit exceeded"));
        }
        let mut ids = BTreeSet::new();
        let mut names = BTreeSet::new();
        for group in &self.groups {
            group.validate()?;
            if !ids.insert(&group.id) {
                return Err(invalid("duplicate group identity"));
            }
            for name in std::iter::once(&group.name).chain(&group.aliases) {
                if !names.insert(normalize(name)) {
                    return Err(invalid("duplicate group name or alias"));
                }
            }
        }
        Ok(())
    }
}

impl Collection {
    pub fn with_group(&self, group: Group) -> Result<Self> {
        let mut next = self.clone();
        if let Some(existing) = next.groups.iter_mut().find(|old| old.id == group.id) {
            *existing = group;
        } else {
            next.groups.push(group);
        }
        next.validate()?;
        Ok(next)
    }

    pub fn without_group(&self, id: &str) -> Result<Self> {
        let mut next = self.clone();
        next.groups.retain(|group| group.id != id);
        next.validate()?;
        Ok(next)
    }
}

/// Failure only disables this optional feature. The caller can retain the error
/// and continue loading the normal application catalogue; no source is rewritten.
pub fn open(directory: &Path) -> Result<Store<Collection>> {
    let path = directory.join("launch-groups.json");
    let collection = match std::fs::read(&path) {
        Ok(bytes) => Collection::decode(&bytes)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => Collection::default(),
        Err(error) => return Err(DataError::new("read launch groups", error)),
    };
    Ok(Store::from_loaded(path, collection))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DraftTicket(u64);

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Merge {
    pub added: usize,
    pub duplicate: usize,
    pub overflow: usize,
    pub stale: bool,
}

/// One editor owns its generation across group switches. Recreating a draft
/// with generation zero could accept another group's delayed file inspection.
#[derive(Default)]
pub struct Draft {
    generation: u64,
    group: Option<Group>,
    baseline: Option<Group>,
}

impl Draft {
    pub fn open(&mut self, group: Group) {
        self.generation = self.generation.wrapping_add(1);
        self.baseline = Some(group.clone());
        self.group = Some(group);
    }

    pub fn close(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.group = None;
        self.baseline = None;
    }

    pub fn group(&self) -> Option<&Group> {
        self.group.as_ref()
    }

    pub fn ticket(&self) -> DraftTicket {
        DraftTicket(self.generation)
    }

    pub fn dirty(&self) -> bool {
        self.group != self.baseline
    }

    pub fn rename(&mut self, name: String) {
        if let Some(group) = &mut self.group {
            group.name = name;
        }
    }

    pub fn merge(&mut self, ticket: DraftTicket, members: Vec<Member>) -> Merge {
        let mut result = Merge::default();
        if ticket != self.ticket() || self.group.is_none() {
            result.stale = true;
            return result;
        }
        if let Some(group) = &mut self.group {
            for member in members {
                if group.members.iter().any(|old| old.app_id == member.app_id) {
                    result.duplicate += 1;
                } else if group.members.len() == MAX_MEMBERS {
                    result.overflow += 1;
                } else {
                    group.members.push(member);
                    result.added += 1;
                }
            }
        }
        result
    }

    pub fn remove(&mut self, app_id: &str) {
        if let Some(group) = &mut self.group {
            group.members.retain(|member| member.app_id != app_id);
            // A file already queued for inspection must not re-add a removed
            // member. Other late files can be explicitly dragged again.
            self.generation = self.generation.wrapping_add(1);
        }
    }

    pub fn set_running_rule(&mut self, app_id: &str, mut rule: RunningRule) {
        if let Some(member) = self.group.as_mut().and_then(|group| {
            group
                .members
                .iter_mut()
                .find(|member| member.app_id == app_id)
        }) {
            for (key, value) in &member.running_rule.unknown {
                rule.unknown
                    .entry(key.clone())
                    .or_insert_with(|| value.clone());
            }
            member.running_rule = rule;
        }
    }

    /// Call only with the group returned by a successful durable commit.
    pub fn committed(&mut self, saved: &Group) {
        if self.group.as_ref() == Some(saved) {
            self.baseline = Some(saved.clone());
        }
    }
}

fn invalid(reason: &'static str) -> DataError {
    DataError::new("validate launch groups", reason)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::model::identity;

    fn app(name: &str) -> AppItem {
        let path = format!(r"C:\SyntheticApps\{name}.exe");
        AppItem {
            id: identity(&path, ""),
            name: name.into(),
            path: path.clone(),
            exec_path: path,
            ..AppItem::default()
        }
    }

    fn group() -> Group {
        Group {
            id: "group:synthetic".into(),
            name: "开黑".into(),
            members: ["游戏", "语音"]
                .map(|name| Member::from_app(&app(name)).unwrap())
                .into(),
            ..Group::default()
        }
    }

    #[test]
    fn drag_deduplicates_bounds_and_rejects_previous_group_and_removed_member_results() {
        let mut draft = Draft::default();
        draft.open(group());
        let ticket = draft.ticket();
        let voice = Member::from_app(&app("语音")).unwrap();
        let accelerator = Member::from_app(&app("加速器")).unwrap();
        let merged = draft.merge(ticket, vec![voice.clone(), accelerator]);
        assert_eq!((merged.added, merged.duplicate), (1, 1));
        assert!(draft.dirty());
        draft.remove(&voice.app_id);
        assert!(draft.merge(ticket, vec![voice.clone()]).stale);
        let removed_ticket = draft.ticket();
        let mut other = group();
        other.id = "group:other".into();
        draft.open(other);
        assert!(draft.merge(removed_ticket, vec![voice]).stale);
        let too_many = (0..MAX_MEMBERS)
            .map(|i| Member::from_app(&app(&format!("App{i}"))).unwrap())
            .collect();
        let merged = draft.merge(draft.ticket(), too_many);
        assert_eq!(merged.overflow, 2);
        assert_eq!(draft.group().unwrap().members.len(), MAX_MEMBERS);
        let ticket = draft.ticket();
        draft.close();
        assert!(draft.merge(ticket, Vec::new()).stale);
        assert!(!draft.dirty());
    }

    #[test]
    fn name_edits_and_failed_or_late_saves_preserve_draft_and_unknown_fields() {
        let mut draft = Draft::default();
        let mut initial = group();
        initial
            .unknown
            .insert("future".into(), serde_json::json!({"value": 7}));
        draft.open(initial.clone());
        draft.rename("游戏时间".into());
        draft.committed(&initial);
        assert!(draft.dirty());
        let saved = draft.group().unwrap().clone();
        draft.committed(&saved);
        assert!(!draft.dirty());
        assert_eq!(draft.group().unwrap().id, initial.id);
        assert_eq!(draft.group().unwrap().unknown, initial.unknown);
    }

    #[test]
    fn persistence_preserves_unknown_fields_and_does_not_create_an_unused_file() {
        let directory = tempfile::tempdir().unwrap();
        let store = open(directory.path()).unwrap();
        let path = directory.path().join("launch-groups.json");
        assert!(!path.exists());
        let group = group();
        let next = Collection::default().with_group(group).unwrap();
        let mut value = serde_json::to_value(&next).unwrap();
        value["future"] = serde_json::json!([1, 2, 3]);
        value["groups"][0]["future"] = serde_json::json!({"enabled": true});
        value["groups"][0]["members"][0]["future"] =
            serde_json::json!(123456789012345678901234567890u128);
        value["groups"][0]["members"][0]["running_rule"]["future"] = serde_json::json!("keep");
        let decoded = Collection::decode(&serde_json::to_vec(&value).unwrap()).unwrap();
        store.update(|_| Ok(decoded)).unwrap();
        let original = std::fs::read(&path).unwrap();
        let loaded = open(directory.path()).unwrap();
        let snapshot = loaded.snapshot().unwrap();
        let mut edited = snapshot.groups[0].clone();
        edited.name = "游戏时间".into();
        loaded.update(|current| current.with_group(edited)).unwrap();
        let changed: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(changed["future"], value["future"]);
        assert_eq!(
            changed["groups"][0]["members"],
            value["groups"][0]["members"]
        );
        assert_eq!(changed["groups"][0]["future"], value["groups"][0]["future"]);
        assert!(!original.is_empty());
    }

    #[test]
    fn damaged_future_and_invalid_documents_are_never_rewritten() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("launch-groups.json");
        for bytes in [
            b"{ invalid original\r\n".as_slice(),
            br#"{"version":2,"groups":[],"future":"keep"}"#,
            br#"{"version":1,"groups":null}"#,
        ] {
            std::fs::write(&path, bytes).unwrap();
            assert!(open(directory.path()).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
        }
    }

    #[test]
    fn failed_atomic_save_keeps_the_old_file_snapshot_and_dirty_draft() {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;

        let directory = tempfile::tempdir().unwrap();
        let store = open(directory.path()).unwrap();
        let initial = group();
        store
            .update(|current| current.with_group(initial.clone()))
            .unwrap();
        let path = directory.path().join("launch-groups.json");
        let original = std::fs::read(&path).unwrap();
        let before = store.snapshot().unwrap();
        let mut draft = Draft::default();
        draft.open(initial);
        draft.rename("新名称".into());
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .open(&path)
            .unwrap();
        let saved = draft.group().unwrap().clone();
        assert!(store.update(|current| current.with_group(saved)).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert_eq!(store.snapshot().unwrap(), before);
        assert!(draft.dirty());
        drop(lock);
        let saved = draft.group().unwrap().clone();
        store
            .update(|current| current.with_group(saved.clone()))
            .unwrap();
        draft.committed(&saved);
        assert!(!draft.dirty());
        assert_eq!(store.snapshot().unwrap().groups[0].name, "新名称");
    }

    #[test]
    fn catalogue_reference_keeps_a_legacy_id_without_creating_a_local_fallback() {
        let mut legacy = app("旧应用");
        legacy.id = "legacy-preserved-id".into();
        let member = Member::from_app(&legacy).unwrap();
        assert_eq!(member.app_id, legacy.id);
    }

    #[test]
    fn group_name_alias_and_member_validation_prevent_ambiguous_entries() {
        let first = group();
        let collection = Collection::default().with_group(first.clone()).unwrap();
        let mut second = first.clone();
        second.id = "group:second".into();
        second.name = "开发".into();
        second.aliases = vec![" 开黑 ".into()];
        assert!(collection.with_group(second).is_err());
        let mut duplicate = first.clone();
        duplicate.members.push(duplicate.members[0].clone());
        assert!(collection.with_group(duplicate).is_err());
        let mut one = first;
        one.members.pop();
        assert!(collection.with_group(one).is_err());
        assert!(
            Member::from_app(&AppItem {
                path: "velo-action:shutdown".into(),
                ..app("关机")
            })
            .is_err()
        );
    }

    #[test]
    fn parameterized_game_and_shell_hosts_need_explicit_detection() {
        let mut game = app("Steam");
        game.arguments = "-applaunch 123".into();
        game.id = identity(&game.exec_path, &game.arguments);
        let mut member = Member::from_app(&game).unwrap();
        assert!(member.running_program(&game).is_none());
        member.running_rule = RunningRule {
            kind: Detection::Executable,
            path: app("Game").path,
            ..RunningRule::default()
        };
        assert_eq!(
            member.running_program(&game),
            Some(PathBuf::from(r"C:\SyntheticApps\Game.exe"))
        );
        let host = app("explorer");
        assert!(
            Member::from_app(&host)
                .unwrap()
                .running_program(&host)
                .is_none()
        );
        let voice = app("语音");
        assert_eq!(
            Member::from_app(&voice).unwrap().running_program(&voice),
            Some(PathBuf::from(voice.path))
        );
    }
}
