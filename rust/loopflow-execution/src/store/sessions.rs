use crate::session::{LfSession, PrimaryScope, TitleSource};
use crate::store::session_projection;
use loopflow_model::durable::TaskId;
use loopflow_model::id::LfProcessId;
use loopflow_storage::session_record::{SessionHistory, SessionOrigin};
use loopflow_storage::sqlite::sessions::{HistorySelection, PendingHistory, PendingInput};
use loopflow_storage::sqlite::{SessionCaller, SqliteStore};

use super::{run_sqlite, Store, StoreError, StoreResult};

impl Store {
    pub async fn session_summaries(
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

    pub async fn task_primary(
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
        let caller = session_caller()?;
        run_sqlite(&self.sqlite, move |store| {
            store.rename_session_prepared(&id, &title, source, &caller)
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
            store.graph_inventory(
                &filter,
                after.as_deref(),
                limit,
                crate::process::os_process::observed_result,
            )
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

pub(crate) fn session_caller() -> StoreResult<SessionCaller> {
    let capture_key = std::env::var_os(crate::session_record::CAPTURE_KEY_ENV)
        .map(|key| {
            key.into_string().map_err(|_| {
                super::StoreError::InvalidData("capture key is not valid UTF-8".into())
            })
        })
        .transpose()?;
    Ok(SessionCaller {
        agent: crate::journal::agent_caller(),
        capture_key,
    })
}

pub fn rename_session(
    store: &SqliteStore,
    id: &str,
    title: &str,
    source: TitleSource,
) -> StoreResult<()> {
    store.rename_session_prepared(id, title, source, &session_caller()?)
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
pub(crate) fn discover_session_repo(session: &mut LfSession) -> StoreResult<()> {
    if session.repo.is_none() && session.cwd.is_dir() {
        session.repo = crate::repository::containing(&session.cwd)
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
    let caller = session_caller()?;
    store.bind_session_prepared(id, capture, task, &saved.cwd, &caller)
}

/// Project retained input history, with its own attribution and chronology.
/// A continuation never moves earlier usage to the conversation's new binding.
pub fn conversation_history(
    store: &SqliteStore,
    wave: Option<&str>,
    project: Option<&str>,
    task: Option<&str>,
    caller: Option<&str>,
    since: i64,
    include_finished: bool,
) -> StoreResult<Vec<SessionHistory>> {
    conversation_inputs(
        store,
        wave,
        project,
        task,
        caller,
        (since, include_finished, None),
        HistorySelection::All,
    )
    .map(|(rows, _)| rows)
}

/// Select the recent presentation budget before history decoding. Every
/// unterminated input is retained; explicit drills use conversation_history.
pub fn recent_conversation_history(
    store: &SqliteStore,
    wave: Option<&str>,
    project: Option<&str>,
    task: Option<&str>,
    since: i64,
    limit: usize,
) -> StoreResult<(Vec<SessionHistory>, bool)> {
    conversation_inputs(
        store,
        wave,
        project,
        task,
        None,
        (since, false, Some(limit)),
        HistorySelection::All,
    )
}

pub fn input_history(store: &SqliteStore, selector: &str) -> StoreResult<SessionHistory> {
    // A copied input's key resolves only among copies, which select it whole.
    let selected = match store.resolve_history_input(selector) {
        Err(StoreError::NotFound) => selector.to_owned(),
        selected => selected?,
    };
    let input = store
        .session(&selected)?
        .and_then(|session| session.artifact_key)
        .unwrap_or(selected);
    conversation_inputs(
        store,
        None,
        None,
        None,
        None,
        (0, false, None),
        HistorySelection::Input(&input),
    )?
    .0
    .pop()
    .ok_or(StoreError::NotFound)
}

fn conversation_inputs(
    store: &SqliteStore,
    wave: Option<&str>,
    project: Option<&str>,
    task: Option<&str>,
    caller: Option<&str>,
    window: (i64, bool, Option<usize>),
    selection: HistorySelection<'_>,
) -> StoreResult<(Vec<SessionHistory>, bool)> {
    let pending = store.pending_inputs(wave, project, task, caller, window, selection)?;
    let mut histories = pending
        .inputs
        .iter()
        .map(|input| project_history_input(store, &pending, input))
        .collect::<StoreResult<Vec<_>>>()?;
    if histories.iter().any(|history| history.origin.is_some()) {
        let labels = super::machines::origin_labels(store)?;
        for history in &mut histories {
            let Some(origin) = history.origin.as_mut() else {
                continue;
            };
            origin.label = labels.get(&origin.machine_id).cloned();
            // The origin's `repo` is a path there; Work kept here names this
            // Machine's checkout so reports group it with local runs. An
            // input's turns can carry Work the Session joined later.
            let work = std::iter::once((history.task_id.as_ref(), history.wave_id.as_ref()))
                .chain(
                    history
                        .providers
                        .iter()
                        .map(|turn| (turn.task_id.as_ref(), turn.wave_id.as_ref())),
                )
                .filter(|(task, wave)| task.is_some() || wave.is_some());
            for (task, wave) in work {
                if let Some(repo) = store.work_repository_path(task, wave)? {
                    history.repo = Some(repo);
                    break;
                }
            }
        }
    }
    Ok((histories, pending.truncated))
}

pub fn project_history_input(
    store: &SqliteStore,
    pending: &PendingHistory,
    input: &PendingInput,
) -> StoreResult<SessionHistory> {
    let saved = store.read_history_input(pending, input)?;
    let origin = input.origin().cloned().map(|machine_id| SessionOrigin {
        machine_id,
        label: None,
    });
    crate::session_record::project_input_history(
        &saved.session,
        saved.artifact_key.as_deref(),
        &saved.events,
        &saved.turns,
        saved.names,
        origin,
    )
    .map_err(|error| super::StoreError::InvalidData(error.to_string()))
}

#[cfg(test)]
mod tests {
    use crate::session::{LfSession, PrimaryScope, TitleSource};

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
            completed_at: None,
            created_at: 1,
        }
    }

    #[test]
    fn malformed_prepared_request_keeps_history_gaps_and_current_fallbacks() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("store.db")).unwrap();
        let input = crate::session_record::new_artifact_key();
        let mut session = candidate(home.path());
        session.artifact_key = Some(input.clone());
        session.provider = Some("claude".into());
        session.model = Some("current-model".into());
        session.skill = Some("current-skill".into());
        let session = super::create_session(&store, session, None).unwrap();
        let mut manifest = serde_json::json!({
            "schema_version":1, "artifact_key":input,
            "created_at":"1970-01-01T00:00:01Z", "harness":"codex",
            "model":"saved-model", "surface":"headless", "cwd":"/saved",
            "skill":"saved-skill", "subjects":[], "host":"fixture",
            "exec":{"system_prompt":42}
        });
        assert!(
            serde_json::from_value::<crate::session_record::SessionCaptureManifest>(
                manifest.clone()
            )
            .is_err()
        );
        let retain = |manifest| {
            store.retain_session_observation(&session, &crate::session::SessionObservation {
            artifact_key: input.clone(), source:"manifest.json".into(), observed_at:1,
            task_id:None, wave_id:None,
            payload:serde_json::json!({"input_id":input,"source":"manifest.json","evidence":manifest}),
        }).unwrap()
        };
        retain(manifest.clone());
        let malformed = super::input_history(&store, &input).unwrap();
        assert_eq!(malformed.evidence_gaps, 2);
        assert_eq!(malformed.harness, "claude");
        assert_eq!(malformed.model.as_deref(), Some("current-model"));
        assert_eq!(malformed.skill.as_deref(), Some("current-skill"));
        assert_eq!(
            malformed.worktree,
            Some(home.path().to_string_lossy().into_owned())
        );
        manifest["exec"] = serde_json::Value::Null;
        // A later valid observation uses the same saved-input reader and projector.
        store.retain_session_observation(&session, &crate::session::SessionObservation {
            artifact_key: input.clone(), source:"manifest.json:recovered".into(), observed_at:2,
            task_id:None, wave_id:None,
            payload:serde_json::json!({"input_id":input,"source":"manifest.json","evidence":manifest}),
        }).unwrap();
        let recovered = super::input_history(&store, &input).unwrap();
        assert_eq!(recovered.evidence_gaps, 1);
        assert_eq!(recovered.harness, "codex");
        assert_eq!(recovered.model.as_deref(), Some("saved-model"));
        assert_eq!(recovered.worktree.as_deref(), Some("/saved"));
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
        let store =
            crate::store::opening::open_ephemeral(&directory.path().join("registry.db")).unwrap();
        let session = super::create_session(&store, candidate(&cwd), None).unwrap();
        assert_eq!(
            session.repo,
            Some(crate::repository::discover(&repo).unwrap().to_string())
        );
        assert_eq!(session.cwd, cwd);
        assert_eq!(store.session(&session.id).unwrap(), Some(session));
    }

    #[test]
    fn repository_primary_resolves_after_its_checkout_is_deleted() {
        let directory = tempfile::tempdir().unwrap();
        let checkout = directory.path().join("repository");
        std::fs::create_dir(&checkout).unwrap();
        let repo = crate::repository::discover(&checkout).unwrap();
        let scope = PrimaryScope::Repository(repo.clone());
        let store =
            crate::store::opening::open_ephemeral(&directory.path().join("registry.db")).unwrap();
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

#[cfg(test)]
pub(crate) mod history_tests;
