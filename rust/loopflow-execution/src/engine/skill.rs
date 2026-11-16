//! Start a skill on its own, detached or in this process.

use std::path::Path;

use super::{graph, launch, Result};

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
