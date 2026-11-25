//! Definitions and Waves offered by a repository, independent of command parsing.
use crate::definition::{resolve_definition, DefinitionKind, Target};
use crate::graph::{Step, XorPath};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub name: String,
    pub kind: String,
    pub source: String,
    pub description: String,
    pub invocation: String,
    /// How a skill file relates to the builtin of the same name; none when
    /// there is no such builtin or the entry is the builtin.
    #[serde(deserialize_with = "Option::deserialize")]
    pub builtin: Option<BuiltinRelation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BuiltinRelation {
    /// `extends: builtin`: the builtin's text with the file's appended.
    Extends,
    /// A whole skill in the builtin's place, drifting as the builtin changes.
    Replaces,
    /// A whole copy, identical to the builtin today.
    Copies,
}

impl BuiltinRelation {
    pub fn describe(self) -> &'static str {
        match self {
            Self::Extends => "extends builtin",
            Self::Replaces => "replaces builtin",
            Self::Copies => "replaces builtin, same as builtin",
        }
    }
}

pub fn definition_source(repo: &Path, path: Option<&Path>) -> String {
    path.map(|path| {
        path.strip_prefix(repo)
            .unwrap_or(path)
            .display()
            .to_string()
    })
    .unwrap_or_else(|| "builtin".to_string())
}

pub fn format_written_steps(steps: &[Step]) -> String {
    if steps.is_empty() {
        return "∅".to_string();
    }
    steps
        .iter()
        .map(format_written_step)
        .collect::<Vec<_>>()
        .join(" → ")
}

fn format_written_step(step: &Step) -> String {
    match &step.target {
        Target::Graph(graph) => graph.name.clone(),
        Target::Skill(skill) if step.interactive => format!("{} (interactive)", skill.name),
        target => format_target(target),
    }
}

fn format_target(target: &Target) -> String {
    match target {
        Target::Skill(skill) => skill.name.clone(),
        Target::Breakpoint(name) => name
            .as_ref()
            .map_or_else(|| "break".into(), |name| format!("break: {name}")),
        Target::Command(command) => command.to_string(),
        Target::Graph(graph) => format_written_steps(&graph.items),
        Target::Xor(xor) => format_xor(xor.router.as_deref(), &xor.paths),
        Target::Loop(steps) => format!("loop({})", format_written_steps(steps)),
    }
}

fn format_xor(router: Option<&str>, paths: &HashMap<String, XorPath>) -> String {
    let label = router.map_or_else(|| "xor".to_string(), |name| format!("xor[{name}]"));
    let mut names: Vec<_> = paths.keys().collect();
    names.sort();
    let rendered = names
        .into_iter()
        .map(|name| format!("{name}: {}", format_written_steps(&paths[name].steps)))
        .collect::<Vec<_>>()
        .join(" | ");
    format!("{label}{{{rendered}}}")
}

fn graph_entry(repo: &Path, name: String) -> Result<Entry> {
    let kind = DefinitionKind::Graph;
    let description = match resolve_definition(repo, &name, Some(kind)) {
        Ok(target) => format_target(&target),
        Err(error) => format!("unavailable: {error}"),
    };
    Ok(Entry {
        source: definition_source(
            repo,
            crate::graph::find_graph_source_path(&name, repo)?.as_deref(),
        ),
        invocation: format!("lf graph -- {name}"),
        name,
        kind: kind.as_str().to_string(),
        description,
        builtin: None,
    })
}

pub fn discover(path: &[String], repo: &Path) -> Result<Vec<Entry>> {
    let mut entries = Vec::new();
    if path.is_empty() || path == ["wave"] {
        for name in crate::ops::linear::list_local_waves(repo)? {
            let summary = crate::work::wave::config::read_wave_summary(repo, &name)?;
            entries.push(Entry {
                source: format!(".lf/wave/{name}/GOAL.md"),
                invocation: format!("lf wave plan connect {name}"),
                name,
                kind: "wave".to_string(),
                description: if summary.is_empty() {
                    "Empty goal; edit GOAL.md before planning".to_string()
                } else {
                    summary
                },
                builtin: None,
            });
        }
        if !path.is_empty() {
            return Ok(entries);
        }
    }
    if path.is_empty() || path.first().is_some_and(|name| name == "skill") {
        let catalog = crate::skills::catalog::SkillCatalog::discover(Some(repo))?;
        let prefix = path.get(1).map(|name| format!("{name}/"));
        for source in catalog.entries() {
            let name = source.name.clone();
            if prefix
                .as_ref()
                .is_some_and(|prefix| !name.starts_with(prefix))
            {
                continue;
            }
            let description = match source.read() {
                Ok(content) => {
                    loopflow_definitions::text::skill_description(&content).unwrap_or_default()
                }
                Err(error) => format!("unavailable: {error}"),
            };
            entries.push(Entry {
                name: name.clone(),
                kind: "skill".into(),
                source: definition_source(repo, source.path.as_deref()),
                description,
                invocation: format!("lf skill -- {name}"),
                builtin: source.builtin_relation(),
            });
        }
        if !path.is_empty() {
            return Ok(entries);
        }
    }
    if path.is_empty() || path == ["graph"] {
        for name in crate::graph::available_graph_names(repo)? {
            entries.push(graph_entry(repo, name)?);
        }
        if !path.is_empty() {
            return Ok(entries);
        }
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::Entry;

    #[test]
    fn entries_match_the_shared_fixture_without_defaults() {
        let message: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/dto/server/skill_entries.json"
        ))
        .unwrap();
        let entries: Vec<Entry> = serde_json::from_value(message["result"].clone()).unwrap();
        assert_eq!(serde_json::to_value(&entries).unwrap(), message["result"]);
        assert_eq!(entries[0].invocation, "lf debug [message]");
        assert!(serde_json::from_str::<Entry>(r#"{"name":"debug","kind":"skill"}"#).is_err());
    }
}
