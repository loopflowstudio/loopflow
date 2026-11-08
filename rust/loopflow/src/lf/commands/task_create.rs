//! Creation prints the same receipt as the shared protocol. A retry keeps its ID.
use std::io::IsTerminal;
use std::path::Path;

use anyhow::{bail, Result};

use super::mutation::CommandIo;
use crate::engine::creation::{self, TaskCreation};
use crate::lf::TaskCommand;
use crate::work::task::Task;

pub fn create(
    repo: &Path,
    command: &TaskCommand,
    io: &mut CommandIo<'_>,
    approve: bool,
    agent: Option<&str>,
) -> Result<Task> {
    let TaskCommand::Create {
        session,
        worktree,
        request,
        no_wave,
        wave,
        title,
        notes,
        run,
        graph,
        request_id,
        json,
    } = command
    else {
        bail!("expected task create");
    };
    let report = match request.as_ref().or(notes.as_ref()) {
        Some(text) => Some(text.clone()),
        None => read_report(io.input)?,
    };
    let id = request_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let created = creation::create(
        &id,
        TaskCreation {
            directory: repo.into(),
            title: title.clone(),
            request: report,
            wave: if *no_wave { None } else { wave.clone() },
            graph: graph.clone(),
            session: session.clone(),
            worktree: worktree.clone(),
            run: *run,
            agent: agent.map(str::to_owned),
            approve,
        },
    );
    let receipt = match created {
        Ok(receipt) => receipt,
        Err(error) => {
            if *json {
                if let Some(result) = creation::lookup(&id)?.and_then(|r| r.result) {
                    writeln!(io.output, "{}", serde_json::to_string_pretty(&result)?)?;
                }
            }
            return Err(error);
        }
    };
    if *json {
        writeln!(io.output, "{}", serde_json::to_string_pretty(&receipt)?)?;
    } else {
        writeln!(
            io.output,
            "{} · {}",
            receipt.task.title, receipt.task.identifier
        )?;
    }
    if *run && approve {
        // Batch callers wait for this exact launch, never whichever Graph is current later.
        if let Some(launch) = creation::lookup(&id)?.and_then(|r| r.launch) {
            if let crate::engine::task::TaskRun::Running { graph_process, .. } =
                serde_json::from_value(launch)?
            {
                let store = crate::store::read_existing_registry()?
                    .ok_or_else(|| anyhow::anyhow!("Machine registry is unavailable"))?;
                loop {
                    let process = store.sqlite.lf_process(&graph_process)?.ok_or_else(|| {
                        anyhow::anyhow!("creation Graph {} is unavailable", graph_process)
                    })?;
                    if process.completed_at.is_some() {
                        if process.exit_code != Some(0) {
                            bail!(
                                "Task Graph {} ended with {:?}",
                                graph_process,
                                process.exit_code
                            );
                        }
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
            }
        }
    }
    if *run && !approve && receipt.session_id.is_some() {
        if !*json && std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
            let status = std::process::Command::new(std::env::current_exe()?)
                .args(["resume", "--task", &receipt.task.identifier])
                .current_dir(repo)
                .status()?;
            if !status.success() {
                bail!(
                    "open its conversation with `lf resume --task {}`",
                    receipt.task.identifier
                );
            }
        } else if !*json {
            writeln!(
                io.output,
                "Open its conversation: lf resume --task {}",
                receipt.task.identifier
            )?;
        }
    }
    Ok(receipt.task)
}

pub fn read_report(input: &mut dyn std::io::Read) -> Result<Option<String>> {
    let mut report = String::new();
    input.read_to_string(&mut report)?;
    Ok((!report.trim().is_empty()).then_some(report))
}
