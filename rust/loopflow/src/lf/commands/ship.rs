//! `lf ship`: run a shipped Task's remaining graph until it reaches `end`. At
//! each stage a headless stand-in runs the stage's skill for the person who
//! would be there and decides the route; each edge runs as `lf task run` would
//! run it. A bare ship inside its execution owner returns once it is shipping
//! and carries on in its log; `-b` stays until the end. It is started again if
//! it dies while the Task is shipping.

use std::path::Path;

use anyhow::Result;

use crate::graph::output::GraphOutput;
use crate::graph::runner::SkillOutcome;
use crate::graph::transitions::{GraphDecision, GraphVerdict};
use crate::lf::Cli;
use crate::ops::task::ship::{
    blocked_backoff, ship_begin, ship_next, ship_progressed, ship_settle_merge, stand_in_message,
    ShipMerge, ShipStart, ShipStep,
};
use crate::ops::task::TaskProcessOptions;

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
        ShipStart::Deferred { issue } => {
            println!("{issue} is deferred; `lf task resume {issue}` before shipping it");
            return Ok(());
        }
        ShipStart::AtEnd { issue } => {
            println!("{issue} is already at its end");
            return Ok(());
        }
    };
    let issue = task.identifier.clone();
    let worktree = task.worktree()?.to_path_buf();
    if !cli.batch && !cli.interactive && crate::transport::current().is_some() {
        let log = super::graph::PendingLog::open(&format!("ship-{issue}"))?;
        println!(
            "{issue} shipping\n  follow: lf task status {issue}\n  log:    {}",
            log.path.display()
        );
        super::graph::release_launcher(&log.file)?;
    }
    // Blocked decisions in a row; each repeat after one waits longer.
    let mut blocked = 0;
    loop {
        match merged(&issue) {
            ShipMerge::Done => return Ok(()),
            ShipMerge::Verifying(_) | ShipMerge::Open => {}
        }
        let (edge, note) = match ship_next(&issue)? {
            ShipStep::Edge(edge) => (edge, None),
            ShipStep::StandIn {
                stage,
                skill,
                advance,
                repeat,
            } => {
                eprintln!("{issue}: standing in at {stage} with {skill}");
                let output = repeat.as_ref().map(|_| GraphOutput::Decision);
                let mut message = stand_in_message(&stage, repeat.is_some());
                if let Some(output) = &output {
                    message.push_str(&output.instructions());
                }
                let outcome = super::graph::run_task_skill(
                    cli,
                    task.id.as_str(),
                    &worktree,
                    &skill,
                    &message,
                    output.as_ref(),
                )?;
                ship_progressed(&task.id)?;
                // Someone may have deferred or stopped it while the stand-in ran.
                let step = ship_next(&issue)?;
                if matches!(step, ShipStep::Deferred | ShipStep::Stopped) {
                    return halt(&issue, &step);
                }
                let (decision, summary) = match outcome {
                    Some(SkillOutcome::Decided(GraphVerdict { decision, summary })) => {
                        (decision, Some(summary))
                    }
                    _ => (GraphDecision::Advance, None),
                };
                // Without a loop behind the stage, every decision advances.
                let repeat = repeat.filter(|_| decision != GraphDecision::Advance);
                blocked = match &repeat {
                    Some(_) if decision == GraphDecision::Blocked => blocked + 1,
                    _ => 0,
                };
                let choice = match (&repeat, blocked) {
                    (None, _) => "advance",
                    (Some(_), 0) => "repeat",
                    (Some(_), _) => "repeat, blocked",
                };
                let edge = repeat.unwrap_or(advance);
                let note = match summary {
                    Some(summary) => format!("{stage} (stand-in) → {choice}: {summary}"),
                    None => format!("{stage} (stand-in) → {choice}"),
                };
                eprintln!("{issue}: {note}");
                if blocked > 0 {
                    let wait = blocked_backoff(blocked);
                    eprintln!(
                        "{issue}: blocked {blocked} time(s) in a row; repeating {stage} in {} min",
                        wait.as_secs() / 60
                    );
                    if let Some(step) = wait_unless_halted(&issue, wait)? {
                        return halt(&issue, &step);
                    }
                }
                (edge, Some(note))
            }
            step => return halt(&issue, &step),
        };
        // Checked again here: a stand-in can run for a long time. A merge
        // that moved the Task replaces the route chosen before it.
        match merged(&issue) {
            ShipMerge::Done => return Ok(()),
            ShipMerge::Verifying(_) => continue,
            ShipMerge::Open => {}
        }
        let (task, runs) = crate::ops::task::task_place(
            repo,
            &issue,
            TaskProcessOptions {
                wave: cli.wave.clone(),
                agent: cli.agent.clone(),
                graph: Some(edge),
                reason: note,
                ..TaskProcessOptions::default()
            },
        )?;
        if let Some(edge) = runs {
            // This ship has already let its launcher go, or keeps it to the end.
            super::graph::run_task_edge_for(cli, &task, &edge, false)?;
            crate::ops::task::workflow_arrive(&task)?;
            ship_progressed(&task.id)?;
        }
    }
}

/// A PR merged by anyone ends the building: the Task goes on to verify it,
/// or finishes when its graph has no such stage. An unreadable PR leaves
/// shipping to continue.
fn merged(issue: &str) -> ShipMerge {
    match ship_settle_merge(issue) {
        Ok(merge) => {
            match &merge {
                ShipMerge::Done => println!("{issue}: its PR merged; completed"),
                ShipMerge::Verifying(stage) => println!("{issue}: its PR merged; on to {stage}"),
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

/// Sleep for `wait`, returning early with the step if the Task is deferred or
/// stopped meanwhile.
fn wait_unless_halted(issue: &str, wait: std::time::Duration) -> Result<Option<ShipStep>> {
    let until = std::time::Instant::now() + wait;
    while let Some(left) = until.checked_duration_since(std::time::Instant::now()) {
        std::thread::sleep(left.min(std::time::Duration::from_secs(10)));
        let step = ship_next(issue)?;
        if matches!(
            step,
            ShipStep::Deferred | ShipStep::Stopped | ShipStep::Finished
        ) {
            return Ok(Some(step));
        }
    }
    Ok(None)
}

/// Say why shipping ends here: deferred, stopped or finished.
fn halt(issue: &str, step: &ShipStep) -> Result<()> {
    match step {
        ShipStep::Deferred => {
            println!(
                "{issue} is deferred; `lf task resume {issue}` and `lf ship` continue shipping"
            )
        }
        ShipStep::Stopped => println!("{issue} stopped shipping"),
        _ => println!("{issue} reached its end"),
    }
    Ok(())
}
