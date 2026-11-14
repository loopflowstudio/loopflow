//! `lf ship`: run a shipped Task's remaining graph until it reaches `end`, as
//! `lf -b task run` runs it: every step headless, an interactive step standing
//! in for the person who would be there. A bare ship inside its execution owner
//! returns once it is shipping and carries on in its log; `-b` stays until the
//! end. It is started again if it dies while shipping, unless a provider refusal or
//! repeated identical failure stopped its run: then it stops shipping with it.

use std::path::Path;

use anyhow::Result;

use crate::engine::task::TaskShip;
use crate::lf::Cli;
use crate::ops::task::ship::{
    ship_begin, ship_next, ship_progressed, ship_settle_merge, ShipMerge, ShipStart, ShipStep,
};
use crate::ops::task::{TaskProcessOptions, TaskRunStart};

/// `lf ship`: approve Auto-Ship and ship the Task here, reporting what it set
/// going once, as `task.ship` returns it.
pub fn run(
    cli: &Cli,
    repo: &Path,
    issue: &str,
    graph: Option<&str>,
    reason: Option<&str>,
    json: bool,
) -> Result<()> {
    let task = match ship_begin(issue, graph, reason)? {
        ShipStart::Begun(task) => task,
        ShipStart::AlreadyShipping { issue, process } => {
            return announce(&TaskShip::AlreadyShipping { issue, process }, json)
        }
        ShipStart::AtEnd { issue } => return announce(&TaskShip::AtEnd { issue }, json),
    };
    let issue = task.identifier.clone();
    let process = crate::journal::current_lf_process_id()
        .expect("a ship that began has a registered Process");
    let letting_go = !cli.batch && !cli.interactive && crate::transport::current().is_some();
    let log = letting_go
        .then(|| super::graph::PendingLog::open(&format!("ship-{issue}")))
        .transpose()?;
    let shipping = TaskShip::Shipping {
        issue: issue.clone(),
        process,
        log: log.as_ref().map(|log| log.path.clone()),
    };
    announce(&shipping, json)?;
    if let Some(log) = &log {
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
                from,
                reason: None,
                ..TaskProcessOptions::default()
            },
            crate::ops::task::TaskRunMode::Headless,
        )?;
        // A Task that already has a conversation keeps it; this starts one only once.
        crate::ops::task::start_conversation_beside(&task);
        if let TaskRunStart::Run { step, .. } = start {
            // This ship has already let its launcher go, or keeps it to the end.
            super::graph::run_task_graph_for(&headless, &task, &step, false, &mut |_, _| Ok(()))?;
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

/// Report what this ship set going to its launcher and print it.
fn announce(shipping: &TaskShip, json: bool) -> Result<()> {
    crate::engine::launch::report(Ok(shipping));
    if json {
        println!("{}", serde_json::to_string(shipping)?);
    } else if let Some(text) = shipping.render() {
        println!("{text}");
    }
    Ok(())
}
