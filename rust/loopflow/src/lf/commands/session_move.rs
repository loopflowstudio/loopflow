//! `lf --machine M resume <session>` moves the conversation before resuming
//! it there; `lf task move X --machine M`, `lf --machine M resume --task X`
//! and resuming a Task's conversation on M move a Task with its conversation.
//! `send` runs where the work lives and `receive` on M, each locally when that
//! side is this Machine, otherwise over one SSH request, so laptop → mini,
//! rental → laptop and rental → mini are the same relay.

use anyhow::{anyhow, Context, Result};

use crate::durable::{Machine, MachineId, RepositoryId, TaskOwnership};
use crate::ops::human_session::moves::{self, Move};
use crate::session::LfSession;
use crate::store::SharedStore;
use crate::work::task::Task;

/// `lf --machine M resume <session>`: move the Session, if this Machine knows
/// it, to `target` unless it is already there. A Task's conversation moves only
/// with its Task, as `lf task move` moves it, and is the conversation the Task
/// carries.
pub fn move_session(id: &str, target: &str, forward_agent: bool) -> Result<()> {
    let task = tokio::runtime::Runtime::new()?.block_on(async {
        let Some(store) = crate::store::open_existing_store().await? else {
            return Ok(None);
        };
        let store: SharedStore = std::sync::Arc::new(store);
        let Some(session) = crate::ops::human_session::session_by_id(&store, id).await? else {
            return Ok(None);
        };
        let target = super::ssh::resolve_target(target, forward_agent).await?;
        if let Some(task) = moves::conversation_task(&store.sqlite, &session.id)? {
            let route = store.task_execution_route(&task).await?;
            return Ok((route.machine_id != target.id).then_some((task, session.id)));
        }
        relay(&store, &session, &target).await.map(|()| None)
    })?;
    if let Some((task, session)) = task {
        move_task(task.as_str(), target, Some(session.as_str()))?;
    }
    Ok(())
}

/// Move a conversation outside any Task to `target` unless it is already there.
async fn relay(store: &SharedStore, session: &LfSession, target: &Machine) -> Result<()> {
    let id = session.id.to_string();
    let moved_to = store.sqlite.session_moved_to(&id)?;
    if moved_to.as_ref() == Some(&target.id) {
        return Ok(());
    }
    ready(store, target, None, &format!("Session {}", session.title)).await?;
    let moved = match moved_to {
        Some(machine) => {
            let source = registered(store, &machine)?;
            let bytes = super::ssh::request(
                &source,
                &[
                    "session".into(),
                    "send".into(),
                    id.clone(),
                    "--to".into(),
                    target.id.to_string(),
                ],
                None,
            )
            .await?;
            serde_json::from_slice(&bytes).context("read the moved Session")?
        }
        None => moves::send(store, &id, &target.id).await?,
    };
    receive(store, &moved, Some(target))
        .await
        .map_err(|error| retry_session(error, &id, target))?;
    eprintln!("Moved {} to {}", session.title, target.name());
    Ok(())
}

/// `lf task move X --machine M`. A Graph the move stopped continues where the
/// Task arrives. `conversation` is the Session it carries; `None` carries the
/// Task's own conversation.
pub fn move_task(issue: &str, target: &str, conversation: Option<&str>) -> Result<TaskOwnership> {
    let (ownership, stopped) = tokio::runtime::Runtime::new()?.block_on(async {
        let store: SharedStore = std::sync::Arc::new(
            crate::store::open_existing_store()
                .await?
                .ok_or_else(|| anyhow!("no Loopflow registry on this machine; run `lf doctor`"))?,
        );
        relay_task(&store, issue, target, conversation).await
    })?;
    if let Some(stopped) = stopped {
        stopped.resume();
    }
    Ok(ownership)
}

/// A Task whose running Graph a move stopped, now owned where it arrived.
struct StoppedRun {
    identifier: String,
    /// The step it was running, which runs again on arrival.
    from: Option<String>,
    task: crate::durable::TaskId,
    /// Where it arrived, as the stage lines name it.
    place: String,
    /// This Machine's checkout of the Task's repository; the run is addressed
    /// from it and starts on the Task's owner.
    repository: Result<std::path::PathBuf, String>,
}

impl StoppedRun {
    /// Start the Task's run on its new owner. The move already happened, so a
    /// run that does not start is reported, never an error.
    fn resume(&self) {
        let name = &self.identifier;
        let started = self.repository.clone().and_then(|repository| {
            crate::engine::task::run(
                &repository,
                self.task.as_str(),
                self.from.as_deref(),
                None,
                None,
                false,
            )
            .map_err(|error| error.message)
        });
        match started {
            Ok(crate::engine::task::TaskRun::Running {
                graph_process,
                ships,
                ..
            }) => {
                eprintln!("Running on {} as Graph process {graph_process}", self.place);
                eprintln!("  follow: lf task status {name}");
                if ships {
                    eprintln!("  ships:  no interactive step is left, so this run may merge the PR; `lf task hold {name}` withdraws");
                }
            }
            Ok(run) => {
                if let Some(text) = run.render() {
                    eprintln!("{text}");
                }
            }
            Err(error) => {
                let from = self
                    .from
                    .as_ref()
                    .map(|step| format!(" --from '{step}'"))
                    .unwrap_or_default();
                eprintln!(
                    "{name} moved, but its Graph did not continue: {error}\n  lf task run {name}{from}"
                )
            }
        }
    }
}

/// Send the Task from the Machine it leaves, receive it on `target`, and learn
/// the move's claim here too so this Machine routes it there. The Machine it
/// leaves records the move as leaving, then sent, until it arrives, so the
/// same command carries it on.
async fn relay_task(
    store: &SharedStore,
    issue: &str,
    target: &str,
    conversation: Option<&str>,
) -> Result<(TaskOwnership, Option<StoppedRun>)> {
    let task = store
        .get_task_by_issue(issue)
        .await?
        .ok_or_else(|| anyhow!("no Task {issue} on this Machine"))?;
    let local = store.sqlite.local_machine()?.id;
    let target = crate::ops::task_ownership::resolve_machine(&store.sqlite, target)?;
    let target = if target == local {
        None
    } else {
        Some(super::ssh::resolve_target(target.as_str(), false).await?)
    };
    let to = target
        .as_ref()
        .map_or(local.clone(), |machine| machine.id.clone());
    let route = store.task_execution_route(&task.id).await?;
    let name = &task.identifier;
    let place = describe(target.as_ref());
    // Already claimed for `to` here: a move that did not arrive. The Machine
    // it leaves carries on leaving, or delivers it again as it was sent.
    let resending = route.machine_id == to;
    let source = if resending {
        let source = sender(&store.sqlite, &task.id, &to)?
            .ok_or_else(|| anyhow!("{name} already runs on {place}"))?;
        eprintln!("Carrying on {name}'s move to {place}");
        source
    } else {
        if let Some(target) = &target {
            ready(
                store,
                target,
                Some((&task, &route.repository_id, &route.machine_id)),
                name,
            )
            .await?;
            eprintln!("Checked {place}: it can take {name}");
        }
        route.machine_id.clone()
    };
    let moved = send_task(
        store,
        &task,
        &route.repository_id,
        &source,
        &to,
        conversation,
    )
    .await
    .map_err(|error| {
        if to != local || source == local {
            return error;
        }
        let from = store
            .sqlite
            .machine_by_id(&source)
            .ok()
            .flatten()
            .map_or_else(|| source.to_string(), |machine| machine.name());
        anyhow!("{error:#}\n{name} is moving here from {from}, which did not send it; the same move run there carries it on")
    })?;
    let moved_task = moved
        .task
        .as_ref()
        .ok_or_else(|| anyhow!("{name} was sent as a Session, not a Task"))?;
    // A stopped Graph continues at the step it was running; one that was
    // resting stays at its step.
    let from = moved_task
        .stopped
        .as_ref()
        .and_then(|stopped| stopped.step.clone());
    let step = from
        .clone()
        .or_else(|| moved_task.graph.as_ref().map(|graph| graph.step.clone()));
    if !resending {
        if moved_task.stopped.is_some() {
            match &from {
                Some(step) => eprintln!("Stopped {name} at `{step}`"),
                None => eprintln!("Stopped {name}'s Graph"),
            }
        }
        if let Some(checkout) = &moved.checkout {
            eprintln!("Committed and pushed {name} on {}", checkout.branch);
        }
    }
    store
        .sqlite
        .ingest_ownership_marker(&task.id, &moved_task.ownership_marker)?;
    receive(store, &moved, target.as_ref())
        .await
        .map_err(|error| match &target {
            Some(target) => retry_task(error, name, target),
            None => error,
        })?;
    // A Task received here recorded its arrival as it was taken in.
    if target.is_some() {
        store
            .sqlite
            .record_move_arrived(&task.id, &moved.from_machine_id, &to)?;
    }
    if moved.from_machine_id != local {
        tell_arrived(&moved.from_machine_id, &route.repository_id, &task, &to).await;
    }
    let at = step
        .map(|step| format!(" (Graph at `{step}`)"))
        .unwrap_or_default();
    let carried = if moved.session.is_some() {
        "; its Session moved with it"
    } else {
        ""
    };
    eprintln!("Arrived on {place}{at}{carried}");
    let stopped = moved_task.stopped.is_some().then(|| StoppedRun {
        identifier: name.clone(),
        from,
        task: task.id.clone(),
        place: place.clone(),
        repository: crate::ops::task::repository_for_task(store, &task)
            .map_err(|error| error.to_string()),
    });
    Ok((store.sqlite.task_ownership(&task.id)?, stopped))
}

/// The Machine a Task moving to `to` comes from: the one `to` checked the
/// move with, when this is `to`, else the writer of the move's claim. `None`
/// once it arrived, or when `to` claimed it itself.
fn sender(
    store: &crate::store::sqlite::SqliteStore,
    task: &crate::durable::TaskId,
    to: &MachineId,
) -> Result<Option<MachineId>> {
    if let Some(from) = store.task_move_checked_from(task)? {
        return Ok(Some(from));
    }
    if store.task_move_arrived_on(task, to)? {
        return Ok(None);
    }
    Ok(store
        .task_ownership(task)?
        .log
        .into_iter()
        .rev()
        .find(|entry| {
            entry.kind == crate::durable::OwnershipKind::Claim
                && entry.machine_id.as_ref() == Some(to)
        })
        .and_then(|entry| entry.writer_machine_id)
        .filter(|writer| writer != to))
}

/// Whether `to` says the Task's last move arrived there; if so, this Machine
/// records the arrival. Best-effort: a Machine out of reach, or one that
/// cannot tell, answers no.
pub(crate) fn arrived_on(task: &crate::durable::TaskId, to: &MachineId) -> Result<bool> {
    tokio::runtime::Runtime::new()?.block_on(async {
        let asked = async {
            let target = super::ssh::resolve_target(to.as_str(), false).await?;
            let args = ["session", "receive", "--arrived", task.as_str()].map(String::from);
            super::ssh::request(&target, &args, None).await
        };
        if let Err(error) = asked.await {
            tracing::info!("{to} did not say Task {task} arrived there: {error:#}");
            return Ok(false);
        }
        let store = crate::store::open_existing_store()
            .await?
            .context("machine commands need an initialized local store")?;
        let here = store.sqlite.local_machine()?.id;
        store.sqlite.record_move_arrived(task, &here, to)?;
        Ok(true)
    })
}

/// The Machine the Task left keeps its move as sent until it hears it
/// arrived. The move already happened, so a Machine out of reach only leaves
/// that record stale, which its next arrival there clears.
async fn tell_arrived(source: &MachineId, repository: &RepositoryId, task: &Task, to: &MachineId) {
    if let Err(error) = send_from(source, repository, task, to, Some("--arrived")).await {
        tracing::warn!(
            "{} arrived, but {source} still records it as sent: {error:#}",
            task.identifier
        );
    }
}

/// Send the Task from `source`: here, or over SSH.
async fn send_task(
    store: &SharedStore,
    task: &Task,
    repository: &RepositoryId,
    source: &MachineId,
    to: &MachineId,
    conversation: Option<&str>,
) -> Result<Move> {
    if *source == store.sqlite.local_machine()?.id {
        return moves::send_task(store, task.id.as_str(), to, conversation).await;
    }
    let bytes = send_from(source, repository, task, to, conversation).await?;
    serde_json::from_slice(&bytes).context("read the moved Task")
}

/// `lf session send --task <task> --to <to> [last]` on `source`, over SSH.
async fn send_from(
    source: &MachineId,
    repository: &RepositoryId,
    task: &Task,
    to: &MachineId,
    last: Option<&str>,
) -> Result<Vec<u8>> {
    let source = super::ssh::resolve_target(source.as_str(), false).await?;
    let mut args = vec![
        "--repo-id".to_string(),
        repository.to_string(),
        "session".into(),
        "send".into(),
        "--task".into(),
        task.id.to_string(),
        "--to".into(),
        to.to_string(),
    ];
    args.extend(last.map(str::to_string));
    super::ssh::request(&source, &args, None).await
}

/// Before anything is committed, pushed, stopped or handed off, `target`
/// answers that it can receive the move and, for a Task, takes it in from
/// planning, cloning the repository first when it has none.
async fn ready(
    store: &SharedStore,
    target: &Machine,
    task: Option<(&Task, &RepositoryId, &MachineId)>,
    what: &str,
) -> Result<()> {
    if target.id == store.sqlite.local_machine()?.id {
        return Ok(());
    }
    let mut args = vec!["session".into(), "receive".into(), "--check".into()];
    if let Some((task, repository, from)) = task {
        args.extend([
            "--task".into(),
            task.id.to_string(),
            "--issue".into(),
            task.identifier.clone(),
            "--repository".into(),
            repository.to_string(),
            "--from".into(),
            from.to_string(),
        ]);
        if let Some(path) = store.sqlite.repository_path(repository)? {
            args.extend([
                "--origin".into(),
                crate::git::git_stdout(
                    std::path::Path::new(&path),
                    &["remote", "get-url", "origin"],
                )?
                .trim()
                .to_string(),
            ]);
        }
        if let Some(key) = store.sqlite.planning_user_key()? {
            args.extend(["--planning-key".into(), key]);
        }
    }
    super::ssh::request(target, &args, None)
        .await
        .map(drop)
        .map_err(|error| {
            anyhow!(
                "{what} did not move: {} cannot receive it; nothing was sent\n{error:#}",
                target.name()
            )
        })
}

/// Receive on `target`, over SSH unless it is this Machine (`None`). Either
/// way `lf session receive` is its own Process: a Graph it carries in arrives
/// stopped, so this move can continue it.
async fn receive(store: &SharedStore, moved: &Move, target: Option<&Machine>) -> Result<()> {
    let args = ["session".to_string(), "receive".to_string()];
    let input = serde_json::to_vec(moved)?;
    match target {
        Some(target) if target.id != store.sqlite.local_machine()?.id => {
            super::ssh::request(target, &args, Some(&input)).await?;
        }
        _ => receive_here(&args, &input).await?,
    }
    Ok(())
}

async fn receive_here(args: &[String], input: &[u8]) -> Result<()> {
    use tokio::io::AsyncWriteExt;
    let mut child = tokio::process::Command::new(crate::os_process::resolve_lf_binary()?)
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .context("start `lf session receive`")?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input).await?;
    }
    let output = child.wait_with_output().await?;
    anyhow::ensure!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(())
}

/// A Session sent by hand is received by hand.
fn retry_session(error: anyhow::Error, id: &str, target: &Machine) -> anyhow::Error {
    anyhow!(
        "{error:#}\nSession {id} was sent but did not arrive on {}; where it lived, retry with\n  lf session send {id} --to {} | lf --machine {} session receive",
        target.name(),
        target.id,
        target.name(),
    )
}

/// A Task's move is recorded as sent, so the same move delivers it again.
fn retry_task(error: anyhow::Error, name: &str, target: &Machine) -> anyhow::Error {
    anyhow!(
        "{error:#}\n{name} was sent but did not arrive on {}; retry with\n  lf task move {name} --machine {}\n  {}",
        target.name(),
        target.name(),
        crate::ops::task_ownership::take_back(name),
    )
}

fn registered(store: &SharedStore, machine: &MachineId) -> Result<Machine> {
    store
        .sqlite
        .machine_by_id(machine)?
        .ok_or_else(|| anyhow!("Machine {machine} is not registered here"))
}

fn describe(machine: Option<&Machine>) -> String {
    match machine {
        Some(machine) => machine.name(),
        None => format!(
            "{} (this Machine)",
            crate::store::sqlite::task_ownership::local_label()
        ),
    }
}
