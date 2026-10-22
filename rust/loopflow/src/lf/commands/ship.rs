//! `lf ship`: run a shipped Task's remaining graph in the foreground until it
//! reaches `end`. At each stage a headless stand-in runs the stage's skill for
//! the person who would be there and decides the route; each edge runs as
//! `lf task run` would run it. The caller backgrounds this process; it is
//! started again if it dies while the Task is shipping.

use std::path::Path;

use anyhow::Result;

use crate::graph::output::GraphOutput;
use crate::graph::runner::SkillOutcome;
use crate::graph::transitions::{GraphDecision, GraphVerdict};
use crate::lf::Cli;
use crate::ops::task::ship::{
    blocked_backoff, ship_begin, ship_next, stand_in_message, ShipStart, ShipStep,
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
        ShipStart::Paused { issue } => {
            println!("{issue} is paused; `lf task resume {issue}` before shipping it");
            return Ok(());
        }
        ShipStart::AtEnd { issue } => {
            println!("{issue} is already at its end");
            return Ok(());
        }
    };
    let issue = task.identifier.clone();
    let worktree = task.worktree()?.to_path_buf();
    // Blocked decisions in a row; each repeat after one waits longer.
    let mut blocked = 0;
    loop {
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
                // Someone may have paused or stopped it while the stand-in ran.
                let step = ship_next(&issue)?;
                if matches!(step, ShipStep::Paused | ShipStep::Stopped) {
                    return halt(&issue, &step);
                }
                let verdict = match outcome {
                    Some(SkillOutcome::Decided(verdict)) => Some(verdict),
                    _ => None,
                };
                let (edge, choice) = match (verdict, repeat) {
                    (Some(GraphVerdict { decision, summary }), Some(repeat))
                        if decision != GraphDecision::Advance =>
                    {
                        if decision == GraphDecision::Blocked {
                            blocked += 1;
                            (repeat, format!("repeat, blocked: {summary}"))
                        } else {
                            blocked = 0;
                            (repeat, format!("repeat: {summary}"))
                        }
                    }
                    (Some(GraphVerdict { summary, .. }), _) => {
                        blocked = 0;
                        (advance, format!("advance: {summary}"))
                    }
                    (None, _) => {
                        blocked = 0;
                        (advance, "advance".to_string())
                    }
                };
                let note = format!("{stage} (stand-in) → {choice}");
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
            super::graph::run_task_edge_for(cli, task.id.as_str(), &edge, &worktree)?;
            crate::ops::task::workflow_arrive(&task)?;
        }
    }
}

/// Sleep for `wait`, returning early with the step if the Task is paused or
/// stopped meanwhile.
fn wait_unless_halted(issue: &str, wait: std::time::Duration) -> Result<Option<ShipStep>> {
    let until = std::time::Instant::now() + wait;
    while let Some(left) = until.checked_duration_since(std::time::Instant::now()) {
        std::thread::sleep(left.min(std::time::Duration::from_secs(10)));
        let step = ship_next(issue)?;
        if matches!(
            step,
            ShipStep::Paused | ShipStep::Stopped | ShipStep::Finished
        ) {
            return Ok(Some(step));
        }
    }
    Ok(None)
}

/// Say why shipping ends here: paused, stopped or finished.
fn halt(issue: &str, step: &ShipStep) -> Result<()> {
    match step {
        ShipStep::Paused => {
            println!("{issue} is paused; `lf task resume {issue}` and `lf ship` continue shipping")
        }
        ShipStep::Stopped => println!("{issue} stopped shipping"),
        _ => println!("{issue} reached its end"),
    }
    Ok(())
}
