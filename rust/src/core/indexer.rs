//! Go-compatible discovery rules. All filesystem work belongs on a worker.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::time::UNIX_EPOCH;

use super::data::{IndexCache, IndexedFile};
use super::model::{AppItem, identity};
use super::normalize::{lower, normalize, word};
use super::{DataError, Result};

#[derive(Clone, Debug)]
pub struct Root {
    pub path: PathBuf,
    pub source: String,
    /// Zero is unlimited. Keep walking deeper directories, like the Go baseline.
    pub max_depth: usize,
}

#[derive(Clone, Default)]
pub struct Generation(Arc<AtomicU64>);

#[derive(Clone)]
pub struct Ticket {
    generation: Generation,
    value: u64,
}

impl Generation {
    pub fn advance(&self) {
        self.0.fetch_add(1, Ordering::AcqRel);
    }
    pub fn ticket(&self) -> Ticket {
        Ticket {
            generation: self.clone(),
            value: self.0.load(Ordering::Acquire),
        }
    }
}

impl Ticket {
    pub fn current(&self) -> bool {
        self.generation.0.load(Ordering::Acquire) == self.value
    }
    pub fn check(&self) -> Result<()> {
        if self.current() {
            Ok(())
        } else {
            Err(DataError::new("scan cancelled", "obsolete generation"))
        }
    }
}

pub trait Resolver {
    fn resolve(&mut self, path: &Path) -> Result<AppItem>;
}

#[derive(Default, Debug)]
pub struct ScanStats {
    pub reused: usize,
    pub resolved: usize,
    pub warnings: usize,
}

pub fn path_key(path: &Path) -> String {
    path.to_string_lossy().chars().map(lower).collect()
}

pub fn extension(path: &str, extension: &str) -> bool {
    Path::new(path)
        .extension()
        .is_some_and(|s| s.eq_ignore_ascii_case(extension))
}

pub fn modified_ns(metadata: &fs::Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|t| match t.duration_since(UNIX_EPOCH) {
            Ok(duration) => i64::try_from(duration.as_nanos()).ok(),
            Err(error) => i64::try_from(error.duration().as_nanos()).ok().map(|n| -n),
        })
        .unwrap_or(-1)
}

fn desktop_link_valid(item: &AppItem) -> bool {
    item.source != "Desktop"
        || !extension(&item.path, "lnk")
        || (Path::new(&item.exec_path).is_absolute()
            && !item.path.eq_ignore_ascii_case(&item.exec_path)
            && fs::metadata(&item.exec_path).is_ok_and(|m| m.is_file()))
}

struct Scan<'a> {
    old: &'a IndexCache,
    files: BTreeMap<String, IndexedFile>,
    resolver: &'a mut dyn Resolver,
    ticket: &'a Ticket,
    stats: ScanStats,
}

impl Scan<'_> {
    fn failed_subtree(&mut self, path: &Path, error: &std::io::Error, root: bool) {
        if error.kind() == std::io::ErrorKind::NotFound {
            self.stats.warnings += usize::from(root && self.stats.warnings < 25);
            return;
        }
        self.stats.warnings += usize::from(self.stats.warnings < 25);
        let key = path_key(path);
        let prefix = format!("{key}\\");
        for (path, item) in self.old.files.iter().flatten() {
            if path == &key || path.starts_with(&prefix) {
                self.files.insert(path.clone(), item.clone());
            }
        }
    }

    fn walk(&mut self, root: &Root, path: &Path, depth: usize) -> Result<()> {
        self.ticket.check()?;
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) => {
                self.failed_subtree(path, &error, path == root.path);
                return Ok(());
            }
        };
        // Windows FileType also identifies junctions. Other reparse points (for
        // example cloud placeholders) remain files/directories, as in Go; the
        // reparse attribute alone is not a reason to discard an application.
        if metadata.file_type().is_symlink() {
            return Ok(());
        }
        if metadata.is_dir() {
            if root.source.starts_with("Program Files")
                && depth == 1
                && path
                    .file_name()
                    .is_some_and(|name| name.eq_ignore_ascii_case("WindowsApps"))
            {
                return Ok(());
            }
            let entries = match fs::read_dir(path) {
                Ok(entries) => entries,
                Err(error) => {
                    self.failed_subtree(path, &error, path == root.path);
                    return Ok(());
                }
            };
            let mut children = Vec::new();
            for entry in entries {
                self.ticket.check()?;
                match entry {
                    Ok(entry) => children.push(entry.path()),
                    Err(error) => self.failed_subtree(path, &error, false),
                }
            }
            children.sort();
            for child in children {
                self.walk(root, &child, depth + 1)?;
            }
            return Ok(());
        }
        if (!extension(&path.to_string_lossy(), "exe")
            && !extension(&path.to_string_lossy(), "lnk"))
            || (root.max_depth > 0 && depth.saturating_sub(1) > root.max_depth)
        {
            return Ok(());
        }
        let key = path_key(path);
        let modified = modified_ns(&metadata);
        let size =
            i64::try_from(metadata.len()).map_err(|e| DataError::new("read file size", e))?;
        if let Some(cached) = self.old.files.as_ref().and_then(|files| files.get(&key))
            && cached.modified == modified
            && cached.size == size
        {
            let mut cached = cached.clone();
            cached.item.source.clone_from(&root.source);
            if desktop_link_valid(&cached.item) {
                self.files.insert(key, cached);
                self.stats.reused += 1;
            }
            return Ok(());
        }
        let path_text = path.to_string_lossy().into_owned();
        let mut item = AppItem {
            name: path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            path: path_text.clone(),
            exec_path: path_text,
            source: root.source.clone(),
            keywords: Some(Vec::new()),
            ..AppItem::default()
        };
        let mut modified = modified;
        if extension(&item.path, "lnk") {
            self.stats.resolved += 1;
            match self.resolver.resolve(path) {
                Ok(mut resolved) => {
                    resolved.name = item.name;
                    resolved.path = item.path;
                    resolved.source = item.source;
                    item = resolved;
                }
                Err(_) => {
                    modified = -1;
                    self.stats.warnings += usize::from(self.stats.warnings < 25);
                }
            }
        }
        self.ticket.check()?;
        if desktop_link_valid(&item) {
            item.id = identity(
                if item.exec_path.is_empty() {
                    &item.path
                } else {
                    &item.exec_path
                },
                &item.arguments,
            );
            self.files.insert(
                key,
                IndexedFile {
                    modified,
                    size,
                    item,
                    ..IndexedFile::default()
                },
            );
        }
        Ok(())
    }
}

pub fn scan(
    roots: &[Root],
    old: &IndexCache,
    resolver: &mut dyn Resolver,
    ticket: &Ticket,
) -> Result<(IndexCache, ScanStats)> {
    let mut scan = Scan {
        old,
        files: BTreeMap::new(),
        resolver,
        ticket,
        stats: ScanStats::default(),
    };
    let mut seen = BTreeSet::new();
    for root in roots {
        let path =
            std::path::absolute(&root.path).map_err(|e| DataError::new("resolve scan root", e))?;
        if seen.insert(path_key(&path)) {
            scan.walk(
                &Root {
                    path: path.clone(),
                    ..root.clone()
                },
                &path,
                0,
            )?;
        }
    }
    ticket.check()?;
    let apps = deduplicate(
        scan.files
            .values()
            .filter(|f| desktop_link_valid(&f.item))
            .map(|f| f.item.clone())
            .collect(),
    );
    Ok((
        IndexCache {
            version: 1,
            files: Some(scan.files),
            apps: Some(apps),
            unknown: old.unknown.clone(),
        },
        scan.stats,
    ))
}

fn target(item: &AppItem) -> &str {
    if item.exec_path.is_empty() {
        &item.path
    } else {
        &item.exec_path
    }
}

fn priority(item: &AppItem) -> u8 {
    if item.source == "Windows Apps" {
        3
    } else if extension(&item.path, "lnk") {
        match item.source.as_str() {
            "Desktop" => 0,
            "Start Menu" => 1,
            _ => 2,
        }
    } else {
        4
    }
}

pub fn deduplicate(mut items: Vec<AppItem>) -> Vec<AppItem> {
    items.sort_by_cached_key(|item| {
        (
            priority(item),
            clean_depth(target(item)),
            path_key(Path::new(&item.path)),
            item.id.clone(),
        )
    });
    let mut ids = BTreeMap::<String, usize>::new();
    let mut names = BTreeSet::new();
    let mut out: Vec<AppItem> = Vec::new();
    for item in items {
        if let Some(&at) = ids.get(&item.id) {
            for alias in std::iter::once(item.name).chain(item.keywords.into_iter().flatten()) {
                if !alias.is_empty()
                    && alias != out[at].name
                    && !out[at].keywords.iter().flatten().any(|k| k == &alias)
                {
                    out[at].keywords.get_or_insert_default().push(alias);
                }
            }
            continue;
        }
        let key = (
            normalize(&item.name),
            path_key(Path::new(
                Path::new(target(&item)).file_name().unwrap_or_default(),
            )),
        );
        if names.insert(key) {
            ids.insert(item.id.clone(), out.len());
            out.push(item);
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
    out
}

fn clean_depth(path: &str) -> usize {
    let mut clean = PathBuf::new();
    for component in Path::new(path).components() {
        if component == std::path::Component::ParentDir
            && clean.file_name().is_some_and(|name| name != "..")
        {
            clean.pop();
        } else {
            clean.push(component.as_os_str());
        }
    }
    clean
        .to_string_lossy()
        .replace('/', "\\")
        .bytes()
        .filter(|b| *b == b'\\')
        .count()
}

pub fn is_noise(name: &str) -> bool {
    let name: String = name.trim().chars().map(lower).collect();
    if name
        .strip_prefix("unins")
        .is_some_and(|rest| rest.bytes().all(|b| b.is_ascii_digit()))
    {
        return true;
    }
    const TOKENS: &[&str] = &[
        "uninstall",
        "uninstaller",
        "uninst",
        "help",
        "readme",
        "documentation",
        "docs",
        "manual",
        "website",
        "homepage",
        "changelog",
        "license",
        "licence",
        "repair",
        "modify",
        "updater",
        "update",
        "install",
        "installer",
        "setup",
        "about",
        "support",
    ];
    [
        "release notes",
        "what's new",
        "read me",
        "help center",
        "卸载",
        "移除",
        "帮助",
        "说明",
        "自述",
        "文档",
        "手册",
        "网站",
        "官网",
        "主页",
        "在线帮助",
        "更新日志",
        "新特性",
        "发行说明",
        "许可",
        "修复",
        "更改",
        "更新",
        "安装",
        "关于",
    ]
    .iter()
    .any(|token| name.contains(token))
        || name
            .split(|c| !word(c))
            .any(|token| TOKENS.contains(&token))
}

pub fn visible(items: &[AppItem], filter: bool) -> Vec<AppItem> {
    deduplicate(
        items
            .iter()
            .filter(|item| {
                let component = ["Program Files", "Program Files (x86)", "Custom"]
                    .contains(&item.source.as_str())
                    && extension(&item.path, "exe")
                    && Path::new(&item.path).parent().is_some_and(|p| {
                        p.iter().any(|part| part.eq_ignore_ascii_case("Hypervisor"))
                    });
                !filter || (!is_noise(&item.name) && !component)
            })
            .cloned()
            .collect(),
    )
}
