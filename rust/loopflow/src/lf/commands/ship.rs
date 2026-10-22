//! `lf ship`: run a shipped Task's remaining graph in the foreground until it
//! reaches `end`. At each stage a headless stand-in runs the stage's skill for
//! the person who would be there and decides the route; each edge runs as
//! `lf task run` would run it. The caller backgrounds this process; reconcile
//! restarts it if it dies.

use std::path::Path;

use anyhow::Result;

use crate::flow::output::FlowOutput;
use crate::flow::runner::SkillOutcome;
use crate::flow::transitions::{FlowDecision, FlowVerdict};
use crate::lf::Cli;
use crate::ops::task::ship::{ship_begin, ship_next, stand_in_message, ShipStart, ShipStep};
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
    loop {
        let (edge, note) = match ship_next(&issue)? {
            ShipStep::Paused => return paused(&issue),
            ShipStep::Finished => {
                println!("{issue} reached its end");
                return Ok(());
            }
            ShipStep::Edge(edge) => (edge, None),
            ShipStep::StandIn {
                stage,
                skill,
                advance,
                repeat,
            } => {
                eprintln!("{issue}: standing in at {stage} with {skill}");
                let output = repeat.as_ref().map(|_| FlowOutput::Decision);
                let mut message = stand_in_message(&stage, repeat.is_some());
                if let Some(output) = &output {
                    message.push_str(&output.instructions());
                }
                let outcome = super::flow::run_task_skill(
                    cli,
                    task.id.as_str(),
                    &worktree,
                    &skill,
                    &message,
                    output.as_ref(),
                )?;
                if ship_next(&issue)? == ShipStep::Paused {
                    return paused(&issue);
                }
                let (edge, note) = match (outcome, repeat) {
                    (
                        Some(SkillOutcome::Decided(FlowVerdict {
                            decision: FlowDecision::Iterate,
                            summary,
                        })),
                        Some(repeat),
                    ) => (repeat, format!("{stage} (stand-in) → repeat: {summary}")),
                    (
                        Some(SkillOutcome::Decided(FlowVerdict {
                            decision: FlowDecision::Blocked,
                            summary,
                        })),
                        Some(repeat),
                    ) => (
                        repeat,
                        format!("{stage} (stand-in) → repeat, blocked: {summary}"),
                    ),
                    (
                        Some(SkillOutcome::Decided(FlowVerdict {
                            decision: FlowDecision::Advance,
                            summary,
                        })),
                        _,
                    ) => (advance, format!("{stage} (stand-in) → advance: {summary}")),
                    (_, _) => (advance, format!("{stage} (stand-in) → advance")),
                };
                eprintln!("{issue}: {note}");
                (edge, Some(note))
            }
        };
        let (task, runs) = crate::ops::task::task_place(
            repo,
            &issue,
            TaskProcessOptions {
                wave: cli.wave.clone(),
                agent: cli.agent.clone(),
                flow: Some(edge),
                reason: note,
                ..TaskProcessOptions::default()
            },
        )?;
        if let Some(edge) = runs {
            super::flow::run_task_edge_for(cli, task.id.as_str(), &edge, &worktree)?;
            crate::ops::task::workflow_arrive(&task)?;
        }
    }
}

fn paused(issue: &str) -> Result<()> {
    println!("{issue} is paused; `lf task resume {issue}` and `lf ship` continue shipping");
    Ok(())
}
