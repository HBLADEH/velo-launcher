use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::Deserialize;
use serde_json::Value;
use velo_slint::core::config::Config;
use velo_slint::core::data::{ApplicationLibrary, DataStore, History, IndexCache, IndexLoad};
use velo_slint::core::model::identity;
use velo_slint::core::{JsonDocument, storage};

pub type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[derive(Deserialize)]
pub struct IdentityCase {
    name: String,
    path: String,
    arguments: String,
    expected: String,
}

#[derive(Deserialize)]
pub struct ConfigCase {
    name: String,
    input: String,
    valid: bool,
    expected: Option<Value>,
}

#[derive(Deserialize)]
pub struct Fixture {
    pub schema_version: u32,
    pub fixed_time: String,
    pub identities: Vec<IdentityCase>,
    pub configs: Vec<ConfigCase>,
    pub bundle: Value,
    pub expected_bundle: Value,
    pub edited_bundle: Value,
    pub legacy: Value,
    pub expected_legacy: Value,
}

impl Fixture {
    pub fn load(path: &Path) -> TestResult<Self> {
        let fixture: Self = serde_json::from_slice(&fs::read(path)?)?;
        if fixture.schema_version != 1 {
            return Err("unsupported fixture schema".into());
        }
        Ok(fixture)
    }

    pub fn check(&self) -> TestResult {
        for case in &self.identities {
            if identity(&case.path, &case.arguments) != case.expected {
                return Err(format!("identity case {} differs from Go", case.name).into());
            }
        }
        for case in &self.configs {
            let result = Config::decode(case.input.as_bytes());
            if result.is_ok() != case.valid {
                return Err(format!("config case {} validity differs from Go", case.name).into());
            }
            if let Ok(mut config) = result {
                // Go drops unknown fields in its typed output. Rust's preservation
                // is verified separately, while all known fields must still agree.
                config.unknown.clear();
                config.search.unknown.clear();
                let expected = case.expected.as_ref().ok_or("missing Go expectation")?;
                if config != decode::<Config>(expected)? {
                    return Err(format!("config case {} differs from Go", case.name).into());
                }
            }
        }
        // These fields are also consumed by the migration tests; validate them
        // when running this support module as an offline example.
        let legacy: Option<Vec<velo_slint::core::model::AppItem>> =
            serde_json::from_value(self.legacy.clone())?;
        if ApplicationLibrary::migrate(legacy) != decode(&self.expected_legacy)? {
            return Err("legacy library differs from Go".into());
        }
        Ok(())
    }

    pub fn emit(&self, directory: &Path) -> TestResult {
        fs::create_dir(directory)?;
        storage::write(
            &directory.join("config.json"),
            &decode::<Config>(&self.bundle["config"])?,
        )?;
        storage::write(
            &directory.join("history.json"),
            &decode::<History>(&self.bundle["history"])?,
        )?;
        storage::write(
            &directory.join("index.json"),
            &decode::<IndexCache>(&self.bundle["index"])?,
        )?;
        storage::write(
            &directory.join("application-library.json"),
            &decode::<ApplicationLibrary>(&self.bundle["library"])?,
        )?;
        Ok(())
    }

    pub fn verify(&self, directory: &Path, edited: bool) -> TestResult {
        let expected = if edited {
            &self.edited_bundle
        } else {
            &self.expected_bundle
        };
        let config: Config = storage::read(&directory.join("config.json"))?;
        let history: History = storage::read(&directory.join("history.json"))?;
        let index: IndexCache = storage::read(&directory.join("index.json"))?;
        let library: ApplicationLibrary =
            storage::read(&directory.join("application-library.json"))?;
        if config != decode(&expected["config"])?
            || history != decode(&expected["history"])?
            || index != decode(&expected["index"])?
            || library != decode(&expected["library"])?
        {
            return Err("data bundle differs from Go expectations".into());
        }
        Ok(())
    }

    pub fn rewrite(&self, directory: &Path) -> TestResult {
        self.verify(directory, false)?;
        let data = DataStore::open(directory)?;
        data.config.update(|current| {
            let mut next = current.clone();
            next.theme = "light".into();
            next.max_results = 19;
            Ok(next)
        })?;
        data.history.update(|current| {
            let mut next = current.clone();
            if let Some(entry) = next.0.get_mut("custom-preserved-id") {
                entry.launch_count += 1;
                entry.last_launch_time.clone_from(&self.fixed_time);
                *entry
                    .query_history
                    .get_or_insert_with(BTreeMap::new)
                    .entry("测试".into())
                    .or_default() += 1;
            }
            Ok(next)
        })?;
        data.library.update(|current| {
            let mut next = current.clone();
            next.applications = Some(Vec::new());
            if let Some(pinned) = &mut next.pinned {
                pinned.retain(|item| item.id != "custom-preserved-id");
            }
            Ok(next)
        })?;
        match data.index {
            IndexLoad::Ready(index) => storage::write(&directory.join("index.json"), &index)?,
            _ => return Err("expected readable fixture index".into()),
        }
        Ok(())
    }
}

pub fn decode<T: JsonDocument>(value: &Value) -> TestResult<T> {
    Ok(T::decode(&serde_json::to_vec(value)?)?)
}
