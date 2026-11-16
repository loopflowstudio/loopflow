//! The `skill` namespace: the skills, graphs and commands a directory's
//! repository offers, and a skill started on its own.

use std::path::Path;

use super::{graph, launch, Result};
use crate::skills::listing::Entry;

/// `skill.run`: start `lf -b [--agent <agent>] skill <name> [message]`
/// detached and return the Process carrying it. The terminal form keeps its
/// conversation; the served run is headless.
pub fn run(
    directory: &Path,
    name: &str,
    message: Option<&str>,
    agent: Option<&str>,
) -> Result<launch::Launched> {
    launch::start(directory, &graph::spell("skill", name, message, agent))
}

/// `lf <skill>` in this process: report the Process to the launcher that
/// started it detached, if one did, then run in a log of its own.
pub fn carry(name: &str) -> anyhow::Result<Option<launch::Launched>> {
    launch::carry("skill.run", format!("skill {name}"))
}

/// `skill.list`: combine repository definitions and Waves with the adapter’s
/// command catalogue, retaining invocation spellings and ordering.
pub fn list(mut entries: Vec<Entry>, commands: Vec<Entry>) -> Vec<Entry> {
    entries.extend(commands);
    entries.sort_by(|a, b| (&a.kind, &a.name).cmp(&(&b.kind, &b.name)));
    entries
}

#[cfg(test)]
mod tests {
    use super::{list, Entry};

    #[test]
    fn listing_keeps_both_kinds_and_the_adapters_invocation_spelling() {
        let definition = Entry {
            name: "list".into(),
            kind: "skill".into(),
            source: ".lf/skills/list.md".into(),
            description: "unavailable: unreadable source".into(),
            invocation: "lf skill -- list".into(),
        };
        let command = Entry {
            name: "list".into(),
            kind: "command".into(),
            source: "builtin".into(),
            description: "List available entries".into(),
            invocation: "lf skill list".into(),
        };
        assert_eq!(
            list(vec![definition.clone()], vec![command.clone()]),
            vec![command, definition]
        );
    }
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
