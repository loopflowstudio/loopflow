//! Creation prints the same receipt as the shared protocol. A retry keeps its ID.
use std::io::IsTerminal;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::lf::TaskCommand;
use loopflow_execution::command::CommandIo;
use loopflow_execution::engine::creation::{self, TaskCreation};

/// `task.create`'s answer: its receipt, the primary conversation's record and
/// the Task's row in the Work projection's shape, read when answering, so a
/// client selects the Task and opens the conversation before its planning
/// subscription lists it. The stored receipt holds neither.
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TaskCreated {
    #[serde(flatten)]
    pub receipt: creation::TaskCreateReceipt,
    #[serde(deserialize_with = "Option::deserialize")]
    pub session: Option<loopflow_execution::ops::human_session::SessionRecord>,
    pub row: super::waves::RoadmapTask,
}

/// Read the created Task's row after its commit and join it to the answer, as
/// the `row` step of request `id`'s creation.
pub(crate) fn answer(
    id: &str,
    receipt: creation::TaskCreateReceipt,
    session: Option<loopflow_execution::ops::human_session::SessionRecord>,
) -> Result<TaskCreated> {
    let _creation = tracing::info_span!(target: loopflow_execution::journal::START_TIMING_TARGET, "task_creation", request_id = id).entered();
    creation::step("row", || {
        tokio::runtime::Runtime::new()?.block_on(async {
            let store = loopflow_execution::store::read_existing_registry()?
                .map(std::sync::Arc::new)
                .context("no Loopflow store exists on this Machine")?;
            let row = super::waves::task_row(&store, &receipt.task).await?;
            Ok(TaskCreated {
                receipt,
                session,
                row,
            })
        })
    })
}

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
    let creation::Created {
        receipt,
        session,
        after,
    } = match created {
        Ok(created) => created,
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
        .filter(|_| receipt.task.tracker_id.is_none() && after.is_none())
    {
        writeln!(
            io.error,
            "Saved locally; pending {provider} sync. Retry with `lf task sync {}`.",
            receipt.task.identifier
        )?;
    }
    if *json {
        let answer = self::answer(&id, receipt.clone(), session)?;
        writeln!(io.output, "{}", serde_json::to_string_pretty(&answer)?)?;
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
                let task = loopflow_execution::store::children::resolve_task_id(
                    &store.sqlite,
                    &receipt.task.identifier,
                    None,
                )?
                .ok_or_else(|| {
                    anyhow::anyhow!("created Task {} is unknown", receipt.task.identifier)
                })?;
                // The run is over once its carrier no longer runs; its last
                // Graph run says how it went.
                while store
                    .sqlite
                    .task_graph_row(&task)?
                    .and_then(|row| row.run)
                    .is_some_and(|(_, carrier)| {
                        loopflow_execution::journal::carrier_evidence(&carrier)
                            != loopflow_execution::journal::ProcessIdentityEvidence::Dead
                    })
                {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                let last = store.sqlite.task_graphs(&task)?.pop().ok_or_else(|| {
                    anyhow::anyhow!("creation Graph {graph_process} is unavailable")
                })?;
                let result = loopflow_execution::journal::observed_result(&last.run);
                if !matches!(
                    result,
                    loopflow_storage::graph_process::ExecutionResult::Succeeded
                ) {
                    bail!(
                        "Task run ended: Graph {} {}",
                        last.id(),
                        result.outcome().unwrap_or("running")
                    );
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
        after.run();
    }
    Ok(())
}

pub fn read_report(input: &mut dyn std::io::Read) -> Result<Option<String>> {
    let mut report = String::new();
    input.read_to_string(&mut report)?;
    Ok((!report.trim().is_empty()).then_some(report))
}
