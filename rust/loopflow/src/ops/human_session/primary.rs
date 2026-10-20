//! The one ongoing conversation of a scope. A primary Session is an ordinary
//! interactive conversation; its row names the scope it is primary for.

use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};

use super::{
    lock_session_process, publish_prepared_input, session_not_found, surface, SessionRecord,
};
use crate::session::{LfSession, PrimaryScope, TitleSource, WorkSource};
use crate::store::SharedStore;

/// Find or admit the primary conversation of a Wave, or of the repository
/// itself, and prepare its first input. The repository's needs no Wave, Task
/// or planning provider.
pub(crate) async fn ensure(
    store: &SharedStore,
    repo: &Path,
    wave: Option<&str>,
    fresh: bool,
) -> Result<SessionRecord> {
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
    let previous = store.sqlite.primary_session(&scope)?;
    if let Some(previous) = previous.as_ref().filter(|_| !fresh) {
        return prepare(store, &previous.id).await;
    }
    let mut session = match &binding {
        Some(binding) => wave_session(binding)?,
        None => repository_session(&crate::repository::CanonicalRepo::discover(repo)?)?,
    };
    session.cwd = ensure_scope_worktree(&session.cwd, &scope)?.path;
    let replacing = previous.as_ref().map(|session| session.id.as_str());
    let session = store
        .ensure_primary_session(&scope, replacing, session)
        .await?;
    prepare(store, &session.id).await
}

/// A Task's primary is one of its own conversations: the one already chosen,
/// else its sole unfinished interactive one, else the most recently used.
/// Only a Task with none gets a new one, in its checkout. The others stay open.
pub(crate) async fn ensure_task(
    store: &SharedStore,
    repo: &Path,
    task: &str,
    fresh: bool,
) -> Result<SessionRecord> {
    let binding = crate::ops::resolve_work_binding(store, repo, &format!("task:{task}")).await?;
    let crate::durable::WorkRef::Task(id) = &binding.work else {
        unreachable!("a task selector binds a Task");
    };
    let scope = PrimaryScope::Task(id.clone());
    let _lock = lock_scope(&scope).await?;
    let chosen = if fresh {
        None
    } else {
        task_conversation(store, id).await?
    };
    if !binding.cwd.try_exists()? {
        // Reuse Task preparation's recorded placement and restoration checks.
        // Opening a conversation does not invent a checkout or start a Flow.
        let repo = repo.to_path_buf();
        let task = id.to_string();
        tokio::task::spawn_blocking(move || {
            crate::ops::task::task_checkout(&repo, &task, Default::default())
        })
        .await??;
    }
    if let Some(session) = chosen {
        let primary = store.choose_task_primary(id, &session.id).await?;
        return prepare(store, &primary.id).await;
    }
    anyhow::ensure!(
        binding.cwd.is_dir(),
        "Task {task} has no checkout to hold a conversation"
    );
    let previous = if fresh {
        store.sqlite.primary_session(&scope)?
    } else {
        None
    };
    let session = store
        .ensure_primary_session(
            &scope,
            previous.as_ref().map(|session| session.id.as_str()),
            task_session(store, &binding, id)?,
        )
        .await?;
    prepare(store, &session.id).await
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
pub(super) async fn admit_workspace(
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
        // A Task's primary stays in the Task's checkout.
        PrimaryScope::Task(_) => return Ok(session),
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
        wave_id: binding.wave_id.clone(),
        work_source: Some(WorkSource::Declared),
        ..conversation(
            &binding.cwd,
            binding.agent.as_deref(),
            "task/session",
            title,
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

fn conversation(cwd: &Path, agent: Option<&str>, skill: &str, title: String) -> Result<LfSession> {
    let skill = crate::flow::load_skill(skill, cwd)?.name;
    let agent = crate::ops::task::resolve_task_agent(cwd, agent, None);
    let (provider, model) = crate::config::parse_agent(&agent);
    Ok(LfSession {
        captured: None,
        id: uuid::Uuid::new_v4().simple().to_string().into(),
        artifact_key: crate::session_record::new_artifact_key(),
        caller_artifact_key: None,
        input_published: false,
        cwd: cwd.to_path_buf(),
        skill: Some(skill),
        provider: Some(provider),
        model,
        node: None,
        iterations: None,
        task_id: None,
        wave_id: None,
        flow_lf_process_id: None,
        work_source: None,
        bound_at: None,
        interactive: true,
        repo: None,
        title,
        title_source: TitleSource::Generated,
        request: None,
        ready_summary: None,
        completed_at: None,
        created_at: crate::store::rows::now_unix(),
    })
}

/// Prepare the first input; connecting owns the foreground provider launch.
async fn prepare(store: &SharedStore, id: &str) -> Result<SessionRecord> {
    let lock_id = id.to_string();
    let _launch = tokio::task::spawn_blocking(move || lock_session_process(&lock_id)).await??;
    let session = store
        .session(id)
        .await?
        .ok_or_else(|| session_not_found(id))?;
    let session = admit_workspace(store, session).await?;
    let session = if session.input_published {
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
    use crate::ops::human_session::capture_is_prepared;
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

            let first = ensure(&store, repo.path(), Some("infrastructure"), false)
                .await
                .unwrap();
            let second = ensure(&store, repo.path(), Some("infrastructure"), false)
                .await
                .unwrap();

            assert_eq!(first.id, second.id);
            assert!(store
                .sqlite
                .session_attachment(&first.id)
                .unwrap()
                .is_none());
            let session = store.session(&first.id).await.unwrap().unwrap();
            assert!(session.interactive && session.input_published);
            assert!(capture_is_prepared(&session.artifact_key).unwrap());
            assert_eq!(session.skill.as_deref(), Some("wave-session"));
            assert_eq!(session.wave_id.as_ref(), Some(wave.id()));
            assert_eq!(session.task_id, None);
        });
    }

    #[test]
    fn fresh_admits_a_successor_and_connect_recovers_it() {
        let _lock = crate::journal::test_env_lock();
        let home = SessionHome::new();
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let store = home.store().await;
            let repo = loopflow_test_support::TestRepo::new();
            wave(&store, &repo).await;
            let first = ensure(&store, repo.path(), Some("infrastructure"), false)
                .await
                .unwrap();

            let successor = ensure(&store, repo.path(), Some("infrastructure"), true)
                .await
                .unwrap();

            assert_ne!(successor.id, first.id);
            assert!(store
                .session(&first.id)
                .await
                .unwrap()
                .unwrap()
                .completed_at
                .is_some());

            assert_eq!(
                ensure(&store, repo.path(), Some("infrastructure"), false)
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

            let first = ensure(&store, repo.path(), None, false).await.unwrap();
            let second = ensure(&store, repo.path(), None, false).await.unwrap();

            assert_eq!(first.id, second.id);
            let session = store.session(&first.id).await.unwrap().unwrap();
            assert_eq!(session.skill.as_deref(), Some("repo-session"));
            assert_eq!((session.wave_id, session.task_id), (None, None));

            // A Wave's conversation in the same repository is a separate scope.
            let ambient_wave = wave(&store, &repo).await;
            std::env::set_var("LF_WAVE_ID", ambient_wave.id().as_str());
            assert_eq!(
                ensure(&store, repo.path(), None, false).await.unwrap().id,
                first.id
            );

            // Explicit selection also ignores stale ambient Wave context.
            std::env::set_var("LF_WAVE_ID", "unknown-wave");
            let wave = ensure(&store, repo.path(), Some("infrastructure"), false)
                .await
                .unwrap();
            assert_ne!(wave.id, first.id);

            let successor = ensure(&store, repo.path(), None, true).await.unwrap();
            assert_ne!(successor.id, first.id);
            assert_eq!(
                ensure(&store, repo.path(), None, false).await.unwrap().id,
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
                let first = ensure(&store, repo.path(), scope, false).await.unwrap();
                let session = store.session(&first.id).await.unwrap().unwrap();
                assert_ne!(session.cwd, repo.path());
                std::fs::create_dir_all(session.cwd.join("scratch")).unwrap();
                std::fs::write(session.cwd.join("scratch/plan.md"), "private plan").unwrap();
                std::fs::write(session.cwd.join("memory.md"), "durable decision").unwrap();
                let successor = ensure(&store, repo.path(), scope, true).await.unwrap();
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
                    super::repository_session(&canonical).unwrap(),
                )
                .await
                .unwrap();
            let first = ensure(&store, repo.path(), None, false).await.unwrap();
            let admitted = store.session(&first.id).await.unwrap().unwrap();
            assert_eq!(admitted.id, previous.id);
            assert_eq!(admitted.title, previous.title);
            assert_eq!(admitted.artifact_key, previous.artifact_key);
            assert_ne!(admitted.cwd, previous.cwd);
        });
    }
}
