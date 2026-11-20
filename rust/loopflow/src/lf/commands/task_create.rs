//! Creation prints the same receipt as the shared protocol. A retry keeps its ID.
use std::io::IsTerminal;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::lf::TaskCommand;
use loopflow_execution::command::CommandIo;
use loopflow_execution::engine::creation::{self, TaskCreation};
use loopflow_storage::graph_process::{Execution, ExecutionResult};
use loopflow_storage::sqlite::SqliteStore;

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
        .map(|tracker| tracker.name())
        // An admitted creation exchanges planning after its answer.
        .filter(|_| {
            receipt.task.tracker_id.is_none() && receipt.task.goes_to_tracker() && after.is_none()
        })
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
            "{} · {}{}",
            receipt.task.title,
            receipt.task.identifier,
            loopflow_execution::ops::task::joined_project_suffix(&receipt.task.id)
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
                // The carrier stays live between attempts. Successful arrival
                // can clear it before the last Graph records its result.
                let (last, result) = loop {
                    let carrying = store
                        .sqlite
                        .task_graph_row(&task)?
                        .and_then(|row| row.run)
                        .is_some_and(|(_, carrier)| {
                            loopflow_execution::process::os_process::carrier_evidence(&carrier)
                                != loopflow_execution::store::ProcessIdentityEvidence::Dead
                        });
                    if !carrying {
                        let last = store.sqlite.task_graphs(&task)?.pop().ok_or_else(|| {
                            anyhow::anyhow!("creation Graph {graph_process} is unavailable")
                        })?;
                        let result = graph_result(&store.sqlite, &last.run)?;
                        if result != ExecutionResult::Running {
                            break (last, result);
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                };
                if result != ExecutionResult::Succeeded {
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

fn graph_result(store: &SqliteStore, run: &Execution) -> Result<ExecutionResult> {
    let result = loopflow_execution::process::os_process::observed_result(run);
    if result == ExecutionResult::Unknown && run.ended_at.is_none() {
        // The Graph can record its result and exit between our SQLite and OS reads.
        // Read that same run after observing its exit before reporting it as unknown.
        return Ok(store
            .graph_process(run.id.as_str())?
            .map(|(graph, _)| graph.run.result())
            .filter(|result| *result != ExecutionResult::Running)
            .unwrap_or(ExecutionResult::Unknown));
    }
    Ok(result)
}

pub fn read_report(input: &mut dyn std::io::Read) -> Result<Option<String>> {
    let mut report = String::new();
    input.read_to_string(&mut report)?;
    Ok((!report.trim().is_empty()).then_some(report))
}

#[cfg(test)]
mod tests {
    use std::process::{Command, Stdio};

    use loopflow_model::id::LfProcessId;
    use loopflow_storage::graph::CompiledGraph;
    use loopflow_storage::graph_process::{ExecutionResult, ExecutionStart, RunOwner};
    use loopflow_storage::rows::now_unix;

    #[test]
    fn graph_exit_uses_the_result_written_since_the_last_read() {
        let home = tempfile::tempdir().unwrap();
        let store =
            loopflow_execution::store::opening::open_ephemeral(&home.path().join("db")).unwrap();
        let id = LfProcessId::new();
        let mut child = Command::new("/bin/cat")
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();
        let started_at = now_unix();
        let start = ExecutionStart {
            started_at,
            pid: Some(child.id()),
            os_started_at: Some(
                loopflow_execution::process::os_process::process_started_at(child.id())
                    .unwrap()
                    .unwrap(),
            ),
            caller_session: None,
        };
        store
            .record_graph(
                &id,
                &CompiledGraph {
                    name: "gate".into(),
                    steps: vec![],
                },
                None,
                &start,
                None,
                None,
                None,
            )
            .unwrap();
        let (graph, _) = store.graph_process(id.as_str()).unwrap().unwrap();
        store
            .finish_run(RunOwner::GraphRun, &id, now_unix(), Some(0), None)
            .unwrap();
        assert!(child.wait().unwrap().success());

        assert_eq!(
            loopflow_execution::process::os_process::observed_result(&graph.run),
            ExecutionResult::Unknown
        );
        assert_eq!(
            super::graph_result(&store, &graph.run).unwrap(),
            ExecutionResult::Succeeded
        );
    }
}
