//! Pre-normalized, immutable search snapshot. Querying never touches the file
//! system, converts pinyin, or copies the complete application catalogue.

use super::model::AppItem;
use super::normalize::{han, normalize, pronunciation, word};
use std::collections::BTreeMap;
use std::sync::Arc;

struct Entry {
    item: Arc<AppItem>,
    name: String,
    terms: Vec<String>,
    aliases: Vec<String>,
    priority: f64,
}

#[derive(Clone, Debug)]
pub struct SearchResult {
    pub item: Arc<AppItem>,
    pub score: f64,
}

pub struct Index {
    entries: Vec<Entry>,
}

impl Index {
    /// Build off the UI thread when the catalogue changes.
    pub fn new(items: impl IntoIterator<Item = Arc<AppItem>>) -> Self {
        Self {
            entries: items
                .into_iter()
                .map(|item| {
                    let name = normalize(&item.name);
                    let mut terms: Vec<_> = name
                        .split(|c| !word(c))
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned)
                        .collect();
                    let initials: String = terms.iter().filter_map(|s| s.chars().next()).collect();
                    terms.push(initials);
                    let mut aliases = pinyin_aliases(&name);
                    for keyword in item.keywords.iter().flatten() {
                        let keyword = normalize(keyword);
                        aliases.extend(pinyin_aliases(&keyword));
                        if !keyword.is_empty() {
                            aliases.push(keyword.clone());
                        }
                        terms.push(keyword);
                    }
                    if name.contains("wechat") || name.contains("微信") {
                        terms.extend(["wx", "weixin", "微信"].map(str::to_owned));
                        aliases.extend(["wx", "weixin", "wechat", "微信"].map(str::to_owned));
                    }
                    if name.contains("腾讯会议") {
                        aliases.extend(["wemeet", "tencent meeting"].map(str::to_owned));
                    }
                    if name == "visual studio code" {
                        aliases.extend(["vscode", "code"].map(str::to_owned));
                    }
                    let priority = priority(&item);
                    Entry {
                        item,
                        name,
                        terms,
                        aliases,
                        priority,
                    }
                })
                .collect(),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn query(
        &self,
        query: &str,
        limit: usize,
        fuzzy: bool,
        weights: &BTreeMap<String, f64>,
        weight: f64,
    ) -> Vec<SearchResult> {
        if limit == 0 {
            return Vec::new();
        }
        let query = normalize(query);
        let mut top: Vec<(&Entry, f64)> = Vec::with_capacity(limit.min(self.len()));
        for entry in &self.entries {
            let mut score = matching(&query, &entry.name, &entry.terms, fuzzy);
            if !query.is_empty() && score < 800 {
                for alias in &entry.aliases {
                    if alias.starts_with(&query) {
                        score = 800;
                        break;
                    }
                    if score < 500 && alias.contains(&query) {
                        score = 500;
                    }
                }
            }
            if score < 0 {
                continue;
            }
            let mut boost =
                entry.priority + weights.get(&entry.item.id).copied().unwrap_or(0.0) * weight;
            if !query.is_empty() {
                boost = boost.clamp(-180.0, 180.0);
            }
            if score == 1000 {
                score += 1000;
            }
            let score = score as f64 + boost;
            let position = top.partition_point(|(other, other_score)| {
                *other_score > score
                    || (*other_score == score
                        && (&other.name, &other.item.id) <= (&entry.name, &entry.item.id))
            });
            if position < limit {
                top.insert(position, (entry, score));
                if top.len() > limit {
                    top.pop();
                }
            }
        }
        top.into_iter()
            .map(|(entry, score)| SearchResult {
                item: Arc::clone(&entry.item),
                score,
            })
            .collect()
    }
}

fn pinyin_aliases(name: &str) -> Vec<String> {
    if !name.chars().any(han) {
        return Vec::new();
    }
    let parts: Vec<String> = name
        .chars()
        .map(|c| {
            pronunciation(c)
                .map(str::to_owned)
                .unwrap_or_else(|| c.to_string())
        })
        .collect();
    let initials: Vec<String> = parts
        .iter()
        .filter_map(|s| s.chars().next().map(|c| c.to_string()))
        .collect();
    let mut aliases = Vec::new();
    for (index, c) in name.chars().enumerate() {
        if han(c) {
            aliases.push(parts[index..].concat());
            aliases.push(initials[index..].concat());
        }
    }
    aliases
}

pub fn priority(item: &AppItem) -> f64 {
    let mut score = match item.source.as_str() {
        "Desktop" => 75.0,
        "Start Menu" => 55.0,
        "Windows Apps" => 40.0,
        "Manual" => 85.0,
        _ => 0.0,
    };
    let name = normalize(&item.name);
    for common in [
        "wechat",
        "weixin",
        "微信",
        "qq",
        "wegame",
        "watt toolkit",
        "visual studio code",
        "chrome",
        "google chrome",
        "edge",
        "microsoft edge",
        "firefox",
        "steam",
        "discord",
        "telegram",
        "excel",
        "word",
        "powerpoint",
        "网易云音乐",
        "腾讯会议",
        "wemeet",
        "windows terminal",
    ] {
        if name == common
            || name
                .strip_prefix(common)
                .is_some_and(|s| s.starts_with(' '))
        {
            score += 65.0;
            break;
        }
    }
    if name == "wab"
        || name == "wabmig"
        || name.ends_with("svc")
        || name.ends_with("service")
        || ["updater", "crashpad", "crashreport", "app cert kit"]
            .iter()
            .any(|part| name.contains(part))
    {
        score -= 160.0;
    }
    if item.pinned {
        score += 100.0;
    }
    score
}

fn matching(query: &str, name: &str, terms: &[String], fuzzy: bool) -> i32 {
    if query.is_empty() {
        return 0;
    }
    if query == name {
        return 1000;
    }
    if name.starts_with(query) || terms.iter().any(|t| t.starts_with(query)) {
        return 800;
    }
    if name.contains(query) || terms.iter().any(|t| t.contains(query)) {
        return 500;
    }
    if !fuzzy {
        return -1;
    }
    if query.contains(' ') && query.split_whitespace().all(|p| name.contains(p)) {
        return 450;
    }
    let query: Vec<char> = query.chars().collect();
    if query.len() < 2 {
        return -1;
    }
    let mut matched = 0;
    let mut gaps = 0;
    for c in name.chars() {
        if matched < query.len() && c == query[matched] {
            matched += 1;
        } else if matched > 0 && matched < query.len() {
            gaps += 1;
        }
    }
    if matched == query.len() {
        return 350 - gaps.min(100);
    }
    if query.len() >= 3
        && terms.iter().map(String::as_str).chain([name]).any(|term| {
            let chars: Vec<_> = term.chars().collect();
            query.len().abs_diff(chars.len()) <= 1 && one_edit(&query, &chars)
        })
    {
        return 250;
    }
    -1
}

fn one_edit(a: &[char], b: &[char]) -> bool {
    let (mut i, mut j, mut edits) = (0, 0, 0);
    while i < a.len() && j < b.len() {
        if a[i] == b[j] {
            i += 1;
            j += 1;
            continue;
        }
        edits += 1;
        if edits > 1 {
            return false;
        }
        match a.len().cmp(&b.len()) {
            std::cmp::Ordering::Equal => {
                if i + 1 < a.len() && a[i] == b[j + 1] && a[i + 1] == b[j] {
                    i += 2;
                    j += 2;
                } else {
                    i += 1;
                    j += 1;
                }
            }
            std::cmp::Ordering::Greater => i += 1,
            std::cmp::Ordering::Less => j += 1,
        }
    }
    if i < a.len() || j < b.len() {
        edits += 1;
    }
    edits <= 1
}
