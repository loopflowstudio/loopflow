//! `lf ship`: run a shipped Task's remaining graph until it reaches `end`, as
//! `lf -b task run` runs it: every step headless, an interactive step standing
//! in for the person who would be there. A bare ship inside its execution owner
//! returns once it is shipping and carries on in its log; `-b` stays until the
//! end. It is started again if it dies while the Task is shipping.

use std::path::Path;

use anyhow::Result;

use crate::lf::Cli;
use crate::ops::task::ship::{
    ship_begin, ship_next, ship_progressed, ship_settle_merge, ShipMerge, ShipStart, ShipStep,
};
use crate::ops::task::{TaskProcessOptions, TaskRunStart};

pub fn run(
    cli: &Cli,
    repo: &Path,
    issue: &str,
    graph: Option<&str>,
    reason: Option<&str>,
) -> Result<()> {
    let task = match ship_begin(issue, graph, reason)? {
        ShipStart::Begun(task) => task,
        ShipStart::AlreadyShipping { issue, process } => {
            println!("{issue} is already shipping (process {process})");
            return Ok(());
        }
        ShipStart::AtEnd { issue } => {
            println!("{issue} is already at its end");
            return Ok(());
        }
    };
    let issue = task.identifier.clone();
    if !cli.batch && !cli.interactive && crate::transport::current().is_some() {
        let log = super::graph::PendingLog::open(&format!("ship-{issue}"))?;
        println!(
            "{issue} shipping\n  follow: lf task status {issue}\n  log:    {}",
            log.path.display()
        );
        super::graph::release_launcher(&log.file)?;
    }
    // Every run is `lf -b task run`: straight through, interactive steps headless.
    let mut headless = cli.process_options();
    headless.batch = true;
    headless.interactive = false;
    loop {
        if merged(&issue) == ShipMerge::Done {
            return Ok(());
        }
        let from = match ship_next(&issue)? {
            ShipStep::Run { from } => from,
            step => return halt(&issue, &step),
        };
        if let Some(step) = &from {
            eprintln!("{issue}: standing in at {step}");
        }
        let (task, start) = crate::ops::task::task_place(
            repo,
            &issue,
            TaskProcessOptions {
                wave: cli.wave.clone(),
                agent: cli.agent.clone(),
                from,
                reason: None,
                ..TaskProcessOptions::default()
            },
            crate::ops::task::TaskRunMode::Headless,
        )?;
        if let TaskRunStart::Run(step) = start {
            // This ship has already let its launcher go, or keeps it to the end.
            super::graph::run_task_graph_for(&headless, &task, &step, false)?;
            crate::ops::task::task_run_finished(&task)?;
            ship_progressed(&task.id)?;
        }
    }
}

/// A PR merged by anyone ends the building: the Task goes on to verify it,
/// or finishes when its graph has no such step. An unreadable PR leaves
/// shipping to continue.
fn merged(issue: &str) -> ShipMerge {
    match ship_settle_merge(issue) {
        Ok(merge) => {
            match &merge {
                ShipMerge::Done => println!("{issue}: its PR merged; completed"),
                ShipMerge::Verifying(step) => println!("{issue}: its PR merged; on to {step}"),
                ShipMerge::Open => {}
            }
            merge
        }
        Err(error) => {
            eprintln!("{issue}: could not read its PR: {error}");
            ShipMerge::Open
        }
    }
}

/// Say why shipping ends here: unassigned, stopped or finished.
fn halt(issue: &str, step: &ShipStep) -> Result<()> {
    match step {
        ShipStep::Unassigned => {
            println!(
                "{issue} is unassigned; `lf task claim {issue}` and `lf ship` continue shipping"
            )
        }
        ShipStep::Stopped => println!("{issue} stopped shipping"),
        _ => println!("{issue} reached its end"),
    }
    Ok(())
}
