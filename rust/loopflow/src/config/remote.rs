//! Origin-owned personal declarations. Delivery receipts describe persistence;
//! they are never consulted by the settings resolver as preference values.
pub use super::definitions::DefinitionPreferences;
use super::{settings::SettingScope, ConfigDeclarations, ConfigSource};
use crate::definition::DefinitionKind;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const PROFILE_ENV: &str = "LF_PERSONAL_PROFILE";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub origin: String,
    pub revision: u64,
    pub repository: Option<String>,
    pub global_path: PathBuf,
    pub global: Value,
    pub repository_path: Option<PathBuf>,
    pub repository_values: Value,
    pub definitions: Vec<DefinitionPreferences>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Retention {
    pub definition_kind: Option<DefinitionKind>,
    pub origin: String,
    pub revision: u64,
    pub seen: BTreeMap<String, u64>,
    pub applied: Option<Value>,
    pub applied_origin: Option<String>,
    pub incoming: Value,
    pub status: String,
}

pub(super) fn project(value: &Value) -> Value {
    let mut normalized = value.clone();
    super::normalize_context(&mut normalized);
    let value = &normalized;
    fn setting(value: &Value, key: &str) -> Option<Value> {
        let definition = super::catalog::definition(key)?;
        let prefix = format!("{key}.");
        if super::catalog::definitions()
            .iter()
            .any(|child| child.key.starts_with(&prefix))
        {
            let projected = value
                .as_mapping()?
                .iter()
                .filter_map(|(name, value)| {
                    setting(value, &format!("{prefix}{}", name.as_str()?))
                        .map(|value| (name.clone(), value))
                })
                .collect::<serde_yaml_ng::Mapping>();
            return (!projected.is_empty()).then(|| projected.into());
        }
        (definition.portability != super::catalog::Portability::HostLocal).then(|| value.clone())
    }

    fn targets(value: &Value) -> Value {
        value
            .as_mapping()
            .into_iter()
            .flatten()
            .map(|(name, value)| (name.clone(), project(value)))
            .collect::<serde_yaml_ng::Mapping>()
            .into()
    }

    let mut out = serde_yaml_ng::Mapping::new();
    if let Some(map) = value.as_mapping() {
        for (key, value) in map {
            let Some(key) = key.as_str() else {
                continue;
            };
            let projected = if ["skills", "waves"].contains(&key) {
                targets(value)
            } else if key == "graphs" {
                let mut graphs = serde_yaml_ng::Mapping::new();
                if let Some(map) = value.as_mapping() {
                    for (name, graph) in map {
                        graphs.insert(
                            name.clone(),
                            serde_yaml_ng::Mapping::from_iter([(
                                "steps".into(),
                                targets(graph.get("steps").unwrap_or(&Value::Null)),
                            )])
                            .into(),
                        );
                    }
                }
                graphs.into()
            } else if let Some(projected) = setting(value, key) {
                projected
            } else {
                continue;
            };
            out.insert(key.into(), projected);
        }
    }
    out.into()
}

fn replace_projection(document: &mut Value, incoming: &Value) {
    super::normalize_context(document);
    let previous = project(document);
    remove_projection(document, &previous);
    super::files::overlay(document, incoming);
}
pub(super) fn remove_projection(document: &mut Value, portable: &Value) {
    if let (Some(map), Some(keys)) = (document.as_mapping_mut(), portable.as_mapping()) {
        for (key, value) in keys {
            if value.is_mapping() {
                if let Some(child) = map.get_mut(key) {
                    remove_projection(child, value);
                    if child.as_mapping().is_some_and(|map| map.is_empty()) {
                        map.remove(key);
                    }
                }
            } else {
                map.remove(key);
            }
        }
    }
}

pub fn outgoing(repo: Option<&Path>, origin: &str) -> Result<Profile> {
    // A forwarded invocation keeps its original author and revision through hops.
    if let Some(profile) = incoming()? {
        return Ok(profile);
    }
    let global_path = crate::store::lf_home_dir().join("config.yaml");
    let global = project(&super::files::read(&global_path)?);
    let repository_path = repo.map(super::files::private_path).transpose()?;
    let repository_values = repository_path
        .as_deref()
        .map(super::files::read)
        .transpose()?
        .map(|value| project(&value))
        .unwrap_or(Value::Mapping(Default::default()));
    let metadata = crate::store::lf_home_dir().join("profile-outgoing.yaml");
    let revision = super::files::update(&metadata, |value| {
        let revision = value
            .get("revision")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            .checked_add(1)
            .context("profile revision exhausted")?;
        value["revision"] = revision.into();
        Ok(revision)
    })?;
    Ok(Profile {
        origin: origin.into(),
        revision,
        repository: repo
            .and_then(|repo| crate::repository::RepoId::discover(repo).ok())
            .map(|repo| repo.to_string()),
        global_path,
        global,
        repository_path,
        repository_values,
        definitions: super::definitions::discover()?,
    })
}

fn validate_document(value: &Value, scope: SettingScope) -> Result<()> {
    let mut general = value.clone();
    let waves = general.as_mapping_mut().and_then(|map| map.remove("waves"));
    anyhow::ensure!(
        waves.is_none() || scope == SettingScope::Repository,
        "private Waves belong to repository configuration"
    );
    super::settings::validate_preferences(&general, false, true)?;
    for key in general
        .as_mapping()
        .context("profile settings must be a mapping")?
        .keys()
    {
        let key = key.as_str().context("setting key must be text")?;
        if !["skills", "graphs"].contains(&key) {
            super::settings::validate_scope(key, ConfigSource::Personal, scope)?;
        }
    }
    if let Some(waves) = waves {
        for (id, value) in waves
            .as_mapping()
            .context("Wave preferences must be a mapping")?
        {
            crate::id::WaveId::parse(id.as_str().context("Wave selector must be an ID")?)
                .map_err(anyhow::Error::msg)?;
            validate_document(value, SettingScope::Wave)?;
        }
    }
    Ok(())
}

pub fn incoming() -> Result<Option<Profile>> {
    std::env::var(PROFILE_ENV)
        .ok()
        .map(|text| {
            let profile: Profile = serde_json::from_str(&text)?;
            anyhow::ensure!(
                profile.global == project(&profile.global)
                    && profile.repository_values == project(&profile.repository_values),
                "remote profile contains host-local or unknown settings"
            );
            validate_document(&profile.global, SettingScope::Global)?;
            validate_document(&profile.repository_values, SettingScope::Repository)?;
            let mut definitions = std::collections::BTreeSet::new();
            for definition in &profile.definitions {
                super::definitions::validate(definition)?;
                anyhow::ensure!(
                    definitions.insert((definition.kind, &definition.name)),
                    "duplicate personal definition"
                );
            }
            Ok(profile)
        })
        .transpose()
}

fn read_projection(path: &Path, kind: Option<DefinitionKind>) -> Result<Value> {
    match kind {
        Some(kind) => super::definitions::read(path, kind),
        None => Ok(project(&super::files::read(path)?)),
    }
}

fn update_projection<T>(
    path: &Path,
    kind: Option<DefinitionKind>,
    change: impl FnOnce(&mut Value) -> Result<T>,
) -> Result<T> {
    match kind {
        Some(kind) => super::definitions::update(path, kind, change),
        None => super::files::update(path, |document| {
            let mut portable = project(document);
            let result = change(&mut portable)?;
            replace_projection(document, &portable);
            Ok(result)
        }),
    }
}

fn receipts(home: &Path) -> PathBuf {
    home.join("profile-retention.json")
}
fn read_receipts(path: &Path) -> Result<BTreeMap<PathBuf, Retention>> {
    let text = super::files::read_text(path)?;
    if text.is_empty() {
        Ok(BTreeMap::new())
    } else {
        Ok(serde_json::from_str(&text)?)
    }
}

fn retain(
    home: &Path,
    path: &Path,
    profile: &Profile,
    incoming: &Value,
    kind: Option<DefinitionKind>,
) -> Result<()> {
    let metadata = receipts(home);
    let _lock = super::files::lock(&metadata)?;
    let mut records = read_receipts(&metadata)?;
    let previous = records.get(path).cloned();
    if previous.as_ref().is_some_and(|previous| {
        previous
            .seen
            .get(&profile.origin)
            .is_some_and(|revision| *revision > profile.revision)
    }) {
        return Ok(());
    }
    if previous.as_ref().is_some_and(|previous| {
        previous.origin == profile.origin
            && previous.revision == profile.revision
            && previous.incoming != *incoming
    }) {
        anyhow::bail!(
            "different remote contents reuse profile revision {}",
            profile.revision
        );
    }
    let mut applied = previous
        .as_ref()
        .and_then(|previous| previous.applied.clone());
    let mut applied_origin = previous
        .as_ref()
        .and_then(|previous| previous.applied_origin.clone());
    let mut status = if previous
        .as_ref()
        .is_some_and(|previous| previous.status == "kept")
    {
        "kept"
    } else {
        "conflict"
    };
    let applicable = kind
        .map(|kind| super::definitions::can_retain(path, kind, incoming))
        .transpose()?
        .unwrap_or(true);
    update_projection(path, kind, |document| {
        let current = document.clone();
        let owned = previous.as_ref().is_some_and(|previous| {
            previous.applied_origin.as_deref() == Some(profile.origin.as_str())
                && previous.status == "retained"
                && previous.applied.as_ref() == Some(&current)
        });
        if applicable
            && ((current == *incoming
                && previous.as_ref().is_none_or(|previous| {
                    previous.applied_origin.as_deref() == Some(profile.origin.as_str())
                }))
                || (previous.is_none() && current.as_mapping().is_some_and(|map| map.is_empty()))
                || owned)
        {
            *document = incoming.clone();
            applied = Some(incoming.clone());
            applied_origin = Some(profile.origin.clone());
            status = "retained";
        }
        Ok(())
    })?;
    let changed = previous
        .as_ref()
        .is_none_or(|previous| previous.status != status || previous.incoming != *incoming);
    let mut seen = previous
        .as_ref()
        .map(|record| record.seen.clone())
        .unwrap_or_default();
    seen.insert(profile.origin.clone(), profile.revision);
    records.insert(
        path.to_owned(),
        Retention {
            definition_kind: kind,
            origin: profile.origin.clone(),
            revision: profile.revision,
            seen,
            applied,
            applied_origin,
            incoming: incoming.clone(),
            status: status.into(),
        },
    );
    super::files::atomic_write(&metadata, &serde_json::to_string_pretty(&records)?)?;
    if status == "conflict" && changed && !applicable {
        eprintln!("Origin preferences apply; {} needs its personal definition or matching occurrences installed before retention. `lf config retained` shows the pending declarations.", path.display());
    } else if status == "conflict" && changed {
        eprintln!("Remote execution uses {} preferences; {} keeps its independent edits. `lf config retained` shows the difference.", profile.origin, path.display());
    }
    Ok(())
}

pub(crate) fn layers(
    repo: Option<&Path>,
    wave: Option<&str>,
) -> Result<Option<Vec<ConfigDeclarations>>> {
    let Some(profile) = incoming()? else {
        return Ok(None);
    };
    let home = crate::store::lf_home_dir();
    let global = home.join("config.yaml");
    let mut local = super::files::read(&global)?;
    let portable = project(&local);
    remove_projection(&mut local, &portable);
    let mut layers = vec![
        ConfigDeclarations {
            source: ConfigSource::Personal,
            scope: SettingScope::Global,
            path: global.clone(),
            value: local,
        },
        ConfigDeclarations {
            source: ConfigSource::Personal,
            scope: SettingScope::Global,
            path: PathBuf::from(format!(
                "{}:{}",
                profile.origin,
                profile.global_path.display()
            )),
            value: profile.global.clone(),
        },
    ];
    if let Some(repo) = repo.filter(|repo| {
        crate::repository::RepoId::discover(repo)
            .ok()
            .map(|id| id.to_string())
            == profile.repository
    }) {
        let path = super::files::private_path(repo)?;
        let mut local = super::files::read(&path)?;
        let portable = project(&local);
        remove_projection(&mut local, &portable);
        let mut value = profile.repository_values.clone();
        for (source, value) in [
            (&path, &mut local),
            (
                profile.repository_path.as_ref().unwrap_or(&path),
                &mut value,
            ),
        ] {
            let waves = super::private_wave_layers(repo, wave, source, value)?;
            super::files::remove(value, &["waves"]);
            layers.push(ConfigDeclarations {
                source: ConfigSource::Personal,
                scope: SettingScope::Repository,
                path: source.clone(),
                value: value.clone(),
            });
            layers.extend(waves);
        }
    }
    Ok(Some(layers))
}

/// Retention is an inbound execution effect, never a side effect of resolution.
pub fn receive(repo: Option<&Path>) -> Result<()> {
    let Some(profile) = incoming()? else {
        return Ok(());
    };
    let home = crate::store::lf_home_dir();
    let mut destinations = vec![(home.join("config.yaml"), &profile.global)];
    if let Some(repo) = repo.filter(|repo| {
        crate::repository::RepoId::discover(repo)
            .ok()
            .map(|id| id.to_string())
            == profile.repository
    }) {
        destinations.push((
            super::files::private_path(repo)?,
            &profile.repository_values,
        ));
    }
    for (path, incoming) in destinations {
        if let Err(error) = retain(&home, &path, &profile, incoming, None) {
            eprintln!(
                "Origin preferences apply; retention unavailable for {}: {error:#}",
                path.display()
            );
        }
    }
    if let Err(error) = retain_definitions(&home, &profile) {
        eprintln!("Origin preferences apply; definition retention unavailable: {error:#}");
    }
    Ok(())
}

fn retain_definitions(home: &Path, profile: &Profile) -> Result<()> {
    let mut definitions = super::definitions::destinations(&profile.definitions)?;
    for (path, receipt) in read_receipts(&receipts(home))? {
        if let Some(kind) = receipt.definition_kind {
            definitions
                .entry(path)
                .or_insert((kind, Value::Mapping(Default::default())));
        }
    }
    for (path, (kind, incoming)) in definitions {
        if let Err(error) = retain(home, &path, profile, &incoming, Some(kind)) {
            eprintln!(
                "Origin preferences apply; retention unavailable for {}: {error:#}",
                path.display()
            );
        }
    }
    Ok(())
}

pub fn inspect(replace: bool, keep: bool) -> Result<serde_json::Value> {
    anyhow::ensure!(!(replace && keep), "choose keep or replace");
    let metadata = receipts(&crate::store::lf_home_dir());
    let _lock = super::files::lock(&metadata)?;
    let mut records = read_receipts(&metadata)?;
    let mut output = Vec::new();
    for (path, record) in &mut records {
        let mut current = read_projection(path, record.definition_kind)?;
        if record.status == "conflict" && replace {
            current = update_projection(path, record.definition_kind, |document| {
                *document = record.incoming.clone();
                Ok(document.clone())
            })?;
            record.applied = Some(record.incoming.clone());
            record.applied_origin = Some(record.origin.clone());
            record.status = "retained".into();
        } else if record.status == "conflict" && keep {
            record.status = "kept".into();
        }
        output.push(serde_json::json!({"path":path,"receipt":record,"saved":current}));
    }
    if replace || keep {
        super::files::atomic_write(&metadata, &serde_json::to_string_pretty(&records)?)?;
    }
    Ok(output.into())
}

/// Apply origin authority to a domain reader without consulting retention metadata.
pub(crate) fn apply_document(path: &Path, document: &mut Value) -> Result<()> {
    let Some(profile) = incoming()? else {
        return Ok(());
    };
    if path == crate::store::lf_home_dir().join("config.yaml") {
        replace_projection(document, &profile.global);
    } else if path
        .file_name()
        .is_some_and(|name| name == super::files::PRIVATE_FILE)
        && path
            .parent()
            .and_then(|repo| crate::repository::RepoId::discover(repo).ok())
            .map(|repo| repo.to_string())
            == profile.repository
    {
        replace_projection(document, &profile.repository_values);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{project, retain, Profile};
    use serde_yaml_ng::Value;
    fn profile(text: &str, revision: u64) -> Profile {
        Profile {
            origin: "origin".into(),
            revision,
            repository: None,
            global_path: "/origin/config.yaml".into(),
            global: serde_yaml_ng::from_str(text).unwrap(),
            repository_path: None,
            repository_values: Value::Mapping(Default::default()),
            definitions: vec![],
        }
    }
    #[test]
    fn context_projection_preserves_resets_and_keeps_clipboard_on_the_host() {
        let incoming: Value = serde_yaml_ng::from_str("context: {docs: [], diff: false, paste: true}\nskills: {design: {context: {paste: true}}}").unwrap();
        let portable = super::project(&incoming);
        assert_eq!(portable["context"]["docs"], Value::Sequence(vec![]));
        assert_eq!(portable["context"]["diff"], Value::Bool(false));
        assert!(portable["context"].get("paste").is_none());
        assert!(portable["skills"]["design"].get("context").is_none());
        let mut saved: Value = serde_yaml_ng::from_str("docs: [old.md]\npaste: true").unwrap();
        super::replace_projection(&mut saved, &portable);
        assert!(saved.get("docs").is_none());
        assert_eq!(saved["context"]["docs"], Value::Sequence(vec![]));
        assert_eq!(saved["context"]["paste"], Value::Bool(true));
    }

    #[test]
    fn origin_declarations_drive_launch_without_inspection_writing_files() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        let _home =
            crate::lf::commands::graph::EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
        let _personal_home =
            crate::lf::commands::graph::EnvVarGuard::set("HOME", home.path().to_str().unwrap());
        let repo = loopflow_test_support::TestRepo::new();
        repo.create_file(
            ".lf/skills/probe.md",
            "---\nagent: claude:shared\n---\nProbe.",
        );
        let path = home.path().join("config.yaml");
        std::fs::write(&path, "agent: codex:executor\nchrome: true\n").unwrap();
        let mut carried = profile("default_agent: codex:origin", 1);
        carried.repository_values = serde_yaml_ng::from_str("{}").unwrap();
        let selected = || {
            let config = crate::config::load_config(Some(repo.path()))
                .unwrap()
                .unwrap();
            let skill = crate::graph::load_skill("probe", repo.path()).unwrap();
            let inspection = crate::config::settings::resolve(&config, Some(&skill)).unwrap();
            let prepared = crate::prompt::process::prepare_process_prompt(
                &config,
                crate::prompt::process::ProcessPromptInput {
                    repo_root: repo.path().to_owned(),
                    resolved_skill: Some(skill),
                    ..Default::default()
                },
            )
            .unwrap();
            assert_eq!(
                prepared.config.agent().unwrap(),
                inspection.config.agent().unwrap()
            );
            assert_eq!(prepared.config.chrome, Some(true));
            inspection.config.agent().unwrap().to_owned()
        };
        {
            let _profile = crate::lf::commands::graph::EnvVarGuard::set(
                super::PROFILE_ENV,
                &serde_json::to_string(&carried).unwrap(),
            );
            assert_eq!(selected(), "claude:shared");
            assert!(!home.path().join("profile-retention.json").exists());
            super::receive(Some(repo.path())).unwrap();
            assert_eq!(selected(), "claude:shared");
        }
        assert_eq!(selected(), "codex:executor");
        // Explicit adoption saves authored fallback strength, not an effective CLI choice.
        let adopted = super::inspect(true, false).unwrap();
        assert_eq!(adopted[0]["saved"]["default_agent"], "codex:origin");
        assert!(adopted[0]["saved"].get("agent").is_none());
        assert_eq!(selected(), "claude:shared");
        repo.create_file(".lf/skills/probe.md", "Probe.");
        assert_eq!(selected(), "codex:origin");
    }

    #[test]
    fn alternating_origins_cannot_revive_stale_deliveries() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        let first = profile("agent: codex", 5);
        retain(dir.path(), &path, &first, &first.global, None).unwrap();
        let mut competing = profile("agent: claude", 1);
        competing.origin = "other".into();
        retain(dir.path(), &path, &competing, &competing.global, None).unwrap();
        let stale = profile("agent: codex:old", 4);
        retain(dir.path(), &path, &stale, &stale.global, None).unwrap();
        let records = super::read_receipts(&super::receipts(dir.path())).unwrap();
        assert_eq!(records[&path].incoming, competing.global);
        assert_eq!(
            project(&crate::config::files::read(&path).unwrap()),
            first.global
        );
    }

    #[test]
    fn retention_refreshes_owned_content_preserves_edits_and_rejects_stale_delivery() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        let first = profile("agent: codex", 1);
        retain(dir.path(), &path, &first, &first.global, None).unwrap();
        let next = profile("agent: claude", 2);
        retain(dir.path(), &path, &next, &next.global, None).unwrap();
        retain(dir.path(), &path, &first, &first.global, None).unwrap();
        assert_eq!(
            project(&crate::config::files::read(&path).unwrap()),
            next.global
        );
        std::fs::write(&path, "theme: custom\nlogs: {linear_requests: true}\n").unwrap();
        let later = profile("agent: codex", 3);
        retain(dir.path(), &path, &later, &later.global, None).unwrap();
        let saved = crate::config::files::read(&path).unwrap();
        assert!(saved.get("agent").is_none());
        assert_eq!(saved["theme"], "custom");
        assert_eq!(saved["logs"]["linear_requests"], true);
    }
}

#[cfg(test)]
mod definition_tests;
