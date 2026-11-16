//! Creation prints the same receipt as the shared protocol. A retry keeps its ID.
use std::io::IsTerminal;
use std::path::Path;

use anyhow::{bail, Result};

use crate::lf::TaskCommand;
use loopflow_execution::command::CommandIo;
use loopflow_execution::engine::creation::{self, TaskCreation};

pub fn create(
    repo: &Path,
    command: &TaskCommand,
    io: &mut CommandIo<'_>,
    approve: bool,
    agent: Option<&str>,
) -> Result<()> {
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
    let (receipt, after) = match created {
        Ok(created) => (created.receipt, created.after),
        Err(error) => {
            if *json {
                if let Some(result) = creation::lookup(&id).ok().flatten().and_then(|r| r.result) {
                    writeln!(io.output, "{}", serde_json::to_string_pretty(&result)?)?;
                }
            }
            return Err(error);
        }
    };
    if let Some(provider) = loopflow_execution::config::load_config_or_default(Some(repo))
        .planning_transport()
        .remote_name()
        // An admitted creation exchanges planning after its answer.
        .filter(|_| receipt.task.linear_id.is_none() && after.is_none())
    {
        writeln!(
            io.error,
            "Saved locally; pending {provider} sync. Retry with `lf task sync {}`.",
            receipt.task.identifier
        )?;
    }
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
        // The first Graph attempt identifies this launch's carrier. Batch callers
        // wait for that run, including its retries, not just its first attempt.
        if let Some(launch) = creation::lookup(&id)?.and_then(|r| r.launch) {
            if let loopflow_execution::engine::task::TaskRun::Running { graph_process, .. } =
                serde_json::from_value(launch)?
            {
                let store = loopflow_execution::store::read_existing_registry()?
                    .ok_or_else(|| anyhow::anyhow!("Machine registry is unavailable"))?;
                let carrier = store
                    .sqlite
                    .lf_process(&graph_process)?
                    .ok_or_else(|| {
                        anyhow::anyhow!("creation Graph {} is unavailable", graph_process)
                    })?
                    .parent_lf_process_id
                    .ok_or_else(|| anyhow::anyhow!("creation Graph has no Task run"))?;
                loop {
                    let process = store.sqlite.lf_process(&carrier)?.ok_or_else(|| {
                        anyhow::anyhow!("creation Task run {} is unavailable", carrier)
                    })?;
                    if process.completed_at.is_some() {
                        if process.exit_code != Some(0) {
                            bail!("Task run {} ended with {:?}", carrier, process.exit_code);
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
    // Planning exchange runs after the answer; this process waits for it.
    if let Some(after) = after {
        if after.join().is_err() {
            tracing::warn!("planning sync after creation panicked");
        }
    }
    Ok(())
}

pub fn read_report(input: &mut dyn std::io::Read) -> Result<Option<String>> {
    let mut report = String::new();
    input.read_to_string(&mut report)?;
    Ok((!report.trim().is_empty()).then_some(report))
}
