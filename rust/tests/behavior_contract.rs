use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::Arc;
use velo_slint::core::catalog::{Catalog, Home};
use velo_slint::core::config::Config;
use velo_slint::core::data::{ApplicationLibrary, History};
use velo_slint::core::model::AppItem;
use velo_slint::core::normalize::normalize;
use velo_slint::core::search::{Index, SearchResult};

#[derive(Deserialize)]
struct Fixture {
    schema_version: i64,
    fixed_time: String,
    normalizations: Vec<NormalizationCase>,
    searches: Vec<SearchCase>,
    histories: Vec<HistoryCase>,
    services: Vec<ServiceCase>,
}
#[derive(Deserialize)]
struct NormalizationCase {
    name: String,
    input: String,
    expected: String,
}
#[derive(Deserialize)]
struct SearchCase {
    name: String,
    apps: Vec<AppItem>,
    queries: Vec<QueryCase>,
}
#[derive(Deserialize)]
struct QueryCase {
    query: String,
    limit: i64,
    fuzzy: bool,
    weights: Option<BTreeMap<String, f64>>,
    weight: f64,
    expected: Vec<ExpectedResult>,
}
#[derive(Deserialize)]
struct ExpectedResult {
    #[serde(flatten)]
    item: AppItem,
    score: f64,
}
#[derive(Deserialize)]
struct HistoryCase {
    name: String,
    initial: History,
    records: Vec<Record>,
    expected: History,
    scores: BTreeMap<String, BTreeMap<String, f64>>,
}
#[derive(Deserialize)]
struct Record {
    id: String,
    query: String,
    time: String,
    success: bool,
}
#[derive(Deserialize)]
struct ServiceCase {
    name: String,
    apps: Vec<AppItem>,
    tools: Vec<AppItem>,
    library: ApplicationLibrary,
    history: History,
    config: Config,
    queries: Vec<String>,
    pins: Vec<Pin>,
    expected: Vec<State>,
}
#[derive(Deserialize)]
struct Pin {
    id: String,
    pinned: bool,
    error: bool,
}
#[derive(Deserialize)]
struct State {
    library: ApplicationLibrary,
    home: ExpectedHome,
    searches: Vec<Vec<ExpectedResult>>,
}
#[derive(Deserialize)]
struct ExpectedHome {
    pinned: Vec<AppItem>,
    frequent: Vec<AppItem>,
    tools: Vec<AppItem>,
}

fn fixture() -> Fixture {
    let fixture: Fixture =
        serde_json::from_str(include_str!("../../tests/fixtures/compat/rs09.json")).unwrap();
    assert_eq!(fixture.schema_version, 1);
    fixture
}

fn results(actual: &[SearchResult], expected: &[ExpectedResult], case: &str) {
    assert_eq!(actual.len(), expected.len(), "{case}: result count");
    for (actual, expected) in actual.iter().zip(expected) {
        assert_eq!(
            *actual.item, expected.item,
            "{case}: ordered item and launch fields"
        );
        assert!(
            (actual.score - expected.score).abs() <= 1e-9,
            "{case}: score {} != {}",
            actual.score,
            expected.score
        );
    }
}

fn home(actual: Home, expected: &ExpectedHome, case: &str) {
    for (actual, expected) in [
        (actual.pinned, &expected.pinned),
        (actual.frequent, &expected.frequent),
        (actual.tools, &expected.tools),
    ] {
        assert_eq!(
            actual.iter().map(AsRef::as_ref).collect::<Vec<_>>(),
            expected.iter().collect::<Vec<_>>(),
            "{case}: home order/fields"
        );
    }
}

#[test]
fn go_normalization_and_ranked_search_outputs_match() {
    let fixture = fixture();
    for case in fixture.normalizations {
        assert_eq!(normalize(&case.input), case.expected, "{}", case.name);
    }
    for case in fixture.searches {
        let index = Index::new(case.apps.into_iter().map(Arc::new));
        for query in case.queries {
            let actual = index.query(
                &query.query,
                usize::try_from(query.limit).unwrap_or(0),
                query.fuzzy,
                &query.weights.unwrap_or_default(),
                query.weight,
            );
            results(
                &actual,
                &query.expected,
                &format!("{} {:?}", case.name, query.query),
            );
        }
    }
}

#[test]
fn go_history_learning_eviction_and_scores_match() {
    let fixture = fixture();
    let now = DateTime::parse_from_rfc3339(&fixture.fixed_time)
        .unwrap()
        .with_timezone(&Utc);
    for case in fixture.histories {
        let mut history = case.initial;
        for record in case.records {
            if record.success {
                history = history
                    .record(
                        &record.id,
                        &record.query,
                        DateTime::parse_from_rfc3339(&record.time).unwrap(),
                    )
                    .unwrap();
            }
        }
        assert_eq!(history, case.expected, "{}", case.name);
        for (query, expected) in case.scores {
            let actual = history.scores(&query, now).unwrap();
            assert_eq!(actual.len(), expected.len());
            for (id, expected) in expected {
                assert!(
                    (actual[&id] - expected).abs() <= 1e-9,
                    "{}: history score {id}",
                    case.name
                );
            }
        }
    }
}

#[test]
fn go_service_home_search_and_pin_transitions_match() {
    let fixture = fixture();
    let now = DateTime::parse_from_rfc3339(&fixture.fixed_time)
        .unwrap()
        .with_timezone(&Utc);
    for case in fixture.services {
        let mut library = case.library;
        assert_eq!(case.expected.len(), case.pins.len() + 1);
        for (step, expected) in case.expected.iter().enumerate() {
            if step > 0 {
                let pin = &case.pins[step - 1];
                let change = Catalog::new(&case.apps, &library, &case.tools)
                    .with_pin(&library, &pin.id, pin.pinned);
                assert_eq!(change.is_err(), pin.error, "{}: pin error", case.name);
                if let Ok(next) = change {
                    library = next;
                }
            }
            assert_eq!(
                library, expected.library,
                "{}: committed library",
                case.name
            );
            let catalog = Catalog::new(&case.apps, &library, &case.tools);
            home(
                catalog.home(&case.history, now).unwrap(),
                &expected.home,
                &case.name,
            );
            for (query, expected) in case.queries.iter().zip(&expected.searches) {
                results(
                    &catalog
                        .search(query, &case.config, &case.history, now)
                        .unwrap(),
                    expected,
                    &case.name,
                );
            }
        }
    }
}
