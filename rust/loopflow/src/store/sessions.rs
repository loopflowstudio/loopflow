use crate::session::{LfSession, PrimaryScope, TitleSource};
use crate::store::session_projection;
use crate::store::sqlite::SqliteStore;
use loopflow_model::durable::TaskId;
use loopflow_model::id::LfProcessId;

use super::{run_sqlite, Store, StoreResult};

impl Store {
    pub(crate) async fn session_summaries(
        &self,
        filter: &crate::session::SessionFilter,
        now: i64,
    ) -> StoreResult<Vec<crate::session::SessionSummary>> {
        let filter = filter.clone();
        run_sqlite(&self.sqlite, move |store| {
            session_projection::session_summaries(&store, &filter, now)
        })
        .await
    }

    pub async fn session(&self, id: &str) -> StoreResult<Option<LfSession>> {
        let id = id.to_string();
        run_sqlite(&self.sqlite, move |store| store.session(&id)).await
    }

    pub async fn session_for_artifact(&self, artifact_key: &str) -> StoreResult<Option<LfSession>> {
        let artifact_key = artifact_key.to_owned();
        run_sqlite(&self.sqlite, move |store| {
            store.session_for_artifact(&artifact_key)
        })
        .await
    }

    pub async fn create_session(&self, session: LfSession) -> StoreResult<LfSession> {
        let caller = crate::journal::current_lf_process_id();
        run_sqlite(&self.sqlite, move |store| {
            create_session(&store, session, caller.as_ref())
        })
        .await
    }

    pub(crate) async fn task_conversations(
        &self,
        task: &loopflow_model::durable::TaskId,
    ) -> StoreResult<Vec<(LfSession, Option<i64>)>> {
        let task = task.clone();
        run_sqlite(&self.sqlite, move |store| store.task_conversations(&task)).await
    }

    pub(crate) async fn task_primary(
        &self,
        task: &loopflow_model::durable::TaskId,
    ) -> StoreResult<Option<LfSession>> {
        let task = task.clone();
        run_sqlite(&self.sqlite, move |store| store.task_primary(&task)).await
    }

    pub(crate) async fn choose_task_primary(
        &self,
        task: &loopflow_model::durable::TaskId,
        session: &str,
    ) -> StoreResult<LfSession> {
        let (task, session) = (task.clone(), session.to_string());
        run_sqlite(&self.sqlite, move |store| {
            store.choose_task_primary(&task, &session)
        })
        .await
    }

    pub async fn ensure_primary_session(
        &self,
        scope: &crate::session::PrimaryScope,
        session: LfSession,
    ) -> StoreResult<LfSession> {
        let scope = scope.clone();
        let caller = crate::journal::current_lf_process_id();
        run_sqlite(&self.sqlite, move |store| {
            ensure_primary_session(&store, &scope, session, caller.as_ref())
        })
        .await
    }

    pub async fn replace_session_input(
        &self,
        expected_capture: Option<i64>,
        session: LfSession,
        creation: Option<String>,
    ) -> StoreResult<LfSession> {
        run_sqlite(&self.sqlite, move |store| {
            replace_session_input(&store, expected_capture, session, creation.as_deref())
        })
        .await
    }

    pub async fn bind_session(
        &self,
        id: &str,
        expected_capture: Option<i64>,
        task: &TaskId,
    ) -> StoreResult<LfSession> {
        let id = id.to_string();
        let task = task.clone();
        run_sqlite(&self.sqlite, move |store| {
            bind_session(&store, &id, expected_capture, &task)
        })
        .await
    }

    pub async fn sessions(
        &self,
        filter: &crate::session::SessionFilter,
    ) -> StoreResult<Vec<LfSession>> {
        let filter = filter.clone();
        run_sqlite(&self.sqlite, move |store| {
            session_projection::sessions(&store, &filter)
        })
        .await
    }

    pub async fn session_inputs(&self, id: &str) -> StoreResult<Vec<String>> {
        let id = id.to_string();
        run_sqlite(&self.sqlite, move |store| store.session_inputs(&id)).await
    }

    pub async fn rename_session(
        &self,
        id: &str,
        title: &str,
        source: TitleSource,
    ) -> StoreResult<()> {
        let id = id.to_string();
        let title = title.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.rename_session(&id, &title, source)
        })
        .await
    }
}

impl Store {
    pub async fn graph_inventory(
        &self,
        filter: &crate::durable::GraphProcessFilter,
        after: Option<&str>,
        limit: std::num::NonZeroU32,
    ) -> StoreResult<crate::durable::GraphProcessPage> {
        let filter = filter.clone();
        let after = after.map(str::to_owned);
        run_sqlite(&self.sqlite, move |store| {
            store.graph_inventory(&filter, after.as_deref(), limit)
        })
        .await
    }

    /// One Graph by Graph process id or unique prefix, drawn from its Processes.
    pub async fn graph_detail(
        &self,
        selector: &str,
    ) -> StoreResult<Option<crate::durable::GraphProcessDetail>> {
        let selector = selector.to_string();
        run_sqlite(&self.sqlite, move |store| {
            Ok(store
                .graph_process(&selector)?
                .map(|(graph, entry)| graph.detail(entry)))
        })
        .await
    }
}

/// Prepare repository identity before admitting a Session to synchronous storage.
pub fn create_session(
    store: &SqliteStore,
    mut session: LfSession,
    caller: Option<&LfProcessId>,
) -> StoreResult<LfSession> {
    discover_session_repo(&mut session)?;
    let _admission = super::admission::lock_session_checkouts(store, &session)?;
    store.create_session_prepared(session, caller)
}

pub fn ensure_primary_session(
    store: &SqliteStore,
    scope: &PrimaryScope,
    mut session: LfSession,
    caller: Option<&LfProcessId>,
) -> StoreResult<LfSession> {
    if let Some(current) = store.primary_session(scope)? {
        return Ok(current);
    }
    discover_session_repo(&mut session)?;
    store.ensure_primary_session(scope, session, caller)
}

/// Name the repository holding the Session's cwd. This runs Git, so callers
/// do it before taking the write lock every other writer waits on.
fn discover_session_repo(session: &mut LfSession) -> StoreResult<()> {
    if session.repo.is_none() && session.cwd.is_dir() {
        session.repo = crate::repository::CanonicalRepo::containing(&session.cwd)
            .map_err(|error| super::StoreError::InvalidData(error.to_string()))?
            .map(|repo| repo.to_string());
    }
    Ok(())
}

/// Capture caller provenance before entering synchronous storage.
pub fn replace_session_input(
    store: &SqliteStore,
    expected: Option<i64>,
    session: LfSession,
    creation: Option<&str>,
) -> StoreResult<LfSession> {
    let caller = crate::journal::current_lf_process_id();
    let saved = store
        .session(&session.id)?
        .ok_or(super::StoreError::NotFound)?;
    let _admission = super::admission::lock_session_checkouts(store, &saved)?;
    store.replace_session_input_prepared(expected, session, creation, caller.as_ref())
}

pub fn bind_session(
    store: &SqliteStore,
    id: &str,
    capture: Option<i64>,
    task: &TaskId,
) -> StoreResult<LfSession> {
    let saved = store.session(id)?.ok_or(super::StoreError::NotFound)?;
    let _admission = super::admission::lock_task_checkouts(store, &[&saved.cwd], Some(task))?;
    store.bind_session_prepared(id, capture, task, &saved.cwd)
}

#[cfg(test)]
mod tests {
    use crate::repository::CanonicalRepo;
    use crate::session::{LfSession, PrimaryScope, TitleSource};
    use crate::store::sqlite::SqliteStore;

    fn candidate(cwd: &std::path::Path) -> LfSession {
        LfSession {
            captured: None,
            id: format!("session_{}", uuid::Uuid::new_v4().simple()).into(),
            artifact_key: None,
            caller_artifact_key: None,
            input_published: true,
            cwd: cwd.into(),
            skill: None,
            provider: None,
            model: None,
            node: None,
            iterations: None,
            task_id: None,
            wave_id: None,
            graph_lf_process_id: None,
            work_source: None,
            bound_at: None,
            interactive: true,
            repo: None,
            title: "Conversation".into(),
            title_source: TitleSource::Human,
            request: None,
            ready_summary: None,
            completed_at: None,
            created_at: 1,
        }
    }

    #[test]
    fn session_admission_discovers_the_repository_from_a_nested_directory() {
        let directory = tempfile::tempdir().unwrap();
        let repo = directory.path().join("repository");
        let output = std::process::Command::new("git")
            .args(["init", "--quiet"])
            .arg(&repo)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let cwd = repo.join("src/nested");
        std::fs::create_dir_all(&cwd).unwrap();
        let store = SqliteStore::open_ephemeral(&directory.path().join("registry.db")).unwrap();
        let session = super::create_session(&store, candidate(&cwd), None).unwrap();
        assert_eq!(
            session.repo,
            Some(CanonicalRepo::discover(&repo).unwrap().to_string())
        );
        assert_eq!(session.cwd, cwd);
        assert_eq!(store.session(&session.id).unwrap(), Some(session));
    }

    #[test]
    fn repository_primary_resolves_after_its_checkout_is_deleted() {
        let directory = tempfile::tempdir().unwrap();
        let checkout = directory.path().join("repository");
        std::fs::create_dir(&checkout).unwrap();
        let repo = CanonicalRepo::discover(&checkout).unwrap();
        let scope = PrimaryScope::Repository(repo.clone());
        let store = SqliteStore::open_ephemeral(&directory.path().join("registry.db")).unwrap();
        let session = super::ensure_primary_session(
            &store,
            &scope,
            LfSession {
                repo: Some(repo.to_string()),
                ..candidate(&checkout)
            },
            None,
        )
        .unwrap();
        std::fs::remove_dir(&checkout).unwrap();
        assert_eq!(
            store.primary_scope(&session.id).unwrap(),
            Some(scope.clone())
        );
        assert_eq!(
            super::ensure_primary_session(&store, &scope, candidate(&checkout), None).unwrap(),
            session
        );
    }
}
