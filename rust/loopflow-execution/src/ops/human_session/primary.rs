//! The ongoing conversation of a scope. Shared scopes mark their primary
//! Session; a Task points to one of its existing conversations.

use crate::store::session_projection;
use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};

use super::{
    lock_session_process, publish_prepared_input, session_not_found, surface, SessionRecord,
};
use crate::session::{conversation, LfSession, PrimaryScope, WorkSource};
use crate::store::SharedStore;
use loopflow_model::durable::WorkStatus;

/// Find or admit the primary conversation of a Wave, or of the repository
/// itself, and prepare its workspace; `true` when it was admitted now. The
/// repository's needs no Wave, Task or tracker.
pub async fn ensure(
    store: &SharedStore,
    repo: &Path,
    wave: Option<&str>,
) -> Result<(SessionRecord, bool)> {
    let binding = match wave {
        Some(wave) => {
            Some(crate::ops::resolve_work_binding(store, repo, &format!("wave:{wave}")).await?)
        }
        None => None,
    };
    let scope = match &binding {
        Some(binding) => {
            PrimaryScope::Wave(binding.wave_id.clone().expect("explicit Wave selection"))
        }
        None => PrimaryScope::Repository(crate::repository::discover(repo)?),
    };
    let _lock = lock_scope(&scope).await?;
    if let Some(previous) = store.sqlite.primary_session(&scope)? {
        return Ok((prepare(store, &previous.id).await?, false));
    }
    let session = match &binding {
        Some(binding) => wave_session(binding)?,
        None => repository_session(&crate::repository::discover(repo)?)?,
    };
    let session = store.ensure_primary_session(&scope, session).await?;
    Ok((prepare(store, &session.id).await?, true))
}

/// A Task's primary is one of its own conversations: the one already chosen,
/// else its sole unfinished interactive one, else the most recently used.
/// Only a Task with none gets a new one, in its checkout; `true` then. The
/// others stay open. `request` is what a person typed to start the Task; a
/// new conversation takes it as its first turn.
pub(crate) async fn ensure_task(
    store: &SharedStore,
    repo: &Path,
    task: &str,
    request: Option<String>,
) -> Result<(SessionRecord, bool)> {
    let selected = crate::ops::run::select_work(
        store,
        repo,
        crate::ops::WorkSelection {
            task: Some(task),
            wave: None,
        },
    )
    .await?
    .ok_or_else(|| anyhow!("Task {task} is unavailable"))?;
    let task = selected
        .task
        .expect("explicit Task selection includes its Task");
    let scope = PrimaryScope::Task(task.id.clone());
    let _lock = lock_scope(&scope).await?;
    let open = crate::ops::task::task_work_status(store, &task).await? == WorkStatus::Ready;
    if open && task.worktree.as_deref().is_some_and(|path| !path.exists()) {
        // Reuse Task preparation's recorded placement and restoration checks.
        // Opening a conversation does not invent a checkout or start a Graph.
        let repo = repo.to_path_buf();
        let id = task.id.to_string();
        crate::journal::request::spawn_blocking(move || {
            crate::ops::task::task_checkout(&repo, &id, Default::default())
        })
        .await??;
    }
    // A closed Task's checkout is retired and stays gone.
    let cwd = match &task.worktree {
        None if open => repo.to_path_buf(),
        _ => crate::ops::task::task_directory(store, &task).await?,
    };
    let (session, started) = record_task_primary(store, &cwd, &task, request).await?;
    Ok((prepare(store, &session.id).await?, started))
}

async fn record_task_primary(
    store: &SharedStore,
    cwd: &Path,
    task: &loopflow_storage::work::task::Task,
    request: Option<String>,
) -> Result<(LfSession, bool)> {
    if let Some(session) = task_conversation(store, &task.id).await? {
        return Ok((
            store.choose_task_primary(&task.id, &session.id).await?,
            false,
        ));
    }
    let candidate = LfSession {
        task_id: Some(task.id.clone()),
        wave_id: task.wave_id.clone(),
        work_source: Some(WorkSource::Declared),
        request,
        ..conversation(cwd, "task/session", task.title.clone())?
    };
    let scope = PrimaryScope::Task(task.id.clone());
    Ok((store.ensure_primary_session(&scope, candidate).await?, true))
}

/// Start a new Task's recorded conversation in a held terminal, where it
/// makes its checkout and starts its Graph before its provider. Setup needs
/// no agent; a missing one is reported in the terminal once setup is done.
/// The creation's `agent` runs the conversation and its Graph.
pub(crate) async fn hold_task_conversation(
    store: &SharedStore,
    task: &loopflow_storage::work::task::Task,
    session: &LfSession,
    agent: Option<&str>,
    request_id: &str,
) -> Result<bool> {
    let wave = match &task.wave_id {
        Some(id) => store.get_wave(id).await?,
        None => None,
    };
    hold(
        store,
        &session.id,
        &session.cwd,
        wave,
        agent,
        Some(request_id),
    )
    .await
}

async fn hold(
    store: &SharedStore,
    session: &loopflow_model::id::LfSessionId,
    cwd: &Path,
    wave: Option<crate::work::wave::Wave>,
    agent: Option<&str>,
    request_id: Option<&str>,
) -> Result<bool> {
    // Prepared conversations can start; previously opened ones stay as they are.
    if opened(store, session.as_str())? {
        return Ok(false);
    }
    let mut argv = vec![crate::os_process::resolve_lf_binary()?
        .to_string_lossy()
        .into_owned()];
    if let Some(agent) = agent {
        argv.extend(["--agent".into(), agent.into()]);
    }
    argv.extend(["session".into(), "resume".into(), session.to_string()]);
    if let Some(request_id) = request_id {
        argv.extend(["--request-id".into(), request_id.into()]);
    }
    let (held, cwd) = (session.clone(), cwd.to_path_buf());
    tokio::task::spawn_blocking(move || {
        crate::transport::start_held(&argv, &cwd, Some(&held), &|command| {
            let key = crate::work::wave::context::WAVE_ID_ENV;
            match &wave {
                Some(wave) => command.env(key, wave.id().as_str()),
                None => command.env_remove(key),
            };
        })
    })
    .await??;
    Ok(true)
}

/// Attached, held, or with its agent running wherever it was started.
fn opened(store: &SharedStore, session: &str) -> Result<bool> {
    if store.sqlite.session_attachment(session)?.is_some()
        || store.sqlite.session_terminal(session)?.is_some()
    {
        return Ok(true);
    }
    let summary = session_projection::session_summary(
        &store.sqlite,
        session,
        time::OffsetDateTime::now_utc().unix_timestamp(),
    )?;
    Ok(summary.is_some_and(|summary| {
        summary.agent_process_id.is_some()
            && summary.agent_process_evidence != crate::store::ProcessIdentityEvidence::Dead
    }))
}

pub(crate) async fn task_conversation(
    store: &SharedStore,
    task: &loopflow_model::durable::TaskId,
) -> Result<Option<LfSession>> {
    if let Some(primary) = store.task_primary(task).await? {
        return Ok(Some(primary));
    }
    // Rank native human input, then last interactive opening, then creation.
    // Assistant output never changes the choice.
    let candidates = store.task_conversations(task).await?;
    let human = super::provider_conversation::human_input_times(
        store,
        candidates.iter().map(|(session, _)| session),
    )
    .await?;
    Ok(candidates
        .into_iter()
        .max_by_key(|(session, opened)| {
            (
                human
                    .get(session.id.as_str())
                    .copied()
                    .or(*opened)
                    .unwrap_or(session.created_at.saturating_mul(1000)),
                session.id.clone(),
            )
        })
        .map(|(session, _)| session))
}

pub(super) fn scope_worktree(
    repo: &Path,
    scope: &PrimaryScope,
) -> Result<crate::git::worktrees::PlacementPlan> {
    Ok(crate::git::worktrees::plan_placement(
        repo,
        scope_segment(scope)?,
    )?)
}

pub fn ensure_scope_worktree(
    repo: &Path,
    scope: &PrimaryScope,
) -> Result<crate::git::worktrees::Worktree> {
    Ok(crate::git::worktrees::ensure_agent_worktree(
        repo,
        scope_segment(scope)?,
    )?)
}

fn scope_segment(scope: &PrimaryScope) -> Result<crate::git::worktrees::WorktreeSegment> {
    use crate::git::worktrees::{wave_agent_segment, WorktreeSegment};
    Ok(match scope {
        PrimaryScope::Repository(_) => WorktreeSegment::parse("repo")?,
        PrimaryScope::Wave(id) => wave_agent_segment(id.as_str())?,
        PrimaryScope::Task(id) => bail!("Task {id}'s conversations live in its own checkout"),
        _ => bail!("Unsupported conversation scope: {scope:?}"),
    })
}

/// Adapt only at an idle attachment boundary. Historical captures and native
/// conversation identity remain attached to the same Session.
pub async fn admit_workspace(store: &SharedStore, session: LfSession) -> Result<LfSession> {
    let summary = session_projection::session_summary(
        &store.sqlite,
        &session.id,
        time::OffsetDateTime::now_utc().unix_timestamp(),
    )?;
    if summary.as_ref().is_some_and(|summary| {
        summary.agent_process_id.is_some()
            && summary.agent_process_evidence != crate::store::ProcessIdentityEvidence::Dead
    }) {
        return Ok(session);
    }
    let session = match store.sqlite.primary_scope(&session.id)? {
        Some(scope) => admit_scope_workspace(store, session, scope).await?,
        None => session,
    };
    leave_gone_directory(store, session).await
}

/// A conversation whose directory is gone, such as a closed Task's retired
/// checkout, continues in its repository's main checkout.
async fn leave_gone_directory(store: &SharedStore, session: LfSession) -> Result<LfSession> {
    if session.cwd.is_dir() {
        return Ok(session);
    }
    let recorded = match &session.task_id {
        Some(task) => match store.get_task(task).await? {
            Some(task) => store.sqlite.repository_path(&task.repository_id)?,
            None => None,
        },
        None => None,
    };
    let recorded = match (recorded.or_else(|| session.repo.clone()), &session.wave_id) {
        (Some(repo), _) => Some(repo),
        (None, Some(wave)) => store
            .get_wave(wave)
            .await?
            .map(|wave| wave.repo().to_string()),
        (None, None) => None,
    };
    // With no repository on record the launch reports the missing directory.
    let Some(root) = recorded
        .map(std::path::PathBuf::from)
        .filter(|repo| repo.is_dir())
        .and_then(|repo| crate::git::worktrees::main_repo_root(&repo).ok())
    else {
        return Ok(session);
    };
    store.sqlite.relocate_session(&session.id, &root)?;
    store
        .session(&session.id)
        .await?
        .ok_or_else(|| session_not_found(&session.id))
}

async fn admit_scope_workspace(
    store: &SharedStore,
    session: LfSession,
    scope: PrimaryScope,
) -> Result<LfSession> {
    let cwd = match &scope {
        PrimaryScope::Repository(repo) => ensure_scope_worktree(repo.as_path(), &scope)?.path,
        PrimaryScope::Wave(id) => {
            let wave = store
                .get_wave(id)
                .await?
                .ok_or_else(|| anyhow!("primary Wave {id} is unavailable"))?;
            ensure_scope_worktree(Path::new(wave.repo()), &scope)?.path
        }
        PrimaryScope::Task(task) => {
            // Explicit adoption and binding already selected this directory.
            if session.work_source == Some(WorkSource::Bound) {
                return Ok(session);
            }
            let task = store
                .get_task(task)
                .await?
                .ok_or_else(|| anyhow!("primary Task is unavailable"))?;
            if task.worktree.as_ref().is_some_and(|root| {
                crate::git::worktree_root(&session.cwd).ok().as_ref() == Some(root)
            }) {
                return Ok(session);
            }
            if let Some(cwd) = task.worktree.clone().filter(|cwd| cwd.is_dir()) {
                cwd
            } else if crate::ops::task::task_work_status(store, &task).await? != WorkStatus::Ready {
                // A closed Task no longer owns a checkout to rebuild.
                return Ok(session);
            } else {
                let task = task.id.to_string();
                let repo = session
                    .repo
                    .as_deref()
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| session.cwd.clone());
                crate::journal::request::spawn_blocking(move || -> Result<std::path::PathBuf> {
                    let task = crate::ops::task::task_checkout(&repo, &task, Default::default())?;
                    Ok(task.worktree()?.clone())
                })
                .await??
            }
        }
        _ => bail!("Unsupported conversation scope: {scope:?}"),
    };
    if session.cwd != cwd {
        return Ok(store.sqlite.admit_session_workspace(&session, &cwd)?);
    }
    Ok(session)
}

fn wave_session(binding: &crate::ops::WorkBinding) -> Result<LfSession> {
    Ok(LfSession {
        wave_id: binding.wave_id.clone(),
        work_source: Some(WorkSource::Declared),
        ..conversation(
            &binding.cwd,
            "wave/session",
            binding.wave_name.clone().expect("Wave primary has a Wave"),
        )?
    })
}

fn repository_session(repo: &crate::repository::CanonicalRepo) -> Result<LfSession> {
    let title = repo
        .as_path()
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| repo.to_string());
    Ok(LfSession {
        repo: Some(repo.to_string()),
        work_source: Some(WorkSource::Declared),
        ..conversation(repo.as_path(), "repo/session", title)?
    })
}

/// A new Task's conversation as a client opens it, read without preparing
/// its workspace: its own setup makes the checkout.
pub(crate) async fn admitted_record(store: &SharedStore, id: &str) -> Result<SessionRecord> {
    let session = store
        .session(id)
        .await?
        .ok_or_else(|| session_not_found(id))?;
    surface(store, &session).await
}

/// Prepare the workspace; connecting owns input reservation and provider launch.
async fn prepare(store: &SharedStore, id: &str) -> Result<SessionRecord> {
    let lock_id = id.to_string();
    let _launch =
        crate::journal::request::spawn_blocking(move || lock_session_process(&lock_id)).await??;
    let session = store
        .session(id)
        .await?
        .ok_or_else(|| session_not_found(id))?;
    if super::resume_intent(&super::current_summary(store, &session.id)?)
        != super::SessionResumeIntent::Start
    {
        // Retained work is attached when shown, never started from here.
        let mut record = surface(store, &session).await?;
        record.open_argv.clear();
        return Ok(record);
    }
    let session = admit_workspace(store, session).await?;
    let session = if session.captured.is_none() || session.input_published {
        session
    } else {
        publish_prepared_input(
            &store.sqlite,
            &session,
            crate::session_record::SessionGraphMembership::Independent,
        )?
    };
    surface(store, &session).await
}

/// Admission and replacement of one scope are serial across processes.
async fn lock_scope(scope: &PrimaryScope) -> Result<std::fs::File> {
    let key = scope.lock_key();
    crate::journal::request::spawn_blocking(move || lock_session_process(&key))
        .await
        .context("lock primary Session scope")?
}

#[cfg(test)]
mod tests {
    use super::ensure;
    use crate::ops::human_session::tests::SessionHome;

    async fn wave(
        store: &crate::store::SharedStore,
        repo: &loopflow_test_support::TestRepo,
    ) -> crate::work::wave::Wave {
        crate::work::wave::ensure_wave_row(store, repo.path(), "infrastructure")
            .await
            .unwrap()
    }

    #[test]
    fn ensure_prepares_the_same_wave_conversation_without_launching() {
        let _lock = crate::journal::test_env_lock();
        let home = SessionHome::new();
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let store = home.store().await;
            let repo = loopflow_test_support::TestRepo::new();
            let wave = wave(&store, &repo).await;

            let (first, started) = ensure(&store, repo.path(), Some("infrastructure"))
                .await
                .unwrap();
            let (second, again) = ensure(&store, repo.path(), Some("infrastructure"))
                .await
                .unwrap();

            assert_eq!(first.id, second.id);
            assert!(started && !again);
            assert!(store
                .sqlite
                .session_attachment(&first.id)
                .unwrap()
                .is_none());
            let session = store.session(&first.id).await.unwrap().unwrap();
            assert!(session.interactive && !session.input_published);
            assert!(session.artifact_key.is_none() && session.captured.is_none());
            assert_eq!(session.skill.as_deref(), Some("wave-session"));
            assert_eq!(session.wave_id.as_ref(), Some(wave.id()));
            assert_eq!(session.task_id, None);
        });
    }

    #[test]
    fn a_repository_without_waves_has_its_own_conversation() {
        let _lock = crate::journal::test_env_lock();
        let home = SessionHome::new();
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let store = home.store().await;
            let repo = loopflow_test_support::TestRepo::new();

            let (first, _) = ensure(&store, repo.path(), None).await.unwrap();
            let (second, _) = ensure(&store, repo.path(), None).await.unwrap();

            assert_eq!(first.id, second.id);
            let session = store.session(&first.id).await.unwrap().unwrap();
            assert_eq!(session.skill.as_deref(), Some("repo-session"));
            assert_eq!((session.wave_id, session.task_id), (None, None));
            // Managed like Task and Wave conversations: held, with hooks.
            assert_eq!(
                session.work_source,
                Some(crate::session::WorkSource::Declared)
            );

            // A Wave's conversation in the same repository is a separate scope.
            let ambient_wave = wave(&store, &repo).await;
            std::env::set_var("LF_WAVE_ID", ambient_wave.id().as_str());
            assert_eq!(
                ensure(&store, repo.path(), None).await.unwrap().0.id,
                first.id
            );

            // Explicit selection also ignores stale ambient Wave context.
            std::env::set_var("LF_WAVE_ID", "unknown-wave");
            let (wave, _) = ensure(&store, repo.path(), Some("infrastructure"))
                .await
                .unwrap();
            assert_ne!(wave.id, first.id);
        });
    }

    #[test]
    fn idle_primary_adopts_persistent_workspace_without_losing_identity() {
        let _lock = crate::journal::test_env_lock();
        let home = SessionHome::new();
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let store = home.store().await;
            let repo = loopflow_test_support::TestRepo::new();
            let canonical = crate::repository::discover(repo.path()).unwrap();
            let previous = store
                .ensure_primary_session(
                    &crate::session::PrimaryScope::Repository(canonical.clone()),
                    super::repository_session(&canonical).unwrap(),
                )
                .await
                .unwrap();
            let (first, started) = ensure(&store, repo.path(), None).await.unwrap();
            assert!(!started);
            let admitted = store.session(&first.id).await.unwrap().unwrap();
            assert_eq!(admitted.id, previous.id);
            assert_eq!(admitted.title, previous.title);
            assert_eq!(admitted.artifact_key, previous.artifact_key);
            assert_ne!(admitted.cwd, previous.cwd);
        });
    }
}
