//! The `task` namespace's core loop: run, ship, hold, steer, report, wait and
//! interrupt a Task, and claim or unassign it.
//!
//! A run and a ship outlive the request that starts them. From a terminal the
//! `lf` process is their carrier; a served request has none, so [`run`] and
//! [`ship`] start the same command detached ([`super::launch`]) and return the
//! result it reports, the value its CLI spelling prints with `--json`. A Task
//! whose Work lives on another Machine runs the operation's `lf … --json`
//! spelling there over SSH; nothing falls back to local execution.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use super::{launch, Error, ErrorCode, Result};
use crate::durable::TaskOwnership;
use crate::id::LfProcessId;
use crate::lf::commands::subscribe::{Subscription, Transport, Wake, ACTIVITY_CLOCK};
use crate::ops::task::{TaskControlResult, TaskSnapshot, TaskWaitUntil};
use crate::ops::task_ownership::OwnershipChange;
use crate::work::task::comment::TaskComments;

/// What `task.run` started.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum TaskRun {
    /// The run's Graph process registered; it goes on without its launcher.
    Running {
        issue: String,
        /// The graph and where the run set out: `code from implement`.
        run: String,
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
                graph_process,
                log: Some(log),
            } => Some(format!(
                "{issue} · {run} running as Graph process {graph_process}\n  follow: lf task status {issue}\n  log:    {}",
                log.display()
            )),
            Self::Running { log: None, .. } => None,
            Self::Waiting { issue, step } => Some(format!("Task {issue} waits at {step}")),
            Self::Ended { issue } => Some(format!("Task {issue} reached the end of its graph")),
        }
    }
}

/// What `task.ship` set going.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum TaskShip {
    /// Auto-Ship is approved and `process` ships the Task to `end`.
    Shipping {
        issue: String,
        #[serde(rename = "lf_process_id")]
        process: LfProcessId,
        /// The ship's output once its launcher lets go; null under `-b`.
        log: Option<PathBuf>,
    },
    /// Another live process already ships it.
    AlreadyShipping {
        issue: String,
        #[serde(rename = "lf_process_id")]
        process: LfProcessId,
    },
    /// The Task already stands at `end`.
    AtEnd { issue: String },
}

impl TaskShip {
    /// What `lf ship` prints; a ship its terminal keeps prints nothing here.
    pub fn render(&self) -> Option<String> {
        match self {
            Self::Shipping {
                issue,
                log: Some(log),
                ..
            } => Some(format!(
                "{issue} shipping\n  follow: lf task status {issue}\n  log:    {}",
                log.display()
            )),
            Self::Shipping { log: None, .. } => None,
            Self::AlreadyShipping { issue, process } => {
                Some(format!("{issue} is already shipping (process {process})"))
            }
            Self::AtEnd { issue } => Some(format!("{issue} is already at its end")),
        }
    }
}

/// What `task.hold` withdrew.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Hold {
    pub issue: String,
    /// A run was shipping the Task; otherwise only the stop was saved.
    pub stopped: bool,
}

impl Hold {
    pub fn render(&self) -> String {
        let issue = &self.issue;
        if self.stopped {
            format!("{issue} stopped shipping")
        } else {
            format!("{issue} is not shipping; no run ships it until `lf ship {issue}`")
        }
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
/// runs every step headless to `end` and approves Auto-Ship, as `-b` does;
/// without it the run stops before the next interactive step. With no
/// `agent`, each step's harness is the repository's configured default.
pub fn run(
    directory: &Path,
    issue: &str,
    from: Option<&str>,
    agent: Option<&str>,
    approve: bool,
) -> Result<TaskRun> {
    let spell = |task: &str| spell_run(task, from, agent, approve);
    if let Some(run) = remote(directory, issue, spell)? {
        return Ok(run);
    }
    launch::start(directory, &spell(issue))
}

/// `task.ship`: approve Auto-Ship and start shipping detached, as `lf --task
/// <issue> ship` does: every remaining step headless, an interactive step
/// standing in for the person, until the PR lands.
pub fn ship(directory: &Path, issue: &str) -> Result<TaskShip> {
    let spell = |task: &str| spell_task(task, "ship");
    if let Some(shipping) = remote(directory, issue, spell)? {
        return Ok(shipping);
    }
    launch::start(directory, &spell(issue))
}

/// `task.hold`: withdraw the Task's Auto-Ship approval, as `lf --task <issue>
/// ship --stop` does, until the next `task.ship`.
pub fn hold(directory: &Path, issue: &str, reason: Option<&str>) -> Result<Hold> {
    let spell = |task: &str| spell_task(task, "hold");
    if let Some(held) = remote(directory, issue, spell)? {
        return Ok(held);
    }
    let stopped = crate::ops::task::ship::ship_stop(issue, reason).map_err(written)?;
    Ok(Hold {
        issue: issue.to_owned(),
        stopped,
    })
}

/// `task.steer`: direction for the Task's runs, as `lf task steer <issue>
/// <message>` does. The delivery is [`super::session::steer`]'s Task scope:
/// the entry joins the Task's thread and the store saves it as the Task's
/// steer, which every Session on the Task, now or later, reads at its next
/// boundary. Returns the thread with the entry.
pub fn steer(
    directory: &Path,
    issue: &str,
    wave: Option<&str>,
    message: &str,
) -> Result<TaskComments> {
    let repo = crate::ops::task::task_repository(directory, Some(issue)).map_err(written)?;
    tokio::runtime::Runtime::new()
        .map_err(written)?
        .block_on(async {
            let (store, task) = crate::ops::task::resolve_saved_task(&repo, wave, issue)
                .await
                .map_err(written)?;
            super::session::steer(
                &store,
                &repo,
                super::session::SteerScope::Task { issue, wave },
                message,
            )
            .await?;
            store.sqlite.task_comments(&task.id).map_err(written)
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

/// `task.claim` / `task.unassign`, on the Machine that writes the Task's ownership.
pub fn ownership(directory: &Path, issue: &str, change: OwnershipChange) -> Result<TaskOwnership> {
    let verb = match change {
        OwnershipChange::Claim => "claim",
        _ => "unassign",
    };
    let spell = |task: &str| spell_task(task, verb);
    if let Some(ownership) = remote(directory, issue, spell)? {
        return Ok(ownership);
    }
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
    let started = Instant::now();
    loop {
        let reading = waited(issue, until)?;
        if reading.reached || timeout.is_some_and(|limit| started.elapsed() >= limit) {
            return Ok(reading);
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

/// `task.wait` as a subscription: a reading whenever the Task changes, the
/// last one `reached`.
pub fn subscribe_wait(
    issue: String,
    until: TaskWaitUntil,
    transport: Transport,
) -> anyhow::Result<Option<Subscription>> {
    crate::lf::commands::subscribe::start(
        (issue, until),
        Wake::every(ACTIVITY_CLOCK),
        |store, (issue, until): (String, TaskWaitUntil)| async move {
            let store = crate::lf::commands::subscribe::require_store(store)?;
            match crate::ops::task::task_wait_reading(&store, &issue, until).await? {
                Some((task, reached)) => Ok(Waited { reached, task }),
                None => Err(missing(&issue).into()),
            }
        },
        transport,
    )
}

/// The value `lf <spell(task)>` prints on the Machine that owns the Task
/// `issue` names, when that is not this one.
fn remote<T: DeserializeOwned>(
    directory: &Path,
    issue: &str,
    spell: impl FnOnce(&str) -> Vec<String>,
) -> Result<Option<T>> {
    let Some(route) = crate::lf::commands::work_route::remote_task(issue)
        .map_err(|error| Error::refused(&error, ErrorCode::Unavailable))?
    else {
        return Ok(None);
    };
    let args = spell(route.task_id.as_str());
    let printed = crate::lf::commands::work_route::run_remote(route, directory, &args)
        .map_err(|error| Error::outcome_unknown(&error))?;
    serde_json::from_value(printed)
        .map(Some)
        .map_err(|error| Error::outcome_unknown(&error.into()))
}

fn written(error: impl Into<anyhow::Error>) -> Error {
    Error::written(&error.into())
}

fn missing(issue: &str) -> Error {
    Error::new(ErrorCode::NotFound, format!("no Task exists for {issue:?}"))
        .with(serde_json::json!({ "task": issue }))
}

pub(super) fn spell_run(
    issue: &str,
    from: Option<&str>,
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
    if let Some(from) = from {
        args.extend(["--from".to_owned(), from.to_owned()]);
    }
    args.push("--json".to_owned());
    args
}

pub(super) fn spell_task(issue: &str, verb: &str) -> Vec<String> {
    if verb == "ship" {
        ["--task", issue, "ship", "--json"]
    } else {
        ["task", verb, issue, "--json"]
    }
    .map(str::to_owned)
    .to_vec()
}

#[cfg(test)]
mod tests {
    use super::{Hold, TaskRun, TaskShip};
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
            "../../../../tests/fixtures/dto/server/task_running.json"
        ));
        assert!(running
            .render()
            .unwrap()
            .contains("running as Graph process"));
        let shipping: TaskShip = result(include_str!(
            "../../../../tests/fixtures/dto/server/task_shipping.json"
        ));
        assert!(shipping.render().unwrap().starts_with("LOO-1 shipping"));
        let held: Hold = result(include_str!(
            "../../../../tests/fixtures/dto/server/task_held.json"
        ));
        assert_eq!(held.render(), "LOO-1 stopped shipping");
        // A missing field is an error, never a default.
        assert!(serde_json::from_str::<TaskRun>(r#"{"state":"running","issue":"LOO-1"}"#).is_err());
        assert!(serde_json::from_str::<TaskShip>(r#"{"state":"at_end"}"#).is_err());
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
