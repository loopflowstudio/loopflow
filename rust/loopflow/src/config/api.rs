//! The destination-based editor used by CLI and Desktop.
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value;
use std::path::{Path, PathBuf};

use super::files::Destination;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Catalog,
    Declaration {
        destination: Destination,
        key: String,
        value: Option<String>,
        unset: bool,
    },
    Explain {
        destination: Destination,
        key: String,
    },
    /// Appearance owns its configured theme file and its Apply/Cancel transaction.
    Theme {
        path: PathBuf,
        value: Option<String>,
    },
}

pub fn run(request: Request) -> Result<serde_json::Value> {
    match request {
        Request::Catalog => Ok(serde_json::to_value(super::catalog::definitions())?),
        Request::Declaration {
            destination,
            key,
            value,
            unset,
        } => Ok(serde_json::to_value(declaration(
            &destination,
            &key,
            value.as_deref(),
            unset,
        )?)?),
        Request::Explain { destination, key } => {
            let destination = destination.resolve()?;
            super::settings::validate_scope(&key, destination.source, destination.scope)?;
            let parts: Vec<_> = key.split('.').collect();
            let preference = super::settings::preference_parts(&parts)?;
            let (skill, graph, step) = match &parts[..parts.len() - preference.len()] {
                ["skills", skill] => (Some(*skill), None, None),
                ["graphs", graph, "steps", step] => (None, Some(*graph), Some(*step)),
                _ => (None, None, None),
            };
            let config = super::load_wave_config(
                destination.repository.as_deref(),
                destination.wave.as_deref(),
            )?
            .unwrap_or_default();
            let home = crate::store::lf_home_dir();
            let rows = inspect_selection(
                &config,
                destination.repository.as_deref().unwrap_or(&home),
                skill,
                graph,
                step,
            )?;
            // Choices and fallbacks are explained together under their effective key.
            let effective_key = if preference == ["default_agent"] {
                "agent".into()
            } else {
                preference.join(".")
            };
            Ok(
                serde_json::json!({"path":destination.path, "key":effective_key, "inspections":rows}),
            )
        }
        Request::Theme { path, value } => Ok(serde_json::to_value(file_value(
            &path,
            &["theme"],
            "theme",
            value.as_deref(),
            false,
        )?)?),
    }
}

#[derive(Debug, Serialize)]
pub struct Declaration {
    pub path: PathBuf,
    pub key: String,
    pub value: Value,
    pub lifecycle: super::catalog::Lifecycle,
}

/// Read, set or unset a declaration at the same destination used by inspection.
pub fn declaration(
    destination: &Destination,
    key: &str,
    value: Option<&str>,
    unset: bool,
) -> Result<Declaration> {
    anyhow::ensure!(!(value.is_some() && unset), "choose a value or unset");
    let destination = destination.resolve()?;
    super::settings::validate_scope(key, destination.source, destination.scope)?;
    if let ("graph", Some(value), Some(repository)) =
        (key, value, destination.repository.as_deref())
    {
        let parsed = super::settings::parse_value(key, value)?;
        if let Some(name) = parsed.as_str() {
            let graph = crate::graph::load_authored_graph(name, repository)?;
            crate::graph::compile_graph(&graph, repository)?;
        }
    }
    let mut parts: Vec<_> = destination.prefix.iter().map(String::as_str).collect();
    parts.extend(key.split('.'));
    file_value(&destination.path, &parts, key, value, unset)
}

fn file_value(
    path: &Path,
    parts: &[&str],
    key: &str,
    value: Option<&str>,
    unset: bool,
) -> Result<Declaration> {
    let parsed = value
        .map(|value| super::settings::parse_value(key, value))
        .transpose()?;
    if parsed.is_some() || unset {
        super::files::edit(path, parts, parsed)?;
    }
    let document = super::files::read(path)?;
    let value = parts
        .iter()
        .try_fold(&document, |value, part| value.get(*part))
        .cloned()
        .unwrap_or(Value::Null);
    Ok(Declaration {
        path: path.to_owned(),
        key: parts.join("."),
        value,
        lifecycle: super::catalog::definition(&super::settings::preference_path(key)?.join("."))
            .expect("validated key")
            .lifecycle,
    })
}

/// The CLI and in-process Desktop read share this inspection.
pub fn inspect(
    repo: &std::path::Path,
    wave: Option<&str>,
    skill: Option<&str>,
    graph: Option<&str>,
    step: Option<&str>,
) -> anyhow::Result<Vec<serde_json::Value>> {
    anyhow::ensure!(
        skill.is_none() || graph.is_none(),
        "choose --skill or --graph"
    );
    anyhow::ensure!(step.is_none() || graph.is_some(), "--step requires --graph");
    let config = crate::config::load_wave_config(Some(repo), wave)?.unwrap_or_default();
    inspect_selection(&config, repo, skill, graph, step)
}

fn inspect_selection(
    config: &crate::config::Config,
    repo: &std::path::Path,
    skill: Option<&str>,
    graph: Option<&str>,
    step: Option<&str>,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let mut selections = Vec::new();
    if let Some(graph) = graph {
        fn collect(
            steps: &[crate::graph::ConcreteStep],
            target: Option<(&str, &str)>,
            out: &mut Vec<crate::graph::Skill>,
        ) {
            for step in steps {
                match step {
                    crate::graph::ConcreteStep::Skill(skill)
                        if target.is_none_or(|(graph, id)| {
                            skill.skill.occurrence.as_ref().is_some_and(|occurrence| {
                                occurrence.graph == graph && occurrence.id.as_deref() == Some(id)
                            })
                        }) =>
                    {
                        out.push(skill.skill.clone())
                    }
                    crate::graph::ConcreteStep::Xor(branch) => {
                        if target.is_none() {
                            out.push(branch.router.clone());
                        }
                        for path in branch.paths.values() {
                            collect(&path.steps, target, out);
                        }
                    }
                    _ => {}
                }
            }
        }
        let definition = crate::graph::load_authored_graph(graph, repo)?;
        collect(
            &crate::graph::compile_graph(&definition, repo)?,
            step.map(|id| (definition.name.as_str(), id)),
            &mut selections,
        );
        anyhow::ensure!(!selections.is_empty(), "no matching agent steps in {graph}");
    } else if let Some(skill) = skill {
        selections.push(crate::graph::load_skill(skill, repo)?);
    }
    let mut output = Vec::new();
    for selection in if selections.is_empty() {
        vec![None]
    } else {
        selections.iter().map(Some).collect()
    } {
        let resolved = crate::config::settings::resolve(config, selection)?;
        output.push(serde_json::json!({"skill": selection.map(|s| &s.name), "occurrence": selection.and_then(|s| s.occurrence.as_ref()), "settings": resolved.settings}));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::Request;
    use crate::config::files::Destination;

    #[test]
    fn catalogue_wire_shape_includes_feature_category() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/dto/configuration_catalog.json"
        ))
        .unwrap();
        let theme = crate::config::catalog::definition("theme").unwrap();
        assert_eq!(serde_json::to_value([theme]).unwrap(), fixture);
    }

    #[test]
    fn destination_wire_shape_cannot_supply_independent_file_or_scope() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/dto/configuration_destinations.json"
        ))
        .unwrap();
        let destinations: Vec<Destination> = serde_json::from_value(fixture.clone()).unwrap();
        assert_eq!(serde_json::to_value(destinations).unwrap(), fixture);
        for destination in fixture.as_array().unwrap() {
            let mut request = serde_json::json!({"action":"declaration", "destination":destination,
                "key":"agent", "value":"codex", "unset":false});
            assert!(serde_json::from_value::<Request>(request.clone()).is_ok());
            request["scope"] = "global".into();
            assert!(serde_json::from_value::<Request>(request).is_err());
            let mut invalid = destination.clone();
            invalid["path"] = "/another/config.yaml".into();
            assert!(serde_json::from_value::<Destination>(invalid).is_err());
        }
    }
}
