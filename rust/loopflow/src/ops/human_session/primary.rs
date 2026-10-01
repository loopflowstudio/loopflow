//! The one ongoing conversation of a scope. A primary Session is an ordinary
//! interactive conversation; its row names the scope it is primary for.

use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};

use super::{
    ask_background_name, ask_exec_is_running, capture_is_prepared, lock_session_exec,
    publish_prepared_input, session_not_found, start_durable_session, surface, NativeSession,
    SessionRecord,
};
use crate::session::{AgentSession, PrimaryScope, SessionKind, TitleSource, WorkSource};
use crate::store::SharedStore;

/// Find or admit the Wave's primary conversation and start its terminal once.
pub(crate) async fn ensure_wave(
    store: &SharedStore,
    repo: &Path,
    wave: &str,
) -> Result<SessionRecord> {
    let binding = crate::ops::resolve_work_binding(store, repo, &format!("wave:{wave}")).await?;
    let scope = PrimaryScope::Wave(binding.wave_id.clone());
    let _lock = lock_scope(&scope).await?;
    let session = store
        .ensure_primary_session(&scope, None, wave_session(&binding))
        .await?;
    start(store, session).await
}

/// Find or admit the repository's own conversation. It needs no Wave, Task
/// or planning provider.
pub(crate) async fn ensure_repository(store: &SharedStore, repo: &Path) -> Result<SessionRecord> {
    let repo = crate::repository::CanonicalRepo::discover(repo)?;
    let scope = PrimaryScope::Repository(repo.clone());
    let _lock = lock_scope(&scope).await?;
    let session = store
        .ensure_primary_session(&scope, None, repository_session(&repo))
        .await?;
    start(store, session).await
}

/// Give the scope a fresh conversation. The predecessor's provider stops
/// first; if it cannot be stopped, the predecessor stays primary.
pub(crate) async fn replace(store: &SharedStore, id: &str) -> Result<SessionRecord> {
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
        let _launch = tokio::task::spawn_blocking(move || lock_session_exec(&lock_id)).await??;
        stop_client(&previous)
            .with_context(|| format!("stop primary Session {id}; it remains primary"))?;
    }
    let successor = match &scope {
        PrimaryScope::Repository(repo) => repository_session(repo),
        PrimaryScope::Wave(wave) => wave_session(
            &crate::ops::resolve_work_binding(store, &previous.cwd, &format!("wave:{wave}"))
                .await?,
        ),
    };
    let session = store
        .ensure_primary_session(&scope, Some(id), successor)
        .await?;
    start(store, session).await
}

fn wave_session(binding: &crate::ops::WorkBinding) -> AgentSession {
    AgentSession {
        wave_id: Some(binding.wave_id.clone()),
        work_source: Some(WorkSource::Declared),
        ..conversation(
            &binding.cwd,
            binding.agent.as_deref(),
            "wave/session",
            binding.wave_name.clone(),
        )
    }
}

fn repository_session(repo: &crate::repository::CanonicalRepo) -> AgentSession {
    let title = repo
        .as_path()
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| repo.to_string());
    AgentSession {
        repo: Some(repo.to_string()),
        ..conversation(repo.as_path(), None, "repo/session", title)
    }
}

fn conversation(cwd: &Path, agent: Option<&str>, skill: &str, title: String) -> AgentSession {
    let agent = crate::ops::task::resolve_task_agent(cwd, agent, None);
    let (provider, model) = crate::engine::config::parse_agent(&agent);
    AgentSession {
        captured: None,
        id: uuid::Uuid::new_v4().simple().to_string(),
        artifact_key: crate::session_record::new_artifact_key(),
        caller_artifact_key: None,
        input_published: false,
        cwd: cwd.to_path_buf(),
        skill: Some(skill.into()),
        provider: Some(provider),
        model,
        node: None,
        iterations: None,
        task_id: None,
        wave_id: None,
        flow_session_id: None,
        work_source: None,
        bound_at: None,
        kind: SessionKind::Conversation,
        interactive: true,
        repo: None,
        title,
        title_source: TitleSource::Generated,
        request: None,
        ready_summary: None,
        completed_at: None,
        created_at: crate::store::rows::now_unix(),
    }
}

/// Publish the admitted input and launch it unless a launcher already holds
/// it. A failed start keeps the prepared input for the next caller.
async fn start(store: &SharedStore, session: AgentSession) -> Result<SessionRecord> {
    let session = if session.input_published {
        session
    } else {
        publish_prepared_input(
            store,
            &session,
            crate::session_record::SessionFlowMembership::Independent,
        )?
    };
    if capture_is_prepared(&session.artifact_key)? && !ask_exec_is_running(&session.id).await? {
        let lf = crate::engine::process::resolve_pinned_lf_binary()?;
        let argv = vec![
            lf.to_string_lossy().to_string(),
            "session".to_string(),
            "serve-ask".to_string(),
            session.artifact_key.to_string(),
        ];
        start_durable_session(&ask_background_name(&session.id), &session.cwd, &argv, &[])
            .await
            .with_context(|| {
                format!(
                    "launch primary Session {}; run the same command to retry",
                    session.id
                )
            })?;
    }
    surface(store, &session).await
}

fn stop_client(session: &AgentSession) -> Result<()> {
    #[cfg(test)]
    if super::action_test::stop(&session.artifact_key) {
        return Ok(());
    }
    let native = NativeSession::of(session)?;
    if !native.dir.is_dir() {
        return Ok(());
    }
    crate::lf::commands::util::stop_provider_session(&native.dir, native.provider)
}

/// Admission and replacement of one scope are serial across processes.
async fn lock_scope(scope: &PrimaryScope) -> Result<std::fs::File> {
    let key = match scope {
        PrimaryScope::Repository(repo) => format!("primary:repository:{repo}"),
        PrimaryScope::Wave(wave) => format!("primary:wave:{wave}"),
    };
    match tokio::task::spawn_blocking(move || lock_session_exec(&key)).await {
        Ok(lock) => lock,
        Err(error) => bail!("lock primary Session scope: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{ensure_repository, ensure_wave, replace};
    use crate::ops::human_session::tests::{AskHome, ASK_LAUNCHERS, FAILED_ASK_LAUNCHERS};
    use crate::ops::human_session::{action_test::NativeClients, ask_background_name};
    use crate::session::SessionKind;

    async fn wave(
        store: &crate::store::SharedStore,
        repo: &loopflow_test_support::TestRepo,
    ) -> crate::work::wave::Wave {
        crate::work::wave::ensure_wave_row(store, repo.path(), "infrastructure")
            .await
            .unwrap()
    }

    #[test]
    fn ensure_finds_the_same_wave_conversation_and_launches_once() {
        let _lock = crate::journal::test_env_lock();
        let home = AskHome::new();
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let store = home.store().await;
            let repo = loopflow_test_support::TestRepo::new();
            let wave = wave(&store, &repo).await;

            let first = ensure_wave(&store, repo.path(), "infrastructure")
                .await
                .unwrap();
            // The simulated launcher refuses a duplicate start, so a second
            // launch attempt would fail this call.
            let second = ensure_wave(&store, repo.path(), "infrastructure")
                .await
                .unwrap();

            assert_eq!(first.id, second.id);
            assert!(ASK_LAUNCHERS
                .lock()
                .unwrap()
                .contains(&ask_background_name(&first.id)));
            let session = store.session(&first.id).await.unwrap().unwrap();
            assert_eq!(session.kind, SessionKind::Conversation);
            assert!(session.interactive && session.input_published);
            assert_eq!(session.skill.as_deref(), Some("wave/session"));
            assert_eq!(session.wave_id.as_ref(), Some(wave.id()));
            assert_eq!(session.task_id, None);
        });
    }

    #[test]
    fn failed_start_keeps_the_session_for_the_next_ensure() {
        let _lock = crate::journal::test_env_lock();
        let home = AskHome::new();
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let store = home.store().await;
            let repo = loopflow_test_support::TestRepo::new();
            wave(&store, &repo).await;

            // The launcher name derives from an id admitted inside ensure; fail
            // every launcher by admitting first, then failing that name.
            let admitted = ensure_wave(&store, repo.path(), "infrastructure")
                .await
                .unwrap();
            let name = ask_background_name(&admitted.id);
            ASK_LAUNCHERS.lock().unwrap().remove(&name);
            FAILED_ASK_LAUNCHERS.lock().unwrap().insert(name.clone());
            assert!(ensure_wave(&store, repo.path(), "infrastructure")
                .await
                .is_err());

            FAILED_ASK_LAUNCHERS.lock().unwrap().clear();
            let retried = ensure_wave(&store, repo.path(), "infrastructure")
                .await
                .unwrap();
            assert_eq!(retried.id, admitted.id);
            assert!(ASK_LAUNCHERS.lock().unwrap().contains(&name));
        });
    }

    #[test]
    fn replace_stops_the_predecessor_and_repeats_to_the_same_successor() {
        let _lock = crate::journal::test_env_lock();
        let home = AskHome::new();
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let store = home.store().await;
            let repo = loopflow_test_support::TestRepo::new();
            wave(&store, &repo).await;
            let first = ensure_wave(&store, repo.path(), "infrastructure")
                .await
                .unwrap();
            let previous = store.session(&first.id).await.unwrap().unwrap();
            let clients =
                NativeClients::new(&first.id, std::slice::from_ref(&previous.artifact_key));

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
                ensure_wave(&store, repo.path(), "infrastructure")
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
        let home = AskHome::new();
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let store = home.store().await;
            let repo = loopflow_test_support::TestRepo::new();

            let first = ensure_repository(&store, repo.path()).await.unwrap();
            let second = ensure_repository(&store, repo.path()).await.unwrap();

            assert_eq!(first.id, second.id);
            let session = store.session(&first.id).await.unwrap().unwrap();
            assert_eq!(session.skill.as_deref(), Some("repo/session"));
            assert_eq!((session.wave_id, session.task_id), (None, None));

            // A Wave's conversation in the same repository is a separate scope.
            wave(&store, &repo).await;
            let wave = ensure_wave(&store, repo.path(), "infrastructure")
                .await
                .unwrap();
            assert_ne!(wave.id, first.id);

            let successor = replace(&store, &first.id).await.unwrap();
            assert_ne!(successor.id, first.id);
            assert_eq!(
                ensure_repository(&store, repo.path()).await.unwrap().id,
                successor.id
            );
        });
    }

    #[test]
    fn replace_refuses_an_ordinary_conversation() {
        let _lock = crate::journal::test_env_lock();
        let home = AskHome::new();
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let store = home.store().await;
            let error = replace(&store, "not-a-primary").await.unwrap_err();
            assert!(error.to_string().contains("not a primary Session"));
        });
    }
}
