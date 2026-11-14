use serde::Deserialize;
use velo_slint::core::model::AppItem;
use velo_slint::core::{indexer, launch};

#[derive(Deserialize)]
struct Fixture {
    schema_version: i64,
    names: Vec<Name>,
    cases: Vec<Case>,
}
#[derive(Deserialize)]
struct Name {
    name: String,
    noise: bool,
}
#[derive(Deserialize)]
struct Case {
    name: String,
    items: Vec<AppItem>,
    deduplicated: Vec<AppItem>,
    filtered: Vec<AppItem>,
    all: Vec<AppItem>,
    actions: Vec<Actions>,
}
#[derive(Deserialize)]
struct Actions {
    open_directory: bool,
    run_as_admin: bool,
}

#[test]
fn discovery_filter_aliases_profiles_and_actions_match_the_go_oracle() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("../../tests/fixtures/compat/rs10.json")).unwrap();
    assert_eq!(fixture.schema_version, 1);
    for case in fixture.names {
        assert_eq!(indexer::is_noise(&case.name), case.noise, "{}", case.name);
    }
    for case in fixture.cases {
        assert_eq!(
            indexer::deduplicate(case.items.clone()),
            case.deduplicated,
            "{}: deduplication",
            case.name
        );
        assert_eq!(
            indexer::visible(&case.items, true),
            case.filtered,
            "{}: filtering",
            case.name
        );
        assert_eq!(
            indexer::visible(&case.items, false),
            case.all,
            "{}: filter disabled",
            case.name
        );
        for (item, expected) in case.items.iter().zip(case.actions) {
            assert_eq!(
                launch::capabilities(item),
                (expected.open_directory, expected.run_as_admin),
                "{}",
                case.name
            );
        }
    }
}
