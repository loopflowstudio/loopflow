use std::io::{self, IsTerminal, Write};
use std::path::Path;

use anyhow::Result;

use crate::lf::navigation::{command_tree, definition_invocation, resolve_path};
use loopflow_execution::definition::DefinitionKind;
use loopflow_execution::skills::listing::Entry;

pub fn list_children(path: &[String], repo: &Path) -> Result<Vec<Entry>> {
    let tree = command_tree();
    let mut entries = loopflow_execution::skills::listing::discover(path, repo)?;
    for entry in &mut entries {
        let kind = match entry.kind.as_str() {
            "skill" => Some(DefinitionKind::Skill),
            "graph" => Some(DefinitionKind::Graph),
            _ => None,
        };
        if let Some(kind) = kind {
            entry.invocation = definition_invocation(&tree, &entry.name, kind);
        }
    }
    if path.is_empty()
        || !(path == ["wave"]
            || path == ["graph"]
            || path.first().is_some_and(|name| name == "skill"))
    {
        let (command, canonical) = resolve_path(&tree, path)?;
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
                builtin: None,
                extends: None,
            });
        }
    }
    entries.sort_by(|a, b| (&a.kind, &a.name).cmp(&(&b.kind, &b.name)));
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
            let source = entry.source_label();
            if kind != entry.kind {
                kind = entry.kind;
                output.push_str(&format!("\n{}\n", kind.to_uppercase()));
            }
            output.push_str(&format!(
                "  {:<26} {}\n    {} ({source})\n",
                entry.name,
                entry.description.lines().next().unwrap_or_default(),
                entry.invocation,
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
