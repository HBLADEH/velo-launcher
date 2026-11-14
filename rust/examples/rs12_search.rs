//! Same serialized synthetic inputs and fixed clock as the Go measurement.
//! This Release-only tool measures memory queries, not input-to-present latency.
use std::collections::BTreeMap;
use std::hint::black_box;
use std::sync::Arc;
use std::time::Instant;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::json;
use velo_slint::core::{
    catalog::Catalog,
    config::Config,
    data::{ApplicationLibrary, History},
    model::AppItem,
    search::Index,
};

#[derive(Deserialize)]
struct Fixture {
    now: String,
    apps: Vec<AppItem>,
    tools: Vec<AppItem>,
    config: Config,
    history: History,
    queries: Vec<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if cfg!(debug_assertions) {
        return Err("RS-12 search timing requires a Release build".into());
    }
    let directory = std::path::PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("missing evidence directory")?,
    );
    let fixture: Fixture = serde_json::from_slice(&std::fs::read(directory.join("fixture.json"))?)?;
    let now: DateTime<Utc> = fixture.now.parse()?;
    if fixture.apps.len() != 10000 {
        return Err("expected the shared 10000-item fixture".into());
    }
    let mut records = Vec::new();
    for count in [1000, 10000] {
        for learned in [false, true] {
            let history = History(if learned {
                fixture.apps[..count]
                    .iter()
                    .filter_map(|item| {
                        fixture
                            .history
                            .0
                            .get(&item.id)
                            .map(|entry| (item.id.clone(), entry.clone()))
                    })
                    .collect()
            } else {
                BTreeMap::new()
            });
            let mut library = ApplicationLibrary::empty();
            library.pinned = Some(
                fixture.apps[..4]
                    .iter()
                    .chain(&fixture.tools[..1])
                    .cloned()
                    .collect(),
            );
            let catalog = Catalog::new(&fixture.apps[..count], &library, &fixture.tools);
            let kernel = Index::new(fixture.apps[..count].iter().cloned().map(Arc::new));
            for query in &fixture.queries {
                let weights = history.scores(query, now)?;
                for layer in ["kernel", "service"] {
                    let call = || -> velo_slint::core::Result<_> {
                        if layer == "kernel" {
                            Ok(kernel.query(
                                query,
                                fixture.config.max_results as usize,
                                fixture.config.search.fuzzy,
                                &weights,
                                fixture.config.search.history_weight,
                            ))
                        } else {
                            catalog.search(query, &fixture.config, &history, now)
                        }
                    };
                    for _ in 0..8 {
                        black_box(call()?);
                    }
                    for round in 1..=3 {
                        let iterations = 256;
                        let start = Instant::now();
                        for _ in 0..iterations {
                            black_box(call()?);
                        }
                        let elapsed = start.elapsed().as_nanos();
                        let ids: Vec<_> = call()?
                            .into_iter()
                            .map(|result| result.item.id.clone())
                            .collect();
                        records.push(json!({"count":count,"history":learned,"layer":layer,"query":query,"round":round,"iterations":iterations,"elapsed_nanoseconds":elapsed as u64,"nanoseconds_per_operation":elapsed as f64 / iterations as f64,"result_ids":ids}));
                    }
                }
            }
        }
    }
    // Timing comparisons are meaningful only after the actual results agree.
    let reference: Vec<serde_json::Value> =
        serde_json::from_slice(&std::fs::read(directory.join("go.json"))?)?;
    let key = |row: &serde_json::Value| {
        format!(
            "{}/{}/{}/{}/{}",
            row["count"], row["history"], row["layer"], row["query"], row["round"]
        )
    };
    let reference: BTreeMap<_, _> = reference
        .iter()
        .map(|row| (key(row), &row["result_ids"]))
        .collect();
    for row in &records {
        if reference.get(&key(row)).copied() != Some(&row["result_ids"]) {
            return Err(format!("cross-language search result mismatch: {}", key(row)).into());
        }
    }
    let destination = directory.join("rust.json");
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)?;
    serde_json::to_writer_pretty(file, &records)?;
    println!(
        "RS-12 shared search measurements passed: {} matching cases",
        records.len()
    );
    Ok(())
}
