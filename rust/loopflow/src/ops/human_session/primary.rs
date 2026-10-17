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
/// itself, and prepare its workspace. The repository's needs no Wave, Task
/// or planning provider.
pub(crate) async fn ensure(
    store: &SharedStore,
    repo: &Path,
    wave: Option<&str>,
) -> Result<SessionRecord> {
    let (scope, mut session) = match wave {
        Some(wave) => {
            let binding =
                crate::ops::resolve_work_binding(store, repo, &format!("wave:{wave}")).await?;
            (
                PrimaryScope::Wave(binding.wave_id.clone()),
                wave_session(&binding),
            )
        }
        None => {
            let repo = crate::repository::CanonicalRepo::discover(repo)?;
            let session = repository_session(&repo);
            (PrimaryScope::Repository(repo), session)
        }
    };
    let _lock = lock_scope(&scope).await?;
    session.cwd = ensure_scope_worktree(&session.cwd, &scope)?.path;
    let session = store.ensure_primary_session(&scope, None, session).await?;
    prepare(store, &session.id).await
}

/// A Task's primary is one of its own conversations: the one already chosen,
/// else its sole unfinished interactive one, else the most recently used.
/// Only a Task with none gets a new one, in its checkout. `choose` names the
/// conversation outright; the others stay open.
pub(crate) async fn ensure_task(
    store: &SharedStore,
    repo: &Path,
    task: &str,
    choose: Option<&str>,
) -> Result<SessionRecord> {
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
    let chosen = match choose {
        Some(session) => Some(session.to_string()),
        None => task_conversation(store, &task.id)
            .await?
            .map(|session| session.id),
    };
    let session = if let Some(id) = chosen {
        store.choose_task_primary(&task.id, &id).await?
    } else {
        let cwd = task.worktree.as_deref().unwrap_or(repo);
        let candidate = LfSession {
            task_id: Some(task.id.clone()),
            wave_id: Some(task.wave_id.clone()),
            work_source: Some(WorkSource::Declared),
            ..conversation(cwd, task.agent.as_deref(), "task-session", task.title)
        };
        store
            .ensure_primary_session(&scope, None, candidate)
            .await?
    };
    prepare(store, &session.id).await
}

pub(super) async fn task_conversation(
    store: &SharedStore,
    task: &crate::durable::TaskId,
) -> Result<Option<LfSession>> {
    if let Some(primary) = store.task_primary(task).await? {
        return Ok(Some(primary));
    }
    super::most_recent(store, store.task_conversations(task).await?).await
}

/// Give the scope a fresh conversation. The predecessor's provider stops
/// first; if it cannot be stopped, the predecessor stays primary.
pub(crate) async fn replace(store: &SharedStore, id: &str) -> Result<SessionRecord> {
    crate::data_clone::require_writable_session(id)?;
    let scope = store
        .sqlite
        .primary_scope(id)?
        .ok_or_else(|| anyhow!("Session {id} is not a primary Session"))?;
    let _lock = lock_scope(&scope).await?;
    let previous = store
        .session(id)
        .await?
        .ok_or_else(|| session_not_found(id))?;
    if previous.completed_at.is_none() {
        let lock_id = previous.id.clone();
        let _launch = tokio::task::spawn_blocking(move || lock_session_process(&lock_id)).await??;
        stop_agent(store, &previous)
            .with_context(|| format!("stop primary Session {id}; it remains primary"))?;
    }
    let mut successor = match &scope {
        PrimaryScope::Repository(repo) => repository_session(repo),
        PrimaryScope::Wave(wave) => wave_session(
            &crate::ops::resolve_work_binding(store, &previous.cwd, &format!("wave:{wave}"))
                .await?,
        ),
        PrimaryScope::Task(task) => task_session(
            store,
            &crate::ops::resolve_work_binding(store, &previous.cwd, &format!("task:{task}"))
                .await?,
            task,
        )?,
    };
    if !matches!(scope, PrimaryScope::Task(_)) {
        successor.cwd = ensure_scope_worktree(&successor.cwd, &scope)?.path;
    }
    let session = store
        .ensure_primary_session(&scope, Some(id), successor)
        .await?;
    prepare(store, &session.id).await
}

pub(crate) fn ensure_scope_worktree(
    repo: &Path,
    scope: &PrimaryScope,
) -> Result<crate::git::worktrees::AgentWorktree> {
    use crate::git::worktrees::{ensure_agent_worktree, wave_agent_segment, WorktreeSegment};
    let segment = match scope {
        PrimaryScope::Repository(_) => WorktreeSegment::parse("repo")?,
        PrimaryScope::Wave(id) => wave_agent_segment(id.as_str())?,
        PrimaryScope::Task(id) => bail!("Task {id}'s conversations live in its own checkout"),
    };
    Ok(ensure_agent_worktree(repo, segment)?)
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
    let repo = match &scope {
        PrimaryScope::Repository(repo) => repo.as_path().to_path_buf(),
        PrimaryScope::Wave(id) => {
            let wave = store
                .get_wave(id)
                .await?
                .ok_or_else(|| anyhow!("primary Wave {id} is unavailable"))?;
            std::path::PathBuf::from(wave.repo())
        }
        PrimaryScope::Task(task) => {
            let task = store
                .get_task(task)
                .await?
                .ok_or_else(|| anyhow!("primary Task is unavailable"))?;
            let cwd = if let Some(cwd) = task.worktree.filter(|cwd| cwd.is_dir()) {
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
            };
            if session.cwd != cwd {
                store.sqlite.move_primary_workspace(&session, &cwd)?;
                session.cwd = cwd;
            }
            return Ok(session);
        }
    };
    let workspace = ensure_scope_worktree(&repo, &scope)?;
    if session.cwd != workspace.path {
        store
            .sqlite
            .move_primary_workspace(&session, &workspace.path)?;
        session.cwd = workspace.path;
    }
    Ok(session)
}

fn wave_session(binding: &crate::ops::WorkBinding) -> LfSession {
    LfSession {
        wave_id: Some(binding.wave_id.clone()),
        work_source: Some(WorkSource::Declared),
        ..conversation(
            &binding.cwd,
            binding.agent.as_deref(),
            "wave-session",
            binding.wave_name.clone(),
        )
    }
}

fn task_session(
    store: &SharedStore,
    binding: &crate::ops::WorkBinding,
    task: &crate::durable::TaskId,
) -> Result<LfSession> {
    let plan = store
        .sqlite
        .task(task)?
        .ok_or_else(|| anyhow!("Task {task} is unavailable"))?;
    let title = crate::naming::generated_session_title(None, None, Some(&plan.title), &binding.cwd);
    Ok(LfSession {
        task_id: Some(task.clone()),
        wave_id: Some(binding.wave_id.clone()),
        work_source: Some(WorkSource::Declared),
        ..conversation(
            &binding.cwd,
            binding.agent.as_deref(),
            "task-session",
            title,
        )
    })
}

fn repository_session(repo: &crate::repository::CanonicalRepo) -> LfSession {
    let title = repo
        .as_path()
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| repo.to_string());
    LfSession {
        repo: Some(repo.to_string()),
        ..conversation(repo.as_path(), None, "repo-session", title)
    }
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
        return super::prepare_attachment(store, &session.id, false, true).await;
    }
    let session = admit_workspace(store, session).await?;
    let session = if session.captured.is_none() || session.input_published {
        session
    } else {
        publish_prepared_input(
            &store.sqlite,
            &session,
            crate::session_record::SessionFlowMembership::Independent,
        )?
    };
    surface(store, &session).await
}

fn stop_agent(store: &SharedStore, session: &LfSession) -> Result<()> {
    #[cfg(test)]
    if session
        .artifact_key
        .as_ref()
        .is_some_and(super::action_test::stop)
    {
        return Ok(());
    }
    crate::harness::agent_process::stop_session_agent(&store.sqlite, &session.id)
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
    use super::{ensure, replace};
    use crate::ops::human_session::tests::SessionHome;
    use crate::ops::human_session::{action_test::NativeClients, append_input};

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

            let first = ensure(&store, repo.path(), Some("infrastructure"))
                .await
                .unwrap();
            let second = ensure(&store, repo.path(), Some("infrastructure"))
                .await
                .unwrap();

            assert_eq!(first.id, second.id);
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
    fn replace_stops_the_predecessor_and_repeats_to_the_same_successor() {
        let _lock = crate::journal::test_env_lock();
        let home = SessionHome::new();
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let store = home.store().await;
            let repo = loopflow_test_support::TestRepo::new();
            wave(&store, &repo).await;
            let first = ensure(&store, repo.path(), Some("infrastructure"))
                .await
                .unwrap();
            let previous = append_input(&store, store.session(&first.id).await.unwrap().unwrap())
                .await
                .unwrap();
            let clients = NativeClients::new(
                &first.id,
                std::slice::from_ref(previous.input_key().unwrap()),
            );

            let successor = replace(&store, &first.id).await.unwrap();

            assert_ne!(successor.id, first.id);
            assert!(clients.active().is_empty());
            assert!(store
                .session(&first.id)
                .await
                .unwrap()
                .unwrap()
                .completed_at
                .is_some());
            // A repeat after a lost response names the replaced predecessor.
            assert_eq!(replace(&store, &first.id).await.unwrap().id, successor.id);
            assert_eq!(
                ensure(&store, repo.path(), Some("infrastructure"))
                    .await
                    .unwrap()
                    .id,
                successor.id
            );
        });
    }

    #[test]
    fn a_repository_without_waves_has_its_own_conversation() {
        let _lock = crate::journal::test_env_lock();
        let home = SessionHome::new();
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let store = home.store().await;
            let repo = loopflow_test_support::TestRepo::new();

            let first = ensure(&store, repo.path(), None).await.unwrap();
            let second = ensure(&store, repo.path(), None).await.unwrap();

            assert_eq!(first.id, second.id);
            let session = store.session(&first.id).await.unwrap().unwrap();
            assert_eq!(session.skill.as_deref(), Some("repo-session"));
            assert_eq!((session.wave_id, session.task_id), (None, None));

            // A Wave's conversation in the same repository is a separate scope.
            let ambient_wave = wave(&store, &repo).await;
            std::env::set_var("LF_WAVE_ID", ambient_wave.id().as_str());
            assert_eq!(
                ensure(&store, repo.path(), None).await.unwrap().id,
                first.id
            );

            // Explicit selection also ignores stale ambient Wave context.
            std::env::set_var("LF_WAVE_ID", "unknown-wave");
            let wave = ensure(&store, repo.path(), Some("infrastructure"))
                .await
                .unwrap();
            assert_ne!(wave.id, first.id);

            let successor = replace(&store, &first.id).await.unwrap();
            assert_ne!(successor.id, first.id);
            assert_eq!(
                ensure(&store, repo.path(), None).await.unwrap().id,
                successor.id
            );
        });
    }

    #[test]
    fn scope_workspaces_survive_replacement_without_task_membership() {
        let _lock = crate::journal::test_env_lock();
        let home = SessionHome::new();
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let store = home.store().await;
            let repo = loopflow_test_support::TestRepo::new();
            wave(&store, &repo).await;
            let mut workspaces = Vec::new();
            for scope in [None, Some("infrastructure")] {
                let first = ensure(&store, repo.path(), scope).await.unwrap();
                let session = store.session(&first.id).await.unwrap().unwrap();
                assert_ne!(session.cwd, repo.path());
                std::fs::create_dir_all(session.cwd.join("scratch")).unwrap();
                std::fs::write(session.cwd.join("scratch/plan.md"), "private plan").unwrap();
                std::fs::write(session.cwd.join("memory.md"), "durable decision").unwrap();
                let successor = replace(&store, &first.id).await.unwrap();
                let session = store.session(&successor.id).await.unwrap().unwrap();
                assert_eq!(
                    std::fs::read_to_string(session.cwd.join("scratch/plan.md")).unwrap(),
                    "private plan"
                );
                assert_eq!(
                    std::fs::read_to_string(session.cwd.join("memory.md")).unwrap(),
                    "durable decision"
                );
                let mut records = vec![successor];
                crate::ops::human_session::workspace::associate(&store, &mut records)
                    .await
                    .unwrap();
                let workspace = records[0].workspace.as_ref().unwrap();
                assert_eq!(workspace.task_id, None);
                assert!(records[0].task_ids.is_empty());
                workspaces.push(session.cwd);
            }
            assert_ne!(workspaces[0], workspaces[1]);
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
                    None,
                    super::repository_session(&canonical),
                )
                .await
                .unwrap();
            let first = ensure(&store, repo.path(), None).await.unwrap();
            let admitted = store.session(&first.id).await.unwrap().unwrap();
            assert_eq!(admitted.id, previous.id);
            assert_eq!(admitted.title, previous.title);
            assert_eq!(admitted.artifact_key, previous.artifact_key);
            assert_ne!(admitted.cwd, previous.cwd);
        });
    }

    #[test]
    fn replace_refuses_an_ordinary_conversation() {
        let _lock = crate::journal::test_env_lock();
        let home = SessionHome::new();
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let store = home.store().await;
            let error = replace(&store, "not-a-primary").await.unwrap_err();
            assert!(error.to_string().contains("not a primary Session"));
        });
    }
}
