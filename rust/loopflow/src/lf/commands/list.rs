use std::collections::BTreeSet;
use std::io::{self, IsTerminal, Write};
use std::path::Path;

use anyhow::Result;
use clap::Command;
use serde::Serialize;

use crate::lf::discovery::{
    definition_source, list_all_skills, resolve_local_definition, DefinitionKind, Target,
};
use crate::lf::navigation::{command_tree, definition_invocation, resolve_path};

#[derive(Debug, Serialize)]
pub struct Entry {
    pub name: String,
    pub kind: String,
    pub source: String,
    pub description: String,
    pub invocation: String,
}

fn definition_entry(tree: &Command, repo: &Path, name: String, kind: DefinitionKind) -> Entry {
    let description = match resolve_local_definition(repo, &name, Some(kind)) {
        Ok(Target::Skill(skill)) => skill
            .content
            .unwrap_or_default()
            .lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or_default()
            .trim_start_matches('#')
            .trim()
            .to_string(),
        Ok(target) => crate::lf::discovery::format_target(&target),
        Err(error) => format!("unavailable: {error}"),
    };
    Entry {
        source: definition_source(repo, &name, kind),
        invocation: definition_invocation(tree, &name, kind),
        name,
        kind: kind.as_str().to_string(),
        description,
    }
}

pub fn list_children(path: &[String], repo: &Path) -> Result<Vec<Entry>> {
    let mut entries = collect_entries(&command_tree(), path, repo)?;
    entries.sort_by(|a, b| (&a.kind, &a.name).cmp(&(&b.kind, &b.name)));
    Ok(entries)
}

fn collect_entries(tree: &Command, path: &[String], repo: &Path) -> Result<Vec<Entry>> {
    let mut entries = Vec::new();
    if path.is_empty() || path.first().is_some_and(|name| name == "skill") {
        let (local, global, builtin, external) = list_all_skills(Some(repo));
        let names: BTreeSet<_> = local
            .into_iter()
            .chain(global)
            .chain(builtin)
            .chain(external.into_iter().map(|(name, _)| name))
            .collect();
        let namespace = path.get(1).map(|name| format!("{name}/"));
        let mut namespaces = BTreeSet::new();
        for name in names {
            if namespace
                .as_ref()
                .is_some_and(|prefix| !name.starts_with(prefix))
            {
                continue;
            }
            if path.len() == 1 {
                if let Some((namespace, _)) = name.split_once('/') {
                    if namespaces.insert(namespace.to_string()) {
                        entries.push(Entry {
                            name: namespace.to_string(),
                            kind: "namespace".to_string(),
                            source: "skills".to_string(),
                            description: "List skills in this namespace".to_string(),
                            invocation: format!("lf skill list {namespace}"),
                        });
                    }
                    continue;
                }
            }
            entries.push(definition_entry(tree, repo, name, DefinitionKind::Skill));
        }
        if !path.is_empty() {
            return Ok(entries);
        }
    }
    if path.is_empty() || path == ["flow"] {
        for name in crate::engine::available_flow_names(repo) {
            entries.push(definition_entry(tree, repo, name, DefinitionKind::Flow));
        }
        if !path.is_empty() {
            return Ok(entries);
        }
    }
    let (command, canonical) = resolve_path(tree, path)?;
    for child in command
        .get_subcommands()
        .filter(|child| !child.is_hide_set())
    {
        let mut child_path = canonical.clone();
        child_path.push(child.get_name().to_string());
        entries.push(Entry {
            name: child.get_name().to_string(),
            kind: "command".to_string(),
            source: "builtin".to_string(),
            description: child
                .get_about()
                .map(ToString::to_string)
                .unwrap_or_default(),
            invocation: format!("lf {}", child_path.join(" ")),
        });
    }
    Ok(entries)
}

pub fn show(path: &[String], repo: &Path, json: bool) -> Result<()> {
    let entries = list_children(path, repo)?;
    let output = if json {
        format!("{}\n", serde_json::to_string(&entries)?)
    } else {
        let mut output = String::new();
        let mut kind = String::new();
        for entry in entries {
            if kind != entry.kind {
                kind = entry.kind;
                output.push_str(&format!("\n{}\n", kind.to_uppercase()));
            }
            output.push_str(&format!(
                "  {:<26} {}\n    {} ({})\n",
                entry.name,
                entry.description.lines().next().unwrap_or_default(),
                entry.invocation,
                entry.source
            ));
        }
        output
    };
    let stdout = io::stdout();
    let output = if stdout.is_terminal() {
        output.replace('\n', "\r\n")
    } else {
        output
    };
    stdout.lock().write_all(output.as_bytes())?;
    Ok(())
}
