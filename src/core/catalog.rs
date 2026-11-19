//! The scanner supplies visible applications; RS-10 owns discovery and noise
//! filtering. This snapshot merges explicit user entries, pins and system tools.

use super::config::Config;
use super::data::{ApplicationLibrary, History};
use super::model::AppItem;
use super::normalize::normalize;
use super::search::{Index, SearchResult};
use super::storage::Store;
use super::{DataError, Result};
use chrono::{DateTime, Utc};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

pub struct Home {
    pub pinned: Vec<Arc<AppItem>>,
    pub frequent: Vec<Arc<AppItem>>,
    pub tools: Vec<Arc<AppItem>>,
}

pub struct Catalog {
    index: Index,
    tool_index: Index,
    items: BTreeMap<String, Arc<AppItem>>,
    application_count: usize,
    tools: Vec<Arc<AppItem>>,
    pinned: Vec<Arc<AppItem>>,
}

impl Catalog {
    /// Build on a bounded background worker before publishing the snapshot.
    /// Input must already have the scanner's deduplication/filtering applied.
    pub fn new(visible: &[AppItem], library: &ApplicationLibrary, tools: &[AppItem]) -> Self {
        let mut visible = visible.to_vec();
        for item in library.applications.iter().flatten() {
            visible.retain(|current| current.id != item.id);
            let mut item = item.clone();
            item.pinned = false;
            visible.push(item);
        }
        for item in library
            .pinned
            .iter()
            .flatten()
            .filter(|i| i.source != "System")
        {
            let mut item = visible
                .iter()
                .find(|i| i.id == item.id)
                .unwrap_or(item)
                .clone();
            visible.retain(|current| current.id != item.id);
            item.pinned = true;
            visible.push(item);
        }
        let items: BTreeMap<_, _> = visible
            .into_iter()
            .map(|item| (item.id.clone(), Arc::new(item)))
            .collect();
        let tools: Vec<_> = tools
            .iter()
            .map(|item| {
                let mut item = item.clone();
                item.pinned = library.has_pin(&item.id);
                Arc::new(item)
            })
            .collect();
        let pinned = library
            .pinned
            .iter()
            .flatten()
            .map(|item| {
                items.get(&item.id).cloned().unwrap_or_else(|| {
                    let mut item = item.clone();
                    item.pinned = true;
                    Arc::new(item)
                })
            })
            .collect();
        Self {
            index: Index::new(items.values().cloned()),
            tool_index: Index::new(tools.iter().cloned()),
            application_count: items.values().filter(|item| !item.is_folder()).count(),
            items,
            tools,
            pinned,
        }
    }

    pub fn len(&self) -> usize {
        self.application_count
    }
    pub fn is_empty(&self) -> bool {
        self.application_count == 0
    }

    pub fn item(&self, id: &str) -> Option<&Arc<AppItem>> {
        self.items
            .get(id)
            .or_else(|| self.tools.iter().find(|item| item.id == id))
    }

    pub fn applications(&self) -> impl Iterator<Item = &Arc<AppItem>> {
        self.items.values().filter(|item| !item.is_folder())
    }

    pub fn search(
        &self,
        query: &str,
        config: &Config,
        history: &History,
        now: DateTime<Utc>,
    ) -> Result<Vec<SearchResult>> {
        self.search_matching(query, config, history, now, |_| true)
    }

    pub fn search_applications(
        &self,
        query: &str,
        config: &Config,
        history: &History,
        now: DateTime<Utc>,
    ) -> Result<Vec<SearchResult>> {
        self.search_matching(query, config, history, now, |item| !item.is_folder())
    }

    fn search_matching(
        &self,
        query: &str,
        config: &Config,
        history: &History,
        now: DateTime<Utc>,
        include: impl Fn(&AppItem) -> bool,
    ) -> Result<Vec<SearchResult>> {
        let query = normalize(query);
        let weights = history.scores(&query, now)?;
        let limit = usize::try_from(config.max_results).unwrap_or(0);
        let mut results = self.index.query_matching(
            &query,
            limit,
            config.search.fuzzy,
            &weights,
            config.search.history_weight,
            include,
        );
        if !query.is_empty() {
            results.extend(self.tool_index.query(
                &query,
                limit,
                config.search.fuzzy,
                &weights,
                config.search.history_weight,
            ));
            // Go keeps application results ahead of equal-score tools. Do not
            // introduce a new global name/ID tie break across the two indexes.
            results.sort_by(|a, b| b.score.total_cmp(&a.score));
            results.truncate(limit);
        }
        for result in &mut results {
            let pinned = self.pinned.iter().any(|item| item.id == result.item.id);
            if result.item.pinned != pinned {
                let mut item = result.item.as_ref().clone();
                item.pinned = pinned;
                result.item = Arc::new(item);
            }
        }
        Ok(results)
    }

    pub fn home(&self, history: &History, now: DateTime<Utc>) -> Result<Home> {
        let mut pinned = Vec::new();
        for item in &self.pinned {
            if item.source == "System" {
                if let Some(tool) = self.tools.iter().find(|tool| tool.id == item.id) {
                    pinned.push(Arc::clone(tool));
                }
            } else {
                pinned.push(Arc::clone(item));
            }
        }
        let ids: BTreeSet<_> = pinned.iter().map(|item| item.id.as_str()).collect();
        let weights = history.scores("", now)?;
        let weight = |id: &str| weights.get(id).copied().unwrap_or(0.0);
        let mut results = self
            .index
            .query("", self.index.len(), false, &BTreeMap::new(), 0.0);
        // Personal history precedes cold-start priority, independently of the
        // search history setting; stable ties retain the search baseline order.
        results.sort_by(|a, b| weight(&b.item.id).total_cmp(&weight(&a.item.id)));
        let frequent = results
            .into_iter()
            .filter(|r| !ids.contains(r.item.id.as_str()))
            .take(12)
            .map(|r| r.item)
            .collect();
        let mut tools: Vec<_> = self
            .tools
            .iter()
            .filter(|item| !ids.contains(item.id.as_str()))
            .cloned()
            .collect();
        tools.sort_by(|a, b| weight(&b.id).total_cmp(&weight(&a.id)));
        Ok(Home {
            pinned,
            frequent,
            tools,
        })
    }

    pub fn with_pin(
        &self,
        library: &ApplicationLibrary,
        id: &str,
        pinned: bool,
    ) -> Result<ApplicationLibrary> {
        if pinned && !library.has_pin(id) {
            let item = self
                .item(id)
                .ok_or_else(|| DataError::new("pin application", "item no longer available"))?;
            return library.with_new_pin(item);
        }
        let mut next = library.clone();
        if !pinned && let Some(pins) = &mut next.pinned {
            pins.retain(|item| item.id != id);
        }
        Ok(next)
    }
}

impl Store<ApplicationLibrary> {
    /// Resolve against the committed library inside the writer transaction, so
    /// concurrent pins cannot overwrite each other. Rebuild after success only.
    pub fn set_pinned(
        &self,
        visible: &[AppItem],
        tools: &[AppItem],
        id: &str,
        pinned: bool,
    ) -> Result<Arc<ApplicationLibrary>> {
        self.update(|library| Catalog::new(visible, library, tools).with_pin(library, id, pinned))
    }
}
