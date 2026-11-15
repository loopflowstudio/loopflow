//! The `graph` namespace's run: a Graph started on its own, outside a Task.
//! `lf task run` keeps its Task spelling in [`super::task::run`]; both start
//! the run detached through [`super::launch`].

use std::path::Path;

use super::{launch, Result};

/// `graph.run`: start `lf -b [--agent <agent>] graph <name> [message]`
/// detached and return the Process carrying it. Served runs are headless:
/// every interactive step runs without a person. A Task checkout binds the
/// run to its Task as the terminal form does; nothing approves Auto-Ship.
pub fn run(
    directory: &Path,
    name: &str,
    message: Option<&str>,
    agent: Option<&str>,
) -> Result<launch::Launched> {
    launch::start(directory, &spell("graph", name, message, agent))
}

/// `lf graph <name>` in this process: report the Process to the launcher
/// that started it detached, if one did, then run in a log of its own.
pub fn carry(name: &str) -> anyhow::Result<Option<launch::Launched>> {
    launch::carry("graph.run", format!("graph {name}"))
}

pub(crate) fn spell(
    noun: &str,
    name: &str,
    message: Option<&str>,
    agent: Option<&str>,
) -> Vec<String> {
    let mut args = vec!["--batch".to_owned()];
    if let Some(agent) = agent {
        args.extend(["--agent".to_owned(), agent.to_owned()]);
    }
    args.extend([noun.to_owned(), name.to_owned()]);
    args.extend(message.map(str::to_owned));
    args
}

#[cfg(test)]
mod tests {
    use super::spell;

    #[test]
    fn a_served_run_is_headless_and_names_its_kind() {
        assert_eq!(
            spell("graph", "code", Some("Keep it small"), Some("claude")),
            [
                "--batch",
                "--agent",
                "claude",
                "graph",
                "code",
                "Keep it small"
            ]
        );
        assert_eq!(
            spell("skill", "debug", None, None),
            ["--batch", "skill", "debug"]
        );
    }
}
