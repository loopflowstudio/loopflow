//! A Session moves to another Machine: its branch through the remote, its
//! provider transcript inside the move, its identity unchanged. `send` runs
//! where it lives and completes it there, naming where it went; `receive`
//! checks out the branch where it arrives and admits the same Session, so the
//! same provider conversation resumes there.
//!
//! A Task moves the same way with its ownership and Graph: `send_task`
//! stops its Graph, checkpoints its worktree, takes its conversation along and
//! hands it off; `receive` takes in the handoff before checking it out.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use base64::Engine as _;
use serde::{Deserialize, Serialize};

use super::{provider_conversation, session_by_id, session_not_found};
use crate::durable::{MachineId, OwnershipKind, RepositoryId, TaskId, TaskOwnershipEntry, WorkRef};
use crate::harness::HarnessKind;
use crate::id::{AgentSessionId, LfSessionId, WaveId};
use crate::journal::ProcessIdentityEvidence;
use crate::process::Departure;
use crate::provider_auth::Provider;
use crate::repository::CanonicalRepo;
use crate::session::{LfSession, PrimaryScope, TitleSource, WorkSource};
use crate::store::sqlite::task_work::TaskGraphRecord;
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
    /// The handoff, as the marked comment planning would deliver later.
    pub ownership_marker: String,
    /// Absent before the Task takes up a Graph.
    pub graph: Option<TaskGraphRecord>,
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
/// reads the same Session again.
pub(crate) async fn send(store: &SharedStore, id: &str, to: &MachineId) -> Result<Move> {
    let session = session_by_id(store, id)
        .await?
        .ok_or_else(|| session_not_found(id))?;
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

/// Stop the Task's Graph, commit and push its worktree, take its conversation
/// and hand it off to `to`. Nothing is handed off when a step before fails;
/// repeating it after a failed receive reuses the handoff.
pub(crate) async fn send_task(store: &SharedStore, issue: &str, to: &MachineId) -> Result<Move> {
    let task = store
        .get_task_by_issue(issue)
        .await?
        .ok_or_else(|| anyhow!("no Task {issue} on this Machine"))?;
    let name = &task.identifier;
    let worktree = task.worktree.clone().filter(|path| path.exists());
    if let Some(root) = &worktree {
        refuse_mid_operation(root, name)?;
    }
    stop_graphs(store, &task).await?;
    let checkout = worktree
        .as_deref()
        .map(|root| checkpoint(&store.sqlite, root, name, to))
        .transpose()?;
    let conversation = match super::primary::task_conversation(store, &task.id).await? {
        Some(session) => Some(session),
        // Repeated after a failed receive: the conversation already left for `to`.
        None => match store.sqlite.task_conversation_moved_to(&task.id)? {
            Some((id, machine)) if machine == *to => session_by_id(store, &id).await?,
            _ => None,
        },
    };
    let taken = conversation
        .as_ref()
        .map(|session| {
            take(
                store,
                session,
                Departure::Machine {
                    machine: to.clone(),
                    task: Some(task.id.clone()),
                },
            )
        })
        .transpose()?;
    let handoff = hand_off(&store.sqlite, &task.id, to)?;
    if let Some(session) = &conversation {
        store.sqlite.complete_moved_session(&session.id, to)?;
    }
    let (session, history) =
        taken.map_or((None, None), |(session, history)| (Some(session), history));
    Ok(Move {
        from_machine_id: store.sqlite.local_machine()?.id,
        session,
        checkout,
        history,
        task: Some(MovedTask {
            task_id: task.id.clone(),
            identifier: task.identifier.clone(),
            ownership_marker: crate::store::sqlite::task_ownership::marker(&handoff)?,
            graph: store.sqlite.task_graph_record(&task.id)?,
        }),
    })
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

/// The handoff to `to`: this Machine's claim moves there. An unowned Task is
/// claimed first; one already handed to `to` reuses that entry.
fn hand_off(store: &SqliteStore, task: &TaskId, to: &MachineId) -> Result<TaskOwnershipEntry> {
    let ownership = store.task_ownership(task)?;
    let owner = ownership
        .owner
        .as_ref()
        .and_then(|owner| owner.machine.id.as_ref());
    if owner == Some(to) {
        if let Some(entry) = ownership.log.iter().rev().find(|entry| {
            entry.kind == OwnershipKind::Handoff && entry.machine_id.as_ref() == Some(to)
        }) {
            return Ok(entry.clone());
        }
    }
    if owner.is_none() {
        store.claim_task(task)?;
    }
    Ok(store.handoff_task(task, to)?)
}

/// Interrupt the Task so nothing starts again, then end each Graph still
/// running and the recorded step Processes it started. Each is signalled
/// only once its pid is proven to be that recorded Process.
async fn stop_graphs(store: &SharedStore, task: &Task) -> Result<()> {
    let live = |process: &crate::process::LfProcess| {
        process.completed_at.is_none()
            && crate::journal::process_evidence(&store.sqlite, &process.id)
                == ProcessIdentityEvidence::Live
    };
    let processes: Vec<crate::process::LfProcess> = store
        .sqlite
        .task_graphs(&task.id)?
        .into_iter()
        .filter(|graph| live(&graph.process))
        .flat_map(|graph| {
            std::iter::once(graph.process).chain(graph.steps.into_iter().map(|step| step.process))
        })
        .filter(|process| live(process))
        .collect();
    if processes.is_empty() {
        return Ok(());
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
                crate::journal::process_evidence(&store.sqlite, &process.id)
                    == ProcessIdentityEvidence::Live
            })
            .map(|process| process.id.to_string())
            .collect();
        if running.is_empty() {
            return Ok(());
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
    let located = crate::transcript::locate(
        store,
        harness,
        &thread,
        reference.account_id.as_ref(),
        &session.cwd,
    )?;
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

/// The Task a move carries, which this Machine must already know.
pub(crate) async fn known_task(store: &SharedStore, task: &TaskId, name: &str) -> Result<Task> {
    store.get_task(task).await?.ok_or_else(|| {
        anyhow!(
            "Task {name} is unknown on this Machine; sync its planning here, then move it again"
        )
    })
}

/// Check out the moved branch in a worktree of this repository, admit the
/// same Session there and place its transcript where its provider resumes it.
/// A moved Task is taken in first: its handoff, its checkout on the carried
/// branch and its Graph. Returns the Session, when one moved.
pub(crate) async fn receive(store: &SharedStore, moved: Move) -> Result<Option<LfSession>> {
    let repo = repository(store, &moved).await?;
    let main = crate::git::worktrees::main_repo_root(&repo)?;
    let task = match &moved.task {
        Some(task) => Some(take_in_task(store, &main, task, moved.checkout.as_ref()).await?),
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
    let home = crate::transcript::conversation_home(
        &store.sqlite,
        provider,
        &history.agent_session_id,
        None,
    )?
    .unwrap_or_else(|| crate::provider_account::activation::native_home(provider, None));
    let base = match provider {
        Provider::Claude => home.join("projects").join(claude_project(&cwd)),
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

/// This Machine's checkout of the moved repository, cloned from its origin
/// under `repo_root` when there is none.
async fn repository(store: &SharedStore, moved: &Move) -> Result<PathBuf> {
    let repository = match (&moved.checkout, &moved.task) {
        (Some(checkout), _) => checkout.repository_id.clone(),
        (None, Some(task)) => {
            known_task(store, &task.task_id, &task.identifier)
                .await?
                .repository_id
        }
        (None, None) => bail!("the move names neither a checkout nor a Task"),
    };
    if let Some(path) = store.sqlite.repository_path(&repository)? {
        return Ok(PathBuf::from(path));
    }
    let checkout = moved
        .checkout
        .as_ref()
        .ok_or_else(|| anyhow!("repository {repository} has no checkout on this Machine"))?;
    let name = checkout
        .origin
        .trim_end_matches('/')
        .rsplit(['/', ':'])
        .next()
        .map(|name| name.trim_end_matches(".git"))
        .filter(|name| !name.is_empty())
        .ok_or_else(|| anyhow!("cannot name a checkout for {}", checkout.origin))?;
    let root = crate::repo::checkouts_root()?;
    let path = root.join(name);
    anyhow::ensure!(
        !path.exists(),
        "{} exists but is not repository {repository} here; run `lf repository --bind {repository}` in it, then move again",
        path.display()
    );
    fs::create_dir_all(&root)?;
    crate::git::git_stdout(
        &root,
        &["clone", "-q", &checkout.origin, &path.to_string_lossy()],
    )?;
    store
        .sqlite
        .bind_repository(&path.canonicalize()?.to_string_lossy(), &repository)?;
    Ok(path.canonicalize()?)
}

/// This Machine owns the Task from here: the handoff, then its checkout of
/// the carried branch, then its Graph where it stood.
async fn take_in_task(
    store: &SharedStore,
    main: &Path,
    moved: &MovedTask,
    checkout: Option<&MovedCheckout>,
) -> Result<Task> {
    let name = &moved.identifier;
    known_task(store, &moved.task_id, name).await?;
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

/// Claude names a project directory for its absolute path, every character
/// other than a letter or digit replaced by `-`.
fn claude_project(cwd: &Path) -> String {
    cwd.to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
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
        work_source: task.map(|_| WorkSource::Declared),
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
        .ensure_primary_session(&scope, None, session, caller.as_ref())?;
    anyhow::ensure!(
        admitted.id == moved.id,
        "this Machine already runs its own Session {} for that scope; complete it, then move again",
        admitted.id
    );
    Ok(admitted)
}

#[cfg(test)]
mod tests {
    use super::{claude_project, Move};
    use std::path::Path;

    #[test]
    fn a_move_round_trips_and_requires_its_source() {
        let json = include_str!("../../../../../tests/fixtures/dto/move.json");
        let moved: Move = serde_json::from_str(json).unwrap();
        assert_eq!(
            serde_json::from_str::<Move>(&serde_json::to_string(&moved).unwrap()).unwrap(),
            moved
        );
        assert_eq!(moved.task.unwrap().graph.unwrap().step, "explore");
        let mut missing: serde_json::Value = serde_json::from_str(json).unwrap();
        missing.as_object_mut().unwrap().remove("from_machine_id");
        assert!(serde_json::from_value::<Move>(missing).is_err());
    }

    #[test]
    fn claude_names_a_project_for_its_path() {
        assert_eq!(
            claude_project(Path::new("/Users/jack/src/loopflow.loo-628")),
            "-Users-jack-src-loopflow-loo-628"
        );
    }
}
