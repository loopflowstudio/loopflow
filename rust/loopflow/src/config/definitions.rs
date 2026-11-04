//! Portable LF preferences in personal definitions, independent of their bodies/topology.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value;

use super::settings::{SettingCandidate, SettingScope, SettingTarget};
use super::ConfigSource;
use crate::definition::DefinitionKind;
use crate::skills::catalog::{SkillCatalog, SkillDialect};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefinitionPreferences {
    pub kind: DefinitionKind,
    pub name: String,
    pub path: PathBuf,
    pub values: Value,
}

/// Select the personal profile before ranking; captured instructions retain their
/// source while host-local preferences come from the executor's definitions.
pub(super) fn candidates(skill: &crate::graph::Skill) -> Result<Vec<SettingCandidate>> {
    let profile = super::remote::incoming()?;
    let mut candidates = Vec::new();
    let mut add = |source, target: SettingTarget, mut locator: String, mut value: Value| {
        if profile.is_some() && source == ConfigSource::Personal {
            let host = host_preferences(&target, skill)?;
            value = match host {
                Some((path, value)) => {
                    locator = path.display().to_string();
                    value
                }
                None => Value::Mapping(Default::default()),
            };
        }
        candidates.push(candidate(source, target, locator, value));
        Ok::<_, anyhow::Error>(())
    };
    let source = skill.source.as_ref();
    add(
        source
            .and_then(|source| source.config_source)
            .unwrap_or_else(|| {
                super::settings::definition_source(source.map(|source| source.path.as_path()))
            }),
        SettingTarget::Skill {
            name: skill.name.clone(),
        },
        source.map_or_else(
            || "builtin".into(),
            |source| source.path.display().to_string(),
        ),
        super::settings::skill_values(skill),
    )?;
    if let Some(occurrence) = &skill.occurrence {
        add(
            occurrence.source,
            SettingTarget::GraphOccurrence {
                graph: occurrence.graph.clone(),
                id: occurrence.id.clone(),
            },
            occurrence.path.as_ref().map_or_else(
                || format!("graph:{}", occurrence.graph),
                |path| path.display().to_string(),
            ),
            occurrence.values.clone(),
        )?;
    }
    let Some(profile) = profile else {
        return Ok(candidates);
    };
    for definition in &profile.definitions {
        let (target, value) = match definition.kind {
            DefinitionKind::Skill if definition.name == skill.name => (
                SettingTarget::Skill {
                    name: skill.name.clone(),
                },
                Some(&definition.values),
            ),
            DefinitionKind::Graph => {
                let Some(occurrence) = skill
                    .occurrence
                    .as_ref()
                    .filter(|occurrence| occurrence.graph == definition.name)
                else {
                    continue;
                };
                (
                    SettingTarget::GraphOccurrence {
                        graph: occurrence.graph.clone(),
                        id: occurrence.id.clone(),
                    },
                    occurrence
                        .address()
                        .and_then(|address| definition.values.get(address)),
                )
            }
            _ => continue,
        };
        if let Some(value) = value {
            candidates.push(candidate(
                ConfigSource::Personal,
                target,
                format!("{}:{}", profile.origin, definition.path.display()),
                value.clone(),
            ));
        }
    }
    Ok(candidates)
}

fn candidate(
    source: ConfigSource,
    target: SettingTarget,
    locator: String,
    value: Value,
) -> SettingCandidate {
    SettingCandidate {
        key: String::new(),
        reason: String::new(),
        scope: if source == ConfigSource::Repository {
            SettingScope::Repository
        } else {
            SettingScope::Global
        },
        source,
        target,
        locator,
        value,
    }
}

pub(super) fn discover() -> Result<Vec<DefinitionPreferences>> {
    let catalog = SkillCatalog::discover(None)?;
    let mut definitions = Vec::new();
    for source in catalog
        .entries()
        .filter(|source| source.dialect == SkillDialect::Loopflow)
    {
        if let Some(path) = &source.path {
            definitions.push(DefinitionPreferences {
                kind: DefinitionKind::Skill,
                name: source.name.clone(),
                path: path.clone(),
                values: read(path, DefinitionKind::Skill)?,
            });
        }
    }
    if let Some(home) = dirs::home_dir() {
        for (name, path) in crate::graph::graph_sources_in(&home)? {
            let values = read(&path, DefinitionKind::Graph)?;
            definitions.push(DefinitionPreferences {
                kind: DefinitionKind::Graph,
                name,
                path,
                values,
            });
        }
    }
    Ok(definitions)
}

fn metadata(value: &Value) -> Result<Value> {
    Ok(serde_yaml_ng::to_value(super::settings::metadata(value)?)?)
}

fn parse(text: &str, kind: DefinitionKind) -> Result<Value> {
    let yaml = match kind {
        DefinitionKind::Skill => {
            crate::graph::split_frontmatter(text).map_or("{}", |(yaml, _)| yaml)
        }
        DefinitionKind::Graph => text,
    };
    Ok(serde_yaml_ng::from_str(yaml)?)
}

fn projection(document: &mut Value, kind: DefinitionKind) -> Result<Value> {
    if kind == DefinitionKind::Skill {
        return Ok(super::remote::project(&metadata(document)?));
    }
    let mut values = serde_yaml_ng::Mapping::new();
    crate::graph::visit_preferences(document, "", &mut |address, value| {
        let selected = super::remote::project(
            &metadata(value)
                .map_err(|error| crate::error::LoadError::InvalidGraph(error.to_string()))?,
        );
        if selected.as_mapping().is_some_and(|map| !map.is_empty()) {
            values.insert(address.into(), selected);
        }
        Ok(())
    })?;
    Ok(values.into())
}

pub(super) fn read(path: &Path, kind: DefinitionKind) -> Result<Value> {
    let text = super::files::read_text(path)?;
    if text.is_empty() {
        return Ok(Value::Mapping(Default::default()));
    }
    projection(&mut parse(&text, kind)?, kind)
}

/// Even captured definitions use the executor's current host bindings.
fn host_preferences(
    target: &super::settings::SettingTarget,
    skill: &crate::graph::Skill,
) -> Result<Option<(PathBuf, Value)>> {
    let (path, mut value) = match target {
        SettingTarget::Skill { name } => {
            let catalog = SkillCatalog::discover(None)?;
            let Some(source) = catalog
                .exact(name)
                .filter(|source| source.dialect == SkillDialect::Loopflow)
            else {
                return Ok(None);
            };
            let Some(path) = &source.path else {
                return Ok(None);
            };
            (
                path.clone(),
                metadata(&parse(
                    &super::files::read_text(path)?,
                    DefinitionKind::Skill,
                )?)?,
            )
        }
        SettingTarget::GraphOccurrence { graph, .. } => {
            let Some(home) = dirs::home_dir() else {
                return Ok(None);
            };
            let sources = crate::graph::graph_sources_in(&home)?;
            let Some(path) = sources.get(graph) else {
                return Ok(None);
            };
            let address = skill
                .occurrence
                .as_ref()
                .and_then(|occurrence| occurrence.address());
            let mut selected = Value::Mapping(Default::default());
            let mut document = parse(&super::files::read_text(path)?, DefinitionKind::Graph)?;
            crate::graph::visit_preferences(&mut document, "", &mut |key, value| {
                if address.as_deref() == Some(key) {
                    selected = metadata(value).map_err(|error| {
                        crate::error::LoadError::InvalidGraph(error.to_string())
                    })?;
                }
                Ok(())
            })?;
            (path.clone(), selected)
        }
        SettingTarget::General => return Ok(None),
    };
    let portable = super::remote::project(&value);
    super::remote::remove_projection(&mut value, &portable);
    Ok(Some((path, value)))
}

pub(super) fn validate(definition: &DefinitionPreferences) -> Result<()> {
    ensure!(
        !definition.name.is_empty()
            && !Path::new(&definition.name).is_absolute()
            && definition
                .name
                .split('/')
                .all(|part| !part.is_empty() && part != ".." && part != "."),
        "personal definition needs a relative name without traversal"
    );
    let values = match definition.kind {
        DefinitionKind::Skill => vec![&definition.values],
        DefinitionKind::Graph => definition
            .values
            .as_mapping()
            .context("Graph preferences must be a mapping")?
            .values()
            .collect(),
    };
    for value in values {
        super::settings::validate_preferences(value, true, true)?;
        ensure!(
            *value == super::remote::project(value),
            "definition profile contains host-local settings"
        );
    }
    Ok(())
}

fn replace_metadata(document: &mut Value, incoming: &Value) -> Result<()> {
    super::normalize_context(document);
    let previous = super::remote::project(&metadata(document)?);
    super::remote::remove_projection(document, &previous);
    super::files::overlay(document, incoming);
    Ok(())
}

fn replace(document: &mut Value, kind: DefinitionKind, incoming: &Value) -> Result<()> {
    if kind == DefinitionKind::Skill {
        return replace_metadata(document, incoming);
    }
    let mut remaining = incoming
        .as_mapping()
        .context("Graph preferences must be a mapping")?
        .clone();
    crate::graph::visit_preferences(document, "", &mut |address, value| {
        let preferences = remaining
            .remove(address)
            .unwrap_or_else(|| Value::Mapping(Default::default()));
        replace_metadata(value, &preferences)
            .map_err(|error| crate::error::LoadError::InvalidGraph(error.to_string()))
    })?;
    ensure!(remaining.is_empty(), "personal Graph is missing incoming occurrences; reconcile its definition before retaining preferences");
    Ok(())
}

pub(super) fn can_retain(path: &Path, kind: DefinitionKind, incoming: &Value) -> Result<bool> {
    let text = super::files::read_text(path)?;
    if text.is_empty() {
        return Ok(incoming.as_mapping().is_some_and(|map| map.is_empty()));
    }
    Ok(replace(&mut parse(&text, kind)?, kind, incoming).is_ok())
}

pub(super) fn update<T>(
    path: &Path,
    kind: DefinitionKind,
    change: impl FnOnce(&mut Value) -> Result<T>,
) -> Result<T> {
    super::files::update_text(path, |original| {
        let mut current = if original.is_empty() {
            Value::Mapping(Default::default())
        } else {
            projection(&mut parse(original, kind)?, kind)?
        };
        let previous = current.clone();
        let result = change(&mut current)?;
        if current == previous {
            return Ok((original.to_owned(), result));
        }
        ensure!(
            !original.is_empty(),
            "{} is missing; install its definition before retaining preferences",
            path.display()
        );
        let mut document = parse(original, kind)?;
        replace(&mut document, kind, &current)?;
        let text = match kind {
            DefinitionKind::Skill => {
                let (frontmatter, body) =
                    crate::graph::split_frontmatter(original).unwrap_or(("", original));
                format!(
                    "---\n{}---\n{body}",
                    super::files::edited_text(frontmatter, &document)?.trim_start_matches('\n')
                )
            }
            DefinitionKind::Graph => super::files::edited_text(original, &document)?,
        };
        ensure!(
            projection(&mut parse(&text, kind)?, kind)? == current,
            "definition edit changed its preference projection"
        );
        Ok((text, result))
    })
}

/// Complete coverage includes executor-only declarations and previously retained removals.
pub(super) fn destinations(
    incoming: &[DefinitionPreferences],
) -> Result<BTreeMap<PathBuf, (DefinitionKind, Value)>> {
    let mut incoming: BTreeMap<_, _> = incoming
        .iter()
        .map(|definition| ((definition.kind, definition.name.clone()), definition))
        .collect();
    let mut destinations = BTreeMap::new();
    for definition in discover()? {
        let values = incoming
            .remove(&(definition.kind, definition.name))
            .map(|other| other.values.clone())
            .unwrap_or_else(|| Value::Mapping(Default::default()));
        destinations.insert(definition.path, (definition.kind, values));
    }
    for definition in incoming.into_values() {
        // No body or topology travels. Keep a conflict receipt at the normal personal
        // destination so an explicit adoption can follow installation of the asset.
        let home = dirs::home_dir().context("personal definitions need a home directory")?;
        let path = match definition.kind {
            DefinitionKind::Skill => home
                .join(".lf/skills")
                .join(format!("{}.md", definition.name)),
            DefinitionKind::Graph => home
                .join(".lf/graphs")
                .join(format!("{}.yaml", definition.name)),
        };
        destinations.insert(path, (definition.kind, definition.values.clone()));
    }
    Ok(destinations)
}
