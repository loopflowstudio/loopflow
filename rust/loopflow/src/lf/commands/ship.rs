//! `lf ship`, spelled `lf task auto on` too: the Task's run, asked for auto.
//! The run stands in at each interactive step because auto is on, not
//! because of how it was launched; a Task waiting at one is stood in for
//! there.

use std::path::Path;

use anyhow::Result;

use crate::lf::Cli;
use loopflow_execution::ops::task::ship::AutoAsk;
use loopflow_execution::ops::task::TaskProcessOptions;

/// `lf ship`: `lf task run` with auto on, reported as `task.run` reports it.
pub fn run(
    cli: &Cli,
    repo: &Path,
    issue: &str,
    graph: Option<&str>,
    reason: Option<&str>,
    json: bool,
) -> Result<()> {
    super::graph::carry_task_run(
        cli,
        repo,
        issue,
        TaskProcessOptions {
            wave: cli.wave.clone(),
            graph: graph.map(str::to_owned),
            auto: AutoAsk::Person {
                note: reason.map(str::to_owned),
            },
            ..TaskProcessOptions::default()
        },
        json,
    )
}
