//! History learning is synchronous background work. Store publishes only after
//! a durable commit; searches score an immutable snapshot without holding I/O locks.

use super::data::{History, HistoryEntry};
use super::normalize::normalize;
use super::storage::Store;
use super::{DataError, Result};
use chrono::{DateTime, Datelike, FixedOffset, SecondsFormat, Utc};
use std::collections::BTreeMap;
use std::sync::Arc;

impl History {
    pub fn record(&self, id: &str, query: &str, now: DateTime<FixedOffset>) -> Result<Self> {
        self.record_batch([id], query, now)
    }

    pub(super) fn record_batch<'a>(
        &self,
        ids: impl IntoIterator<Item = &'a str>,
        query: &str,
        now: DateTime<FixedOffset>,
    ) -> Result<Self> {
        if !(0..=9999).contains(&now.year()) {
            return Err(DataError::new(
                "record history",
                "timestamp outside Go RFC3339 range",
            ));
        }
        let query = normalize(query);
        let mut next = self.clone();
        let timestamp = now.to_rfc3339_opts(SecondsFormat::Nanos, true);
        let (seconds, fraction) = timestamp.split_at(19);
        // Go emits only significant fractional digits, preserving the offset.
        let digits = fraction[1..10].trim_end_matches('0');
        let last_launch_time = if digits.is_empty() {
            format!("{seconds}{}", &fraction[10..])
        } else {
            format!("{seconds}.{digits}{}", &fraction[10..])
        };
        for id in ids {
            let entry = next.0.entry(id.into()).or_default();
            entry.launch_count = entry
                .launch_count
                .checked_add(1)
                .ok_or_else(|| DataError::new("record history", "launch count overflow"))?;
            let queries = entry.query_history.get_or_insert_default();
            if !query.is_empty() {
                let count = queries.entry(query.clone()).or_default();
                *count = count
                    .checked_add(1)
                    .ok_or_else(|| DataError::new("record history", "query count overflow"))?;
            }
            if queries.len() > 100 {
                // A stable tie break keeps the current query during eviction.
                let key = queries
                    .iter()
                    .filter(|(key, _)| **key != query)
                    .min_by_key(|(_, count)| **count)
                    .map(|(key, _)| key.clone());
                if let Some(key) = key {
                    queries.remove(&key);
                }
            }
            entry.last_launch_time.clone_from(&last_launch_time);
        }
        Ok(next)
    }

    pub fn scores(&self, query: &str, now: DateTime<Utc>) -> Result<BTreeMap<String, f64>> {
        let query = normalize(query);
        self.0
            .iter()
            .map(|(id, entry)| Ok((id.clone(), score(entry, &query, now)?)))
            .collect()
    }
}

fn score(entry: &HistoryEntry, query: &str, now: DateTime<Utc>) -> Result<f64> {
    let last = DateTime::parse_from_rfc3339(&entry.last_launch_time)
        .map_err(|e| DataError::new("score history timestamp", e))?;
    // Go time.Sub saturates at an i64 nanosecond duration (about 292 years).
    let elapsed = now.signed_duration_since(last);
    let nanos = elapsed
        .num_nanoseconds()
        .unwrap_or(if elapsed < chrono::TimeDelta::zero() {
            i64::MIN
        } else {
            i64::MAX
        });
    let age = ((nanos as f64 / 3_600_000_000_000.0) / 24.0).max(0.0);
    let count = entry
        .query_history
        .as_ref()
        .and_then(|q| q.get(query))
        .copied()
        .unwrap_or(0);
    Ok(((entry.launch_count as f64 + 1.0).log2() * 15.0).min(100.0)
        + 35.0 / (1.0 + age)
        + (count as f64 * 35.0).min(220.0))
}

impl Store<History> {
    /// Call only after the launcher reports success; failed/cancelled launches
    /// must never call this API. Actual Shell dispatch is introduced in RS-10.
    pub fn record_success(
        &self,
        id: &str,
        query: &str,
        now: DateTime<FixedOffset>,
    ) -> Result<Arc<History>> {
        self.update(|history| history.record(id, query, now))
    }
}
