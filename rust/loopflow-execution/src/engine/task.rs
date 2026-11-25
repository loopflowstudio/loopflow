//! The `task` namespace's core loop: run, auto, steer, report, wait and
//! interrupt a Task, and claim or unassign it.
//!
//! A run and an auto on outlive the request that starts them. From a terminal the
//! `lf` process is their carrier; a served request has none, so [`run`] and
//! [`auto_on`] start the same command detached ([`super::launch`]) and return the
//! result it reports, the value its CLI spelling prints with `--json`. Each
//! runs here: the caller sends a Task another Machine runs to that Machine,
//! whose server starts it with the `environment` the sender carried.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::{launch, Error, ErrorCode, Result};
use crate::ops::task::{TaskControlResult, TaskSnapshot, TaskWaitUntil};
use crate::ops::task_ownership::OwnershipChange;
use crate::read_stream::{Subscription, Transport};
use loopflow_model::comment::TaskComments;
use loopflow_model::durable::TaskOwnership;
use loopflow_model::id::LfProcessId;

/// What `task.run` started.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum TaskRun {
    /// The run's Graph process registered; it goes on without its launcher.
    Running {
        issue: String,
        /// The graph and where the run set out: `code from implement`.
        run: String,
        /// The run may merge the PR: auto is on, or no interactive step is
        /// left ahead, and nobody turned auto off.
        merges: bool,
        graph_process: LfProcessId,
        /// Where the run's output goes once its launcher lets go; null while a
        /// terminal (`-b`, `-i`) keeps it.
        log: Option<PathBuf>,
    },
    /// Nothing runs: the Task now waits at this interactive step.
    Waiting { issue: String, step: String },
    /// Nothing remained: the Task is at `end`.
    Ended { issue: String },
}

impl TaskRun {
    /// What `lf task run` prints; a run its terminal keeps prints nothing here.
    pub fn render(&self) -> Option<String> {
        match self {
            Self::Running {
                issue,
                run,
                merges,
                graph_process,
                log: Some(log),
            } => Some(format!(
                "{issue} · {run} running as Graph run {graph_process}{}\n  follow: lf task status {issue}\n  log:    {}",
                if *merges {
                    format!("\n  merges: this run may merge the PR; `lf task auto off {issue}` stops it")
                } else {
                    String::new()
                },
                log.display()
            )),
            Self::Running { log: None, .. } => None,
            Self::Waiting { issue, step } => Some(format!("Task {issue} waits at {step}")),
            Self::Ended { issue } => Some(format!("Task {issue} reached the end of its graph")),
        }
    }
}

/// What `task.auto.off` turned off.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AutoOff {
    pub issue: String,
    /// Auto was on before this.
    pub was_on: bool,
}

impl AutoOff {
    pub fn render(&self) -> String {
        format!("{}: auto off", self.issue)
    }
}

/// One reading of `task.wait`: the Task now, and whether it reached the state waited for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Waited {
    pub reached: bool,
    pub task: TaskSnapshot,
}

/// `task.run`: start the Task's run detached, as `lf [-b] [-a <agent>] task
/// run <issue> [--from <step>]` does, and return once it is known. `approve`
/// runs every step headless to `end`, as `-b` does, and writes no auto;
/// without it the run stops before the next interactive step. With no
/// `agent`, each step's harness is the repository's configured default.
pub fn run(
    directory: &Path,
    issue: &str,
    from: Option<&str>,
    graph: Option<&str>,
    agent: Option<&str>,
    approve: bool,
    environment: &[(String, String)],
) -> Result<TaskRun> {
    launch::start_with(
        directory,
        &spell_run(issue, from, graph, agent, approve),
        environment,
    )
}

/// `task.auto.on`: turn auto on and start the Task's run detached, as
/// `lf task auto on <issue>` does: it returns what `task.run` returns, and the
/// run stands in at each interactive step until the PR lands. It is the one
/// way to turn auto on.
pub fn auto_on(directory: &Path, issue: &str, environment: &[(String, String)]) -> Result<TaskRun> {
    launch::start_with(directory, &spell_auto_on(issue), environment)
}

/// `task.auto.off`: turn the Task's auto setting off, as `lf task auto off
/// <issue>` does. Nothing is killed: the run waits at its next interactive or
/// merging step until `task.auto.on` turns auto on again.
pub fn auto_off(issue: &str, reason: Option<&str>) -> Result<AutoOff> {
    let was_on = crate::ops::task::auto::off(issue, reason)
        .map_err(written)?
        .ok_or_else(|| missing(issue))?;
    Ok(AutoOff {
        issue: issue.to_owned(),
        was_on,
    })
}

/// Where the steer has been saved so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SteerReached {
    Owner,
    Shared,
    Here,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SteerDelivery {
    pub reached: SteerReached,
    #[serde(deserialize_with = "Option::deserialize")]
    pub runs_on: Option<loopflow_model::durable::MachineRef>,
    pub sentence: String,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TaskSteerReceipt {
    pub identifier: String,
    pub steer_id: loopflow_model::id::SteerId,
    pub delivery: SteerDelivery,
}

/// Save a direction once. When the owner could not be reached, wait for the
/// exchange and read the acknowledgement for this comment before answering.
pub async fn steer(
    store: &crate::store::Store,
    task: &crate::work::task::Task,
    message: &str,
    participant: Option<String>,
    elsewhere: Option<(loopflow_model::durable::MachineRef, String)>,
) -> anyhow::Result<TaskSteerReceipt> {
    use crate::ops::{
        planning_sync,
        task::{append_task_comment_as, repository_for_task},
    };
    use loopflow_storage::sqlite::tracker_actions::ActionOwner;
    let repo = repository_for_task(store, task)?;
    let repo = repo.to_string_lossy();
    let owner = ActionOwner::Task(task.id.clone());
    // Callers validate before any peer is dialed; the append validates again.
    let comment = append_task_comment_as(store, task, message, true, || Ok(participant))?;
    let steer = store
        .sqlite
        .steer_by_source(&format!("comment:{comment}"), "local")?
        .ok_or_else(|| {
            Error::new(
                ErrorCode::OutcomeUnknown,
                format!("comment {comment} derived no steer"),
            )
        })?;
    let id = &steer.id;
    let delivery = match elsewhere {
        None => {
            planning_sync::sync_after_save(store, &repo, Some(&owner)).await;
            let reader = super::session::task_steer_reader(&store.sqlite, task)?;
            SteerDelivery {
                reached: SteerReached::Owner,
                runs_on: None,
                sentence: format!("Saved {id}; {reader}"),
            }
        }
        Some((machine, reason)) => {
            planning_sync::sync_saved(store, &repo, Some(&owner)).await;
            let name = machine.name();
            let carrier = store.sqlite.comment_carrier(&task.id, &comment)?;
            let (reached, sentence) = match carrier {
                Some(carrier) => {
                    let carrier = match carrier.as_str() {
                        "linear" => "Linear",
                        "asana" => "Asana",
                        "git" => "Git",
                        "hub" => "the hub",
                        other => other,
                    };
                    let next = if store.sqlite.task_state(&task.id)?.is_terminal() {
                        format!("{} is finished", task.identifier)
                    } else {
                        format!("{name} reads it when it next syncs")
                    };
                    (
                        SteerReached::Shared,
                        format!("Queued {id} in {carrier} for {name}; {reason}. {next}"),
                    )
                }
                None => (
                    SteerReached::Here,
                    format!("Saved {id} here for {name}; {reason}. Sync is pending."),
                ),
            };
            SteerDelivery {
                reached,
                runs_on: Some(machine),
                sentence,
            }
        }
    };
    Ok(TaskSteerReceipt {
        identifier: task.identifier.clone(),
        steer_id: steer.id,
        delivery,
    })
}

/// `task.report`: progress for the Task's thread, as `lf task report <issue>
/// <message>` does. An agent's report carries its Session's provenance
/// (`<!-- loopflow-progress:… -->`) and steers nobody. Returns the thread
/// with the entry.
pub fn report(
    directory: &Path,
    issue: &str,
    wave: Option<&str>,
    message: &str,
) -> Result<TaskComments> {
    if message.trim().is_empty() {
        return Err(Error::new(
            ErrorCode::Invalid,
            "Task comment cannot be empty",
        ));
    }
    let repo = crate::ops::task::task_repository(directory, Some(issue)).map_err(written)?;
    crate::ops::task::task_comment(&repo, issue, wave, Some(message), false).map_err(written)
}

/// `task.interrupt`: stop the Task's live Graph or Task run and its scheduled
/// automation, as `lf task interrupt <issue>` does. The receipt says what the
/// interrupt reached, or that nothing ran.
pub fn interrupt(issue: &str) -> Result<TaskControlResult> {
    crate::ops::task::task_interrupt(issue).map_err(written)
}

/// `task.claim` / `task.unassign`, here: the caller sends a Task whose
/// ownership another Machine writes to that Machine.
pub fn ownership(directory: &Path, issue: &str, change: OwnershipChange) -> Result<TaskOwnership> {
    let repo = crate::ops::task::task_repository(directory, Some(issue)).map_err(written)?;
    crate::ops::task_ownership::change(&repo, issue, change).map_err(written)
}

/// One `task.wait` reading: the Task now and whether it reached `until`.
pub fn waited(issue: &str, until: TaskWaitUntil) -> Result<Waited> {
    match crate::ops::task::task_wait_now(issue, until) {
        Ok(Some((task, reached))) => Ok(Waited { reached, task }),
        Ok(None) => Err(missing(issue)),
        Err(error) => Err(Error::refused(&error.into(), ErrorCode::Unavailable)),
    }
}

/// `lf task wait`: read until the Task reaches `until` or `timeout` passes,
/// and return the last reading.
pub fn wait(issue: &str, until: TaskWaitUntil, timeout: Option<Duration>) -> Result<Waited> {
    match crate::ops::task::task_wait(issue, until, timeout) {
        Ok(Some((task, reached))) => Ok(Waited { reached, task }),
        Ok(None) => Err(missing(issue)),
        Err(error) => Err(Error::refused(&error.into(), ErrorCode::Unavailable)),
    }
}

/// `task.wait` as a subscription: a reading whenever the Task changes, the
/// last one `reached`.
pub fn subscribe_wait(
    issue: String,
    until: TaskWaitUntil,
    transport: Transport,
) -> anyhow::Result<Option<Subscription>> {
    crate::read_stream::start(
        crate::read_stream::Watch::State,
        (issue, until),
        |store, (issue, until): (String, TaskWaitUntil)| async move {
            let store = crate::read_stream::require_store(store)?;
            match crate::ops::task::task_wait_reading(&store, &issue, until).await? {
                Some((task, reached)) => Ok(Waited { reached, task }),
                None => Err(missing(&issue).into()),
            }
        },
        transport,
    )
}

fn written(error: impl Into<anyhow::Error>) -> Error {
    Error::written(&error.into())
}

fn missing(issue: &str) -> Error {
    Error::new(ErrorCode::NotFound, format!("no Task exists for {issue:?}"))
        .with(serde_json::json!({ "task": issue }))
}

pub fn spell_run(
    issue: &str,
    from: Option<&str>,
    graph: Option<&str>,
    agent: Option<&str>,
    approve: bool,
) -> Vec<String> {
    let mut args = Vec::new();
    if approve {
        args.push("--batch".to_owned());
    }
    if let Some(agent) = agent {
        args.extend(["--agent".to_owned(), agent.to_owned()]);
    }
    args.extend(["task", "run", issue].map(str::to_owned));
    args.extend(graph.map(str::to_owned));
    if let Some(from) = from {
        args.extend(["--from".to_owned(), from.to_owned()]);
    }
    args.push("--json".to_owned());
    args
}

pub fn spell_auto_on(issue: &str) -> Vec<String> {
    ["--task", issue, "task", "auto", "on", "--json"]
        .map(str::to_owned)
        .to_vec()
}

#[cfg(test)]
mod tests {
    use super::{AutoOff, TaskRun};
    use serde::{de::DeserializeOwned, Serialize};

    /// A response fixture's `result` decodes as `T` and encodes back unchanged.
    fn result<T: DeserializeOwned + Serialize>(raw: &str) -> T {
        let message: serde_json::Value = serde_json::from_str(raw).unwrap();
        let typed: T = serde_json::from_value(message["result"].clone()).unwrap();
        assert_eq!(serde_json::to_value(&typed).unwrap(), message["result"]);
        typed
    }

    #[test]
    fn results_match_the_shared_fixtures_without_defaults() {
        let running: TaskRun = result(include_str!(
            "../../tests/fixtures/dto/server/task_running.json"
        ));
        let text = running.render().unwrap();
        assert!(text.contains("running as Graph run"));
        assert!(!text.contains("may merge the PR"));
        let TaskRun::Running {
            issue,
            run,
            graph_process,
            log,
            ..
        } = running
        else {
            panic!("the fixture is a running Task")
        };
        let merging = TaskRun::Running {
            issue,
            run,
            merges: true,
            graph_process,
            log,
        };
        assert!(merging
            .render()
            .unwrap()
            .contains("this run may merge the PR; `lf task auto off LOO-1` stops it"));
        let off: AutoOff = result(include_str!(
            "../../tests/fixtures/dto/server/task_auto_off_answer.json"
        ));
        assert_eq!(off.render(), "LOO-1: auto off");
        // A missing field is an error, never a default.
        assert!(serde_json::from_str::<TaskRun>(r#"{"state":"running","issue":"LOO-1"}"#).is_err());
        let waiting = TaskRun::Waiting {
            issue: "LOO-1".into(),
            step: "demo".into(),
        };
        assert_eq!(
            serde_json::to_value(&waiting).unwrap(),
            serde_json::json!({"state": "waiting", "issue": "LOO-1", "step": "demo"})
        );
        assert_eq!(waiting.render().unwrap(), "Task LOO-1 waits at demo");
    }
}
