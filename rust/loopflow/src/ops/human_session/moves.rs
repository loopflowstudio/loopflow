//! A Session moves to another Machine: its branch through the remote, its
//! provider transcript inside the move, its identity unchanged. `send` runs
//! where it lives and completes it there, naming where it went; `receive`
//! checks out the branch where it arrives and admits the same Session, so the
//! same provider conversation resumes there.
//!
//! A Task moves the same way with its ownership and Graph: `send_task`
//! claims it for the target, stops its Graph, checkpoints its worktree and
//! takes its conversation along; `receive` takes in that claim before
//! checking it out.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use base64::Engine as _;
use serde::{Deserialize, Serialize};

use super::{provider_conversation, session_by_id, session_not_found};
use crate::durable::{MachineId, RepositoryId, TaskId, TaskOwnershipEntry, WorkRef};
use crate::harness::HarnessKind;
use crate::id::{AgentSessionId, LfSessionId, WaveId};
use crate::journal::ProcessIdentityEvidence;
use crate::process::Departure;
use crate::provider_auth::Provider;
use crate::repository::CanonicalRepo;
use crate::session::{LfSession, PrimaryScope, TitleSource, WorkSource};
use crate::store::sqlite::task_moves::{MoveInFlight, MoveStage};
use crate::store::sqlite::task_work::{ShipApproval, TaskGraphRecord};
use crate::store::sqlite::SqliteStore;
use crate::store::SharedStore;
use crate::transcript::{Located, Source};
use crate::work::task::Task;

/// One Session, or one Task with its Session, on its way from one Machine to another.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Move {
    pub from_machine_id: MachineId,
    /// Absent for a Task with no conversation.
    pub session: Option<MovedSession>,
    /// Absent for a Task not checked out where it lived.
    pub checkout: Option<MovedCheckout>,
    /// Absent before the conversation's first turn: nothing to resume yet.
    pub history: Option<MovedHistory>,
    /// Present when a Task moves, not only a conversation.
    pub task: Option<MovedTask>,
}

/// What a Task carries beyond its conversation and branch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct MovedTask {
    pub task_id: TaskId,
    pub identifier: String,
    /// The move's claim, as the marked comment planning would deliver later.
    pub ownership_marker: String,
    /// Absent before the Task takes up a Graph.
    pub graph: Option<TaskGraphRecord>,
    /// Absent unless the move stopped a running Graph, which continues where
    /// the Task arrives.
    pub stopped: Option<StoppedGraph>,
    /// The standing Auto-Ship approval; absent when the Task was not shipping.
    pub approval: Option<ShipApproval>,
}

/// A Graph the move stopped.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct StoppedGraph {
    /// The step it was running, which runs again from the checkpoint; absent
    /// when no step was running, so the run starts again where it began.
    pub step: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct MovedSession {
    pub id: LfSessionId,
    pub title: String,
    pub title_source: TitleSource,
    pub provider: Option<String>,
    pub model: Option<String>,
    /// The Wave or repository whose ongoing conversation this is.
    pub primary: Option<MovedPrimary>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum MovedPrimary {
    Repository,
    Wave { wave_id: WaveId },
}

/// Where the conversation's code is: a pushed commit on a branch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct MovedCheckout {
    pub repository_id: RepositoryId,
    /// Where a Machine without a checkout of the repository clones it from.
    pub origin: String,
    pub branch: String,
    pub commit: String,
}

/// The provider's own transcript, file by file. Claude's paths are relative to
/// its project directory, which is named for the checkout; Codex's to its home.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct MovedHistory {
    pub agent_session_id: AgentSessionId,
    pub files: Vec<MovedFile>,
    /// The conversation as plain text, for a Machine whose provider cannot
    /// read these files.
    pub log: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct MovedFile {
    pub path: String,
    /// Base64 of the file's bytes.
    pub content: String,
}

/// Commit and push the Session's branch, stop its agent, read its transcript
/// and complete it here as moved to `to`. Repeating it after a failed receive
/// reads the same Session again. A Task's conversation moves only with its
/// Task, so it sends the Task.
pub(crate) async fn send(store: &SharedStore, id: &str, to: &MachineId) -> Result<Move> {
    let session = session_by_id(store, id)
        .await?
        .ok_or_else(|| session_not_found(id))?;
    if let Some(task) = conversation_task(&store.sqlite, &session.id)? {
        return send_task(store, task.as_str(), to, Some(session.id.as_str())).await;
    }
    let name = &session.title;
    let root = crate::git::worktree_root(&session.cwd).map_err(|_| {
        anyhow!(
            "Session {name} runs outside a repository ({}); only a conversation in a Git checkout moves",
            session.cwd.display()
        )
    })?;
    refuse_mid_operation(&root, name)?;
    let checkout = checkpoint(&store.sqlite, &root, name, to)?;
    let (moved, history) = take(
        store,
        &session,
        Departure::Machine {
            machine: to.clone(),
            task: None,
        },
    )?;
    store.sqlite.complete_moved_session(&session.id, to)?;
    Ok(Move {
        from_machine_id: store.sqlite.local_machine()?.id,
        session: Some(moved),
        checkout: Some(checkout),
        history,
        task: None,
    })
}

/// The Task whose conversation `session` is: the Task it is the primary
/// Session of, else the one Task it belongs to.
pub(crate) fn conversation_task(store: &SqliteStore, session: &str) -> Result<Option<TaskId>> {
    if let Some(PrimaryScope::Task(task)) = store.primary_scope(session)? {
        return Ok(Some(task));
    }
    Ok(match store.session_task_ids(session)?.as_slice() {
        [task] => Some(task.clone()),
        _ => None,
    })
}

/// Claim the Task for `to`, then stop its Graph, commit and push its
/// worktree and take `conversation` (the Task's own conversation when `None`).
/// From its claim until it is sent the move is leaving: a repeat naming the
/// same Machine carries on from there, with the Graph it stopped. A failure
/// in those steps hands it back here. The move is recorded as sent before it
/// returns, so a repeat returns what was sent, even when the first reply never
/// reached the caller.
pub(crate) async fn send_task(
    store: &SharedStore,
    issue: &str,
    to: &MachineId,
    conversation: Option<&str>,
) -> Result<Move> {
    let task = store
        .get_task_by_issue(issue)
        .await?
        .ok_or_else(|| anyhow!("no Task {issue} on this Machine"))?;
    let name = &task.identifier;
    let moving = store.sqlite.task_move_in_flight(&task.id)?;
    if let Some(MoveInFlight {
        to_machine_id,
        stage: MoveStage::Sent { payload },
        ..
    }) = &moving
    {
        if to_machine_id == to {
            let moved: Move = serde_json::from_str(payload).context("read the sent move")?;
            if let Some(session) = &moved.session {
                store
                    .sqlite
                    .complete_task_conversation(&session.id, &task.id)?;
            }
            return Ok(moved);
        }
    }
    let worktree = task.worktree.clone().filter(|path| path.exists());
    if let Some(root) = &worktree {
        refuse_mid_operation(root, name)?;
    }
    // Owned by `to` from here, so nothing on this Machine (recovery restarting
    // a ship process, a run waiting out a limit) starts the Task again while
    // the move stops it. If it cannot leave, it is handed back.
    let (claim, stopped) = match moving {
        Some(MoveInFlight {
            to_machine_id,
            stage: MoveStage::Leaving { claim_id, stopped },
            ..
        }) if &to_machine_id == to => {
            let claim = store
                .sqlite
                .task_ownership(&task.id)?
                .log
                .into_iter()
                .find(|entry| entry.id == claim_id)
                .ok_or_else(|| anyhow!("{name}'s move claim {claim_id} is gone"))?;
            let stopped = stopped
                .map(|stopped| serde_json::from_str(&stopped))
                .transpose()
                .context("read the stopped Graph")?;
            (claim, stopped)
        }
        Some(
            leaving @ MoveInFlight {
                stage: MoveStage::Leaving { .. },
                ..
            },
        ) => bail!(
            "{}",
            crate::ops::task_ownership::describe_move(&store.sqlite, name, &leaving)?
        ),
        _ => (claim_for(&store.sqlite, &task, to)?, None),
    };
    let (stopped, checkout, conversation, taken) =
        match leave(store, &task, worktree.as_deref(), to, conversation, stopped).await {
            Ok(left) => left,
            Err(error) => {
                store.sqlite.hand_back(&task.id, &claim)?;
                return Err(error);
            }
        };
    let (session, history) =
        taken.map_or((None, None), |(session, history)| (Some(session), history));
    let moved = Move {
        from_machine_id: store.sqlite.local_machine()?.id,
        session,
        checkout,
        history,
        task: Some(MovedTask {
            task_id: task.id.clone(),
            identifier: task.identifier.clone(),
            ownership_marker: crate::store::sqlite::task_ownership::marker(&claim)?,
            graph: store.sqlite.task_graph_record(&task.id)?,
            stopped,
            approval: store.sqlite.task_ship_approval(&task.id)?,
        }),
    };
    store
        .sqlite
        .record_move_sent(&task.id, &serde_json::to_string(&moved)?)?;
    if let Some(session) = &conversation {
        store
            .sqlite
            .complete_task_conversation(&session.id, &task.id)?;
    }
    Ok(moved)
}

/// The Task this Machine sent to `to` arrived there: nothing is in flight.
pub(crate) async fn sent_task_arrived(
    store: &SharedStore,
    issue: &str,
    to: &MachineId,
) -> Result<()> {
    let task = store
        .get_task_by_issue(issue)
        .await?
        .ok_or_else(|| anyhow!("no Task {issue} on this Machine"))?;
    let here = store.sqlite.local_machine()?.id;
    Ok(store.sqlite.record_move_arrived(&task.id, &here, to)?)
}

type Left = (
    Option<StoppedGraph>,
    Option<MovedCheckout>,
    Option<LfSession>,
    Option<(MovedSession, Option<MovedHistory>)>,
);

/// What leaves with the Task: its stopped Graph, its pushed checkpoint and
/// its stopped conversation, `conversation` or else the Task's own. A Graph
/// stopped by an earlier attempt of this move is `stopped`: nothing of it is
/// left running to stop again.
async fn leave(
    store: &SharedStore,
    task: &Task,
    worktree: Option<&Path>,
    to: &MachineId,
    conversation: Option<&str>,
    stopped: Option<StoppedGraph>,
) -> Result<Left> {
    let name = &task.identifier;
    let stopped = match stop_graphs(store, task).await? {
        Some(now) => {
            store
                .sqlite
                .record_move_stopped(&task.id, &serde_json::to_string(&now)?)?;
            Some(now)
        }
        None => stopped,
    };
    let checkout = worktree
        .map(|root| checkpoint(&store.sqlite, root, name, to))
        .transpose()?;
    let conversation = match conversation {
        Some(id) => Some(
            session_by_id(store, id)
                .await?
                .ok_or_else(|| session_not_found(id))?,
        ),
        None => super::primary::task_conversation(store, &task.id).await?,
    };
    let taken = match conversation.as_ref() {
        Some(session) => {
            await_provider_identity(&store.sqlite, &session.id).await?;
            Some(take(
                store,
                session,
                Departure::Machine {
                    machine: to.clone(),
                    task: Some(task.id.clone()),
                },
            )?)
        }
        None => None,
    };
    Ok((stopped, checkout, conversation, taken))
}

/// A conversation checked out moments ago may still be launching: its
/// attachment exists, but its provider has not recorded a PID. Give it the
/// owner's bind time so the stop meets a provider, or an attempt that ended.
async fn await_provider_identity(store: &SqliteStore, session: &str) -> Result<()> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
        let starting = store
            .session_attachment(session)?
            .is_some_and(|attachment| {
                crate::journal::agent_evidence(store, &attachment.agent_process_id)
                    == ProcessIdentityEvidence::Unknown
            });
        if !starting || std::time::Instant::now() >= deadline {
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

/// The Session as it leaves: its agent stopped, its transcript read.
fn take(
    store: &SharedStore,
    session: &LfSession,
    departure: Departure,
) -> Result<(MovedSession, Option<MovedHistory>)> {
    crate::harness::agent_process::stop_session_agent(&store.sqlite, &session.id, departure)?;
    let history = history(&store.sqlite, session)?;
    let primary = match store.sqlite.session_primary_scope(&session.id)?.as_deref() {
        Some("repository") => Some(MovedPrimary::Repository),
        Some("wave") => Some(MovedPrimary::Wave {
            wave_id: session
                .wave_id
                .clone()
                .ok_or_else(|| anyhow!("Wave Session {} names no Wave", session.id))?,
        }),
        _ => None,
    };
    Ok((
        MovedSession {
            id: session.id.clone(),
            title: session.title.clone(),
            title_source: session.title_source,
            provider: session.provider.clone(),
            model: session.model.clone(),
            primary,
        },
        history,
    ))
}

/// The claim naming `to`, written here with the move leaving: this Machine's
/// claim moves there. An unowned Task is claimed first; one `to` already owns
/// has nothing to send.
fn claim_for(store: &SqliteStore, task: &Task, to: &MachineId) -> Result<TaskOwnershipEntry> {
    let owner = store
        .task_ownership(&task.id)?
        .owner
        .and_then(|owner| owner.machine.id);
    if owner.as_ref() == Some(to) {
        bail!("{} already runs on {to}", task.identifier);
    }
    Ok(store.begin_task_move(&task.id, to)?)
}

/// Interrupt the Task so nothing starts again, then end each Graph still
/// running and the recorded step Processes it started. Each is signalled
/// only once its pid is proven to be that recorded Process. Returns the
/// stopped Graph, if one was running.
async fn stop_graphs(store: &SharedStore, task: &Task) -> Result<Option<StoppedGraph>> {
    let live = |process: &crate::process::LfProcess| {
        process.completed_at.is_none()
            && crate::journal::lf_evidence(&store.sqlite, &process.id)
                == ProcessIdentityEvidence::Live
    };
    let graphs: Vec<_> = store
        .sqlite
        .task_graphs(&task.id)?
        .into_iter()
        .filter(|graph| live(&graph.process))
        .collect();
    let step = graphs.iter().find_map(|graph| {
        graph
            .latest()
            .filter(|step| live(&step.process))
            .map(|step| graph.label(step))
    });
    let processes: Vec<crate::process::LfProcess> = graphs
        .into_iter()
        .flat_map(|graph| {
            std::iter::once(graph.process).chain(graph.steps.into_iter().map(|step| step.process))
        })
        .filter(|process| live(process))
        .collect();
    if processes.is_empty() {
        return Ok(None);
    }
    store
        .append_interrupt(&WorkRef::Task(task.id.clone()))
        .await?;
    for process in &processes {
        if let Some((pid, birth)) = process.pid.zip(process.os_started_at) {
            if crate::journal::process_identity_evidence(pid, birth)
                == ProcessIdentityEvidence::Live
            {
                // SAFETY: the pid was just proven to be the recorded Process.
                unsafe {
                    libc::kill(pid as i32, libc::SIGTERM);
                }
            }
        }
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
        let running: Vec<String> = processes
            .iter()
            .filter(|process| {
                crate::journal::lf_evidence(&store.sqlite, &process.id)
                    == ProcessIdentityEvidence::Live
            })
            .map(|process| process.id.to_string())
            .collect();
        if running.is_empty() {
            return Ok(Some(StoppedGraph { step }));
        }
        if std::time::Instant::now() >= deadline {
            bail!(
                "{}'s Graph did not stop ({}); nothing moved",
                task.identifier,
                running.join(", ")
            );
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

fn refuse_mid_operation(root: &Path, name: &str) -> Result<()> {
    if let Some(state) = crate::git::intervention_state(root)? {
        bail!("{name}'s worktree is mid-{state}; finish or abort the {state}, then move it");
    }
    Ok(())
}

/// Bring the checkout's code to the remote: commit what is uncommitted and push
/// its branch. The default branch is never committed to or pushed; it moves
/// only when clean.
fn checkpoint(
    store: &SqliteStore,
    root: &Path,
    name: &str,
    to: &MachineId,
) -> Result<MovedCheckout> {
    let branch = crate::git::current_branch(root)?.ok_or_else(|| {
        anyhow!("{name}'s worktree has no branch checked out; check one out, then move it")
    })?;
    let clean = crate::git::is_clean(root)?;
    if branch == crate::git::get_default_branch(root)? {
        anyhow::ensure!(
            clean,
            "{name}'s checkout has uncommitted work on {branch}; move it to a branch first"
        );
    } else {
        if !clean {
            crate::git::stage_all(root, &|_| {})?;
            crate::git::commit(root, &format!("Move {name} to {to}"), &|_| {})?;
        }
        crate::git::push_with_upstream(root, "origin", &branch, &|_| {})
            .with_context(|| format!("push {branch} before moving {name}"))?;
    }
    let canonical = CanonicalRepo::discover(root)?;
    Ok(MovedCheckout {
        repository_id: store
            .repository_id(&canonical.to_string())?
            .ok_or_else(|| {
                anyhow!("repository identity unavailable; run `lf repo identity` in {canonical}")
            })?,
        origin: crate::git::git_stdout(root, &["remote", "get-url", "origin"])?
            .trim()
            .to_string(),
        branch,
        commit: crate::git::rev_parse(root, "HEAD")?,
    })
}

fn history(store: &SqliteStore, session: &LfSession) -> Result<Option<MovedHistory>> {
    let Some(key) = session.artifact_key.as_deref() else {
        return Ok(None);
    };
    let Some(reference) = store.input_provider_session(key)? else {
        return Ok(None);
    };
    let named = session.provider.as_deref().unwrap_or_default();
    let (Ok(provider), Some(harness)) = (named.parse::<Provider>(), HarnessKind::parse(named))
    else {
        bail!(
            "Session {} runs on {}, whose conversations do not move between Machines",
            session.id,
            session.provider.as_deref().unwrap_or("an unknown provider")
        );
    };
    let thread = reference.agent_session;
    let located = crate::transcript::locate(store, harness, &thread, &session.cwd)?;
    let (path, home) = match located {
        Located::Found {
            source: Source::File(path),
            home: Some(home),
        } => (path, home),
        Located::Found { .. } => bail!("{provider} conversation {thread} has no transcript file"),
        Located::Gone(gap) => bail!(
            "the {provider} transcript of Session {} ({thread}) is unavailable ({gap:?}); nothing moved",
            session.id
        ),
    };
    let base = match provider {
        Provider::Claude => path
            .parent()
            .expect("a Claude transcript is in its project directory")
            .to_path_buf(),
        _ => home,
    };
    let mut files = vec![read_file(&base, &path)?];
    let sibling = path.with_extension("");
    if provider == Provider::Claude && sibling.is_dir() {
        read_tree(&base, &sibling, &mut files)?;
    }
    Ok(Some(MovedHistory {
        agent_session_id: thread,
        files,
        log: super::conversation_log::read(store, session)?,
    }))
}

fn read_tree(base: &Path, dir: &Path, files: &mut Vec<MovedFile>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            read_tree(base, &path, files)?;
        } else {
            files.push(read_file(base, &path)?);
        }
    }
    Ok(())
}

fn read_file(base: &Path, path: &Path) -> Result<MovedFile> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(MovedFile {
        path: path
            .strip_prefix(base)
            .expect("a transcript file is under its base")
            .to_string_lossy()
            .into_owned(),
        content: base64::engine::general_purpose::STANDARD.encode(bytes),
    })
}

/// The Task a move carries: saved here, else acquired from the planning of
/// `main`, the repository it arrives in.
pub(crate) async fn arriving_task(
    store: &SharedStore,
    main: &Path,
    task: &TaskId,
    identifier: &str,
) -> Result<Task> {
    if let Some(saved) = store.get_task(task).await? {
        return Ok(saved);
    }
    let found =
        crate::ops::planning_sync::find_task(store, &main.to_string_lossy(), identifier).await?;
    let acquired = match found {
        Some(task) => task,
        None => crate::ops::task::acquire_from_planning(main, identifier).await?,
    };
    anyhow::ensure!(
        acquired.id == *task,
        "{identifier} in {} is Task {}, not the moving Task {task}",
        main.display(),
        acquired.id
    );
    Ok(acquired)
}

/// Check out the moved branch in a worktree of this repository, admit the
/// same Session there and place its transcript where its provider resumes it.
/// A moved Task is taken in first: its claim, its checkout on the carried
/// branch and its Graph. Returns the Session, when one moved.
pub(crate) async fn receive(store: &SharedStore, moved: Move) -> Result<Option<LfSession>> {
    let repo = repository(store, &moved).await?;
    let main = crate::git::worktrees::main_repo_root(&repo)?;
    let task = match &moved.task {
        Some(task) => {
            let taken = take_in_task(store, &main, task, moved.checkout.as_ref()).await?;
            store.sqlite.record_move_arrived(
                &task.task_id,
                &moved.from_machine_id,
                &store.sqlite.local_machine()?.id,
            )?;
            Some(taken)
        }
        None => None,
    };
    let Some(moved_session) = &moved.session else {
        return Ok(None);
    };
    let cwd = match (&task, &moved.checkout) {
        (Some(task), _) => task
            .worktree
            .clone()
            .ok_or_else(|| anyhow!("Task {} arrived without a checkout", task.identifier))?,
        (None, Some(checkout)) => check_out(&main, checkout)?,
        (None, None) => bail!("Session {} moved without its checkout", moved_session.id),
    };
    let canonical = CanonicalRepo::discover(&cwd)?;
    let session = admit(store, moved_session, task.as_ref(), &cwd, &canonical).await?;
    let (Some(history), Some(provider)) = (&moved.history, moved_session.provider.as_deref())
    else {
        return Ok(Some(session));
    };
    let (Ok(provider), Some(harness)) =
        (provider.parse::<Provider>(), HarnessKind::parse(provider))
    else {
        bail!("unknown provider {provider}");
    };
    let home =
        crate::transcript::conversation_home(&store.sqlite, provider, &history.agent_session_id)?
            .unwrap_or_else(|| crate::provider_account::activation::native_home(provider, None));
    let base = match provider {
        Provider::Claude => crate::transcript::live::claude_project(&home, &cwd),
        _ => home,
    };
    for file in &history.files {
        write_file(&base, file)?;
    }
    if !readable(harness, &base, &history.files) {
        // This Machine's provider cannot read the transcript: a fresh
        // conversation starts from its log instead.
        let request =
            super::conversation_log::continue_from(&session, provider.as_str(), &history.log)?;
        let mut next = session;
        next.skill = None;
        next.request = Some(request);
        return super::append_input(store, next).await.map(Some);
    }
    provider_conversation::select(
        store,
        &session,
        provider,
        history.agent_session_id.as_str(),
        None,
    )
    .await
    .map(Some)
}

/// This Machine's checkout of the moved repository.
async fn repository(store: &SharedStore, moved: &Move) -> Result<PathBuf> {
    let repository = match (&moved.checkout, &moved.task) {
        (Some(checkout), _) => checkout.repository_id.clone(),
        (None, Some(task)) => {
            store
                .get_task(&task.task_id)
                .await?
                .ok_or_else(|| {
                    anyhow!(
                        "Task {} moved without a checkout and is unknown here",
                        task.identifier
                    )
                })?
                .repository_id
        }
        (None, None) => bail!("the move names neither a checkout nor a Task"),
    };
    let origin = moved
        .checkout
        .as_ref()
        .map(|checkout| checkout.origin.as_str());
    checkout_of(store, &repository, origin)
}

/// This Machine's checkout of `repository`, cloned from `origin` under
/// `repo_root` when there is none. The pre-move check clones too, so a Machine
/// that has never seen the repository can take its Task in before anything stops.
pub(crate) fn checkout_of(
    store: &SharedStore,
    repository: &RepositoryId,
    origin: Option<&str>,
) -> Result<PathBuf> {
    if let Some(path) = store.sqlite.repository_path(repository)? {
        return Ok(PathBuf::from(path));
    }
    let origin =
        origin.ok_or_else(|| anyhow!("repository {repository} has no checkout on this Machine"))?;
    let name = origin
        .trim_end_matches('/')
        .rsplit(['/', ':'])
        .next()
        .map(|name| name.trim_end_matches(".git"))
        .filter(|name| !name.is_empty())
        .ok_or_else(|| anyhow!("cannot name a checkout for {origin}"))?;
    let root = crate::repo::checkouts_root()?;
    let path = root.join(name);
    anyhow::ensure!(
        !path.exists(),
        "{} exists but is not repository {repository} here; run `lf config repo --bind {repository}` in it, then move again",
        path.display()
    );
    fs::create_dir_all(&root)?;
    crate::git::git_stdout(&root, &["clone", "-q", origin, &path.to_string_lossy()])?;
    store
        .sqlite
        .bind_repository(&path.canonicalize()?.to_string_lossy(), repository)?;
    Ok(path.canonicalize()?)
}

/// This Machine owns the Task from here: the move's claim, then its checkout of
/// the carried branch, then its Graph where it stood and its Auto-Ship approval.
async fn take_in_task(
    store: &SharedStore,
    main: &Path,
    moved: &MovedTask,
    checkout: Option<&MovedCheckout>,
) -> Result<Task> {
    let name = &moved.identifier;
    arriving_task(store, main, &moved.task_id, name).await?;
    store
        .sqlite
        .ingest_ownership_marker(&moved.task_id, &moved.ownership_marker)?;
    if let Some(checkout) = checkout {
        let source = crate::ops::task::remote::TaskSource {
            branch: checkout.branch.clone(),
            commit: checkout.commit.clone(),
            task_id: moved.task_id.clone(),
            identifier: moved.identifier.clone(),
            issue_id: None,
        };
        let main = main.to_path_buf();
        // Task preparation runs its own runtime; it cannot nest in this one.
        std::thread::scope(|scope| {
            scope
                .spawn(|| crate::ops::task::check_out_moved_task(&main, source))
                .join()
                .expect("Task checkout thread panicked")
        })?;
    }
    if let Some(graph) = &moved.graph {
        store.sqlite.place_moved_task_graph(
            &moved.task_id,
            graph,
            crate::journal::current_lf_process_id().as_ref(),
        )?;
    }
    if let Some(approval) = &moved.approval {
        store
            .sqlite
            .take_in_ship_approval(&moved.task_id, approval)?;
    }
    store
        .get_task(&moved.task_id)
        .await?
        .ok_or_else(|| anyhow!("Task {name} disappeared while it arrived"))
}

/// Whether the placed transcript reads as turns here. A provider version this
/// Machine's reader does not know yields none.
fn readable(harness: HarnessKind, base: &Path, files: &[MovedFile]) -> bool {
    files.first().is_some_and(|file| {
        crate::transcript::read(harness, &Source::File(base.join(&file.path)))
            .is_ok_and(|turns| !turns.is_empty())
    })
}

fn write_file(base: &Path, file: &MovedFile) -> Result<()> {
    let relative = Path::new(&file.path);
    anyhow::ensure!(
        relative
            .components()
            .all(|part| matches!(part, std::path::Component::Normal(_))),
        "moved transcript path {} leaves its directory",
        file.path
    );
    let path = base.join(relative);
    fs::create_dir_all(path.parent().expect("a joined path has a parent"))?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&file.content)
        .with_context(|| format!("decode moved transcript {}", file.path))?;
    fs::write(&path, bytes).with_context(|| format!("write {}", path.display()))
}

/// The worktree holding `checkout.branch` at `checkout.commit` or later:
/// the main checkout for the default branch, else the branch's worktree,
/// created from origin when there is none.
fn check_out(main: &Path, checkout: &MovedCheckout) -> Result<PathBuf> {
    let branch = &checkout.branch;
    crate::git::fetch(main, "origin", branch)?;
    let existing = crate::git::worktrees::list_porcelain(main)?
        .into_iter()
        .find_map(|(path, at)| (at.as_deref() == Some(branch)).then_some(path));
    let path = match existing {
        Some(path) => path,
        None => {
            let leaf = branch
                .rsplit('/')
                .next()
                .unwrap_or(branch)
                .replace('.', "-");
            let segment = crate::git::worktrees::WorktreeSegment::parse(&leaf)?;
            let plan = crate::git::worktrees::plan_branch_placement(main, segment, Some(branch))?;
            crate::git::worktrees::create_from_placement_plan(main, &plan)?.path
        }
    };
    if !crate::git::is_ancestor(&path, &checkout.commit, "HEAD")? {
        anyhow::ensure!(
            crate::git::is_clean(&path)?
                && crate::git::is_ancestor(&path, "HEAD", &checkout.commit)?,
            "{} has work on {branch} that {} lacks; reconcile it there, then move again",
            path.display(),
            checkout.commit
        );
        crate::git::git_stdout(&path, &["merge", "--ff-only", &checkout.commit])?;
    }
    Ok(path)
}

/// The moved Session here, open in `cwd`: the row it left, reopened, or a new
/// row with its identity. A moved Task's Session is its conversation here.
async fn admit(
    store: &SharedStore,
    moved: &MovedSession,
    task: Option<&Task>,
    cwd: &Path,
    repo: &CanonicalRepo,
) -> Result<LfSession> {
    if store.session(&moved.id).await?.is_some() {
        store.sqlite.reopen_moved_session(&moved.id, cwd)?;
        return store
            .session(&moved.id)
            .await?
            .ok_or_else(|| session_not_found(&moved.id));
    }
    let session = LfSession {
        captured: None,
        id: moved.id.clone(),
        artifact_key: Some(crate::session_record::new_artifact_key()),
        caller_artifact_key: None,
        input_published: false,
        cwd: cwd.to_path_buf(),
        skill: None,
        provider: moved.provider.clone(),
        model: moved.model.clone(),
        node: None,
        iterations: None,
        task_id: task.map(|task| task.id.clone()),
        wave_id: match (&moved.primary, task) {
            (Some(MovedPrimary::Wave { wave_id }), _) => Some(wave_id.clone()),
            (_, Some(task)) => task.wave_id.clone(),
            _ => None,
        },
        graph_lf_process_id: None,
        // Task, Wave and repository conversations are managed wherever they move.
        work_source: (task.is_some() || moved.primary.is_some()).then_some(WorkSource::Declared),
        bound_at: None,
        interactive: true,
        repo: Some(repo.to_string()),
        title: moved.title.clone(),
        title_source: moved.title_source,
        request: None,
        ready_summary: None,
        completed_at: None,
        created_at: crate::store::rows::now_unix(),
    };
    let scope = match (&moved.primary, task) {
        (_, Some(task)) => PrimaryScope::Task(task.id.clone()),
        (None, None) => return Ok(store.create_session(session).await?),
        (Some(MovedPrimary::Repository), None) => PrimaryScope::Repository(repo.clone()),
        (Some(MovedPrimary::Wave { wave_id }), None) => PrimaryScope::Wave(wave_id.clone()),
    };
    let caller = crate::journal::current_lf_process_id();
    let admitted = store
        .sqlite
        .ensure_primary_session(&scope, session, caller.as_ref())?;
    anyhow::ensure!(
        admitted.id == moved.id,
        "this Machine already runs its own Session {} for that scope; complete it, then move again",
        admitted.id
    );
    Ok(admitted)
}

#[cfg(test)]
mod tests {
    use super::Move;

    #[test]
    fn a_move_round_trips_and_requires_its_source() {
        let json = include_str!("../../../../../tests/fixtures/dto/move.json");
        let moved: Move = serde_json::from_str(json).unwrap();
        assert_eq!(
            serde_json::from_str::<Move>(&serde_json::to_string(&moved).unwrap()).unwrap(),
            moved
        );
        let task = moved.task.unwrap();
        assert_eq!(task.graph.unwrap().step, "explore");
        assert_eq!(task.stopped.unwrap().step.as_deref(), Some("implement"));
        let mut missing: serde_json::Value = serde_json::from_str(json).unwrap();
        missing.as_object_mut().unwrap().remove("from_machine_id");
        assert!(serde_json::from_value::<Move>(missing).is_err());
    }
}
