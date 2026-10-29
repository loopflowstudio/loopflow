//! The one ongoing conversation of a scope. A primary Session is an ordinary
//! interactive conversation; its row names the scope it is primary for.

use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};

use super::{
    lock_session_process, publish_prepared_input, session_not_found, surface, SessionRecord,
};
use crate::session::{conversation, LfSession, PrimaryScope, WorkSource};
use crate::store::SharedStore;

/// Find or admit the primary conversation of a Wave, or of the repository
/// itself, and prepare its workspace; `true` when it was admitted now. The
/// repository's needs no Wave, Task or planning provider.
pub(crate) async fn ensure(
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
        None => PrimaryScope::Repository(crate::repository::CanonicalRepo::discover(repo)?),
    };
    let _lock = lock_scope(&scope).await?;
    if let Some(previous) = store.sqlite.primary_session(&scope)? {
        return Ok((prepare(store, &previous.id).await?, false));
    }
    let session = match &binding {
        Some(binding) => wave_session(binding)?,
        None => repository_session(&crate::repository::CanonicalRepo::discover(repo)?)?,
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
    if task.worktree.as_deref().is_some_and(|path| !path.exists()) {
        // Reuse Task preparation's recorded placement and restoration checks.
        // Opening a conversation does not invent a checkout or start a Graph.
        let repo = repo.to_path_buf();
        let id = task.id.to_string();
        tokio::task::spawn_blocking(move || {
            crate::ops::task::task_checkout(&repo, &id, Default::default())
        })
        .await??;
    }
    let chosen = task_conversation(store, &task.id).await?;
    let started = chosen.is_none();
    let session = if let Some(session) = chosen {
        store.choose_task_primary(&task.id, &session.id).await?
    } else {
        let cwd = task.worktree.as_deref().unwrap_or(repo);
        let candidate = LfSession {
            task_id: Some(task.id.clone()),
            wave_id: task.wave_id.clone(),
            work_source: Some(WorkSource::Declared),
            request,
            ..conversation(cwd, task.agent.as_deref(), "task/session", task.title)?
        };
        store.ensure_primary_session(&scope, candidate).await?
    };
    Ok((prepare(store, &session.id).await?, started))
}

pub(super) async fn task_conversation(
    store: &SharedStore,
    task: &crate::durable::TaskId,
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

pub(crate) fn ensure_scope_worktree(
    repo: &Path,
    scope: &PrimaryScope,
) -> Result<crate::git::worktrees::AgentWorktree> {
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
    })
}

/// Adapt only at an idle attachment boundary. Historical captures and native
/// conversation identity remain attached to the same Session.
pub(crate) async fn admit_workspace(
    store: &SharedStore,
    mut session: LfSession,
) -> Result<LfSession> {
    let Some(scope) = store.sqlite.primary_scope(&session.id)? else {
        return Ok(session);
    };
    if store
        .sqlite
        .session_summary(
            &session.id,
            time::OffsetDateTime::now_utc().unix_timestamp(),
        )?
        .is_some_and(|summary| {
            summary.agent_process_id.is_some()
                && summary.agent_process_evidence != crate::journal::ProcessIdentityEvidence::Dead
        })
    {
        return Ok(session);
    }
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
            let task = store
                .get_task(task)
                .await?
                .ok_or_else(|| anyhow!("primary Task is unavailable"))?;
            if let Some(cwd) = task.worktree.filter(|cwd| cwd.is_dir()) {
                cwd
            } else {
                let task = task.id.to_string();
                let repo = session
                    .repo
                    .as_deref()
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| session.cwd.clone());
                tokio::task::spawn_blocking(move || -> Result<std::path::PathBuf> {
                    let task = crate::ops::task::task_checkout(&repo, &task, Default::default())?;
                    Ok(task.worktree()?.clone())
                })
                .await??
            }
        }
    };
    if session.cwd != cwd {
        store.sqlite.move_primary_workspace(&session, &cwd)?;
        session.cwd = cwd;
    }
    Ok(session)
}

fn wave_session(binding: &crate::ops::WorkBinding) -> Result<LfSession> {
    Ok(LfSession {
        wave_id: binding.wave_id.clone(),
        work_source: Some(WorkSource::Declared),
        ..conversation(
            &binding.cwd,
            binding.agent.as_deref(),
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
        ..conversation(repo.as_path(), None, "repo/session", title)?
    })
}

/// Prepare the workspace; connecting owns input reservation and provider launch.
async fn prepare(store: &SharedStore, id: &str) -> Result<SessionRecord> {
    let lock_id = id.to_string();
    let _launch = tokio::task::spawn_blocking(move || lock_session_process(&lock_id)).await??;
    let session = store
        .session(id)
        .await?
        .ok_or_else(|| session_not_found(id))?;
    if store.sqlite.session_transport(&session.id)?.is_some()
        || session
            .artifact_key
            .as_deref()
            .map(|key| store.sqlite.input_provider_session(key))
            .transpose()?
            .flatten()
            .is_some()
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
    let key = match scope {
        PrimaryScope::Repository(repo) => format!("primary:repository:{repo}"),
        PrimaryScope::Wave(wave) => format!("primary:wave:{wave}"),
        PrimaryScope::Task(task) => format!("primary:task:{task}"),
    };
    tokio::task::spawn_blocking(move || lock_session_process(&key))
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
            let canonical = crate::repository::CanonicalRepo::discover(repo.path()).unwrap();
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
