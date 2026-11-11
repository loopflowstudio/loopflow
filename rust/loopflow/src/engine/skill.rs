//! The `skill` namespace: the skills, graphs and commands a directory's
//! repository offers, and a skill started on its own.

use std::path::Path;

use super::{graph, launch, Error, ErrorCode, Result};
use crate::lf::commands::list::Entry;

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

/// `skill.list`: what `lf list [<path>…]` shows under `path`: commands,
/// skills, graphs and Waves, each with its invocation and source.
pub fn list(directory: &Path, path: &[String]) -> Result<Vec<Entry>> {
    let repo = crate::repo::discover_repo_root(directory)
        .map_err(|error| Error::refused(&error, ErrorCode::Invalid))?
        .unwrap_or_else(|| directory.to_owned());
    crate::lf::commands::list::list_children(path, &repo)
        .map_err(|error| Error::refused(&error, ErrorCode::Unavailable))
}

#[cfg(test)]
mod tests {
    use super::Entry;

    #[test]
    fn entries_match_the_shared_fixture_without_defaults() {
        let message: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/dto/server/skill_entries.json"
        ))
        .unwrap();
        let entries: Vec<Entry> = serde_json::from_value(message["result"].clone()).unwrap();
        assert_eq!(serde_json::to_value(&entries).unwrap(), message["result"]);
        assert_eq!(entries[0].invocation, "lf debug [message]");
        assert!(serde_json::from_str::<Entry>(r#"{"name":"debug","kind":"skill"}"#).is_err());
    }
}
