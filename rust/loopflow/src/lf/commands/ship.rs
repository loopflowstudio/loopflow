//! `lf ship`, spelled `lf task auto on` too: turn auto on for the Task's
//! Graph and run it. The run stands in at each interactive step because auto
//! is on, not because of how it was launched; a Task waiting at one is stood
//! in for there.

use std::path::Path;

use anyhow::Result;

use crate::lf::Cli;
use loopflow_execution::ops::task::ship::{ship_on, ShipStart};
use loopflow_execution::ops::task::TaskProcessOptions;

/// `lf ship`: auto on, then the Task's run, reported as `task.run` reports it.
pub fn run(
    cli: &Cli,
    repo: &Path,
    issue: &str,
    graph: Option<&str>,
    reason: Option<&str>,
    json: bool,
) -> Result<()> {
    let (task, from) = match ship_on(issue, graph, reason)? {
        ShipStart::Run { task, from } => (task, from),
        ShipStart::Reported(run) => return super::graph::announce_run(&run, json),
    };
    if let Some(step) = &from {
        eprintln!("{}: standing in at {step}", task.identifier);
    }
    super::graph::carry_task_run(
        cli,
        repo,
        &task.identifier,
        TaskProcessOptions {
            wave: cli.wave.clone(),
            from,
            ..TaskProcessOptions::default()
        },
        json,
    )
}
