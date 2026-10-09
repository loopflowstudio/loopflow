use std::io::{self, IsTerminal, Write};
use std::path::Path;

use anyhow::Result;
use clap::Command;
use serde::Serialize;

use crate::engine::target::{resolve_definition, DefinitionKind};
use crate::lf::navigation::{
    command_tree, definition_invocation, definition_source, format_target, resolve_path,
};

#[derive(Debug, Serialize)]
pub struct Entry {
    pub name: String,
    pub kind: String,
    pub source: String,
    pub description: String,
    pub invocation: String,
}

fn flow_entry(tree: &Command, repo: &Path, name: String) -> Result<Entry> {
    let kind = DefinitionKind::Flow;
    let description = match resolve_definition(repo, &name, Some(kind)) {
        Ok(target) => format_target(&target),
        Err(error) => format!("unavailable: {error}"),
    };
    Ok(Entry {
        source: definition_source(
            repo,
            crate::engine::flow::find_flow_source_path(&name, repo)?.as_deref(),
        ),
        invocation: definition_invocation(tree, &name, kind),
        name,
        kind: kind.as_str().to_string(),
        description,
    })
}

pub fn list_children(path: &[String], repo: &Path) -> Result<Vec<Entry>> {
    let mut entries = collect_entries(&command_tree(), path, repo)?;
    entries.sort_by(|a, b| (&a.kind, &a.name).cmp(&(&b.kind, &b.name)));
    Ok(entries)
}

fn collect_entries(tree: &Command, path: &[String], repo: &Path) -> Result<Vec<Entry>> {
    let mut entries = Vec::new();
    if path.is_empty() || path == ["wave"] {
        for name in crate::ops::pm::list_local_waves(repo)? {
            let summary = crate::work::wave::config::read_wave_summary(repo, &name)?;
            entries.push(Entry {
                source: format!("wave/{name}/GOAL.md"),
                invocation: format!("lf repo connect {name}"),
                name,
                kind: "wave".to_string(),
                description: if summary.is_empty() {
                    "Empty goal; edit GOAL.md before planning".to_string()
                } else {
                    summary
                },
            });
        }
        if !path.is_empty() {
            return Ok(entries);
        }
    }
    if path.is_empty() || path.first().is_some_and(|name| name == "skill") {
        let catalog = crate::engine::skill_catalog::SkillCatalog::discover(Some(repo))?;
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
                    crate::engine::skills::skill_description(&content).unwrap_or_default()
                }
                Err(error) => format!("unavailable: {error}"),
            };
            entries.push(Entry {
                name: name.clone(),
                kind: "skill".into(),
                source: definition_source(repo, source.path.as_deref()),
                description,
                invocation: definition_invocation(tree, &name, DefinitionKind::Skill),
            });
        }
        if !path.is_empty() {
            return Ok(entries);
        }
    }
    if path.is_empty() || path == ["flow"] {
        for name in crate::engine::available_flow_names(repo)? {
            entries.push(flow_entry(tree, repo, name)?);
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
