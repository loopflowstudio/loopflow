//! `lf ship`: run a shipped Task's remaining Workflow in the foreground until
//! it reaches `end`. At each node a headless stand-in runs the node's skill for
//! the person who would be there; each edge runs as `lf task run` would run it.
//! The caller backgrounds this process; reconcile restarts it if it dies.

use std::path::Path;

use anyhow::Result;

use crate::lf::Cli;
use crate::ops::task::ship::{ship_begin, ship_next, stand_in_message, ShipStart, ShipStep};
use crate::ops::task::TaskProcessOptions;
use crate::workflow::END;

pub fn run(
    cli: &Cli,
    repo: &Path,
    issue: &str,
    flow: Option<&str>,
    reason: Option<&str>,
) -> Result<()> {
    let task = match ship_begin(issue, reason)? {
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
    if let Some(flow) = flow {
        super::flow::run_for_task(cli, task.id.as_str(), flow, &worktree, None)?;
        let note = format!("shipped with {flow}");
        println!(
            "{}",
            crate::ops::task::workflow_set(&issue, END, Some(&note))?
        );
        return Ok(());
    }
    loop {
        let edge = match ship_next(&issue)? {
            ShipStep::Paused => return paused(&issue),
            ShipStep::Finished => {
                println!("{issue} reached its end");
                return Ok(());
            }
            ShipStep::Edge(edge) => edge,
            ShipStep::StandIn { stage, skill, edge } => {
                eprintln!("{issue}: standing in at {stage} with {skill}");
                let message = stand_in_message(&stage);
                super::flow::run_for_task(
                    cli,
                    task.id.as_str(),
                    &skill,
                    &worktree,
                    Some(&message),
                )?;
                if ship_next(&issue)? == ShipStep::Paused {
                    return paused(&issue);
                }
                edge
            }
        };
        let (task, flow) = crate::ops::task::task_place(
            repo,
            &issue,
            TaskProcessOptions {
                wave: cli.wave.clone(),
                agent: cli.agent.clone(),
                flow: Some(edge),
                ..TaskProcessOptions::default()
            },
        )?;
        if let Some(flow) = flow {
            super::flow::run_for_task(cli, task.id.as_str(), &flow, &worktree, None)?;
            crate::ops::task::workflow_arrive(&task)?;
        }
    }
}

fn paused(issue: &str) -> Result<()> {
    println!("{issue} is paused; `lf task resume {issue}` and `lf ship` continue shipping");
    Ok(())
}
