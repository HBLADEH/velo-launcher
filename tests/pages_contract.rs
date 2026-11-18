use serde::Deserialize;
use std::path::Path;
use velo_slint::core::{
    model::AppItem,
    system_tools::{self, Environment},
    update::Version,
};

#[derive(Deserialize)]
struct Capabilities {
    build: u32,
    settings: bool,
    policies: Vec<String>,
    missing: Vec<String>,
}
#[derive(Deserialize)]
struct Input {
    versions: Vec<[String; 2]>,
    environments: Vec<Capabilities>,
}
#[derive(Deserialize)]
struct Expected {
    versions: Vec<Option<i32>>,
    tools: Vec<Vec<AppItem>>,
}

#[test]
fn go_versions_and_windows_capabilities_match_shared_cases() {
    let input: Input =
        serde_json::from_slice(include_bytes!("../tests/fixtures/compat/rs11.json")).unwrap();
    let expected: Expected = serde_json::from_slice(include_bytes!(
        "../tests/fixtures/compat/rs11.expected.json"
    ))
    .unwrap();
    for (pair, expected) in input.versions.iter().zip(expected.versions) {
        let actual = Version::parse(&pair[0])
            .and_then(|a| Version::parse(&pair[1]).map(|b| a.compare(&b) as i32))
            .ok();
        assert_eq!(actual, expected, "{pair:?}");
    }
    let windows = Path::new(r"D:\Windows");
    for (case, expected) in input.environments.iter().zip(expected.tools) {
        let env = Environment {
            windows,
            build: case.build,
            settings: case.settings,
            policies: &case.policies,
        };
        let actual = system_tools::discover(&env, |path| {
            !case.missing.iter().any(|p| {
                p == "*"
                    || windows
                        .join(p.replace('/', "\\"))
                        .to_string_lossy()
                        .eq_ignore_ascii_case(&path.to_string_lossy())
            })
        })
        .unwrap();
        assert_eq!(actual, expected);
        for item in actual {
            if item.path.starts_with("ms-settings:") || item.path.starts_with("velo-action:") {
                assert!(system_tools::special(&item));
                let mut forged = item.clone();
                forged.source = "Manual".into();
                assert!(!system_tools::special(&forged));
                let mut forged = item.clone();
                forged.arguments = "--untrusted".into();
                assert!(!system_tools::special(&forged));
            }
        }
    }
}
