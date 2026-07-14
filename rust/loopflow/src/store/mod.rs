//! Daemonless local persistence shared by `lf`, Waves, Projects, and Tasks.

use std::path::{Component, PathBuf};
use std::sync::Arc;

use crate::id::WaveId;
use crate::wave::Wave;
mod child_sessions;
pub mod migrations;
pub mod rows;
pub mod sqlite;
mod token_crypto;

/// One row of the machine-grain run ledger (`run_events`): a lifecycle event
/// for a run, flow, or skill, written directly by `lf` into the
/// local store. Token/cost fields are cumulative snapshots populated on skill
/// boundaries and terminal run events when the stream reported them.
#[derive(Debug, Clone, PartialEq)]
pub struct RunEventRow {
    pub run_id: String,
    pub process_id: String,
    pub parent_process_id: Option<String>,
    pub seq: i64,
    pub ts: i64,
    pub repo: Option<String>,
    pub worktree: Option<String>,
    pub wave: Option<String>,
    pub node: String,
    pub event: String,
    pub command: Option<String>,
    pub flow: Option<String>,
    pub skill: Option<String>,
    pub step_index: Option<i64>,
    pub error: Option<String>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub cost_usd: Option<f64>,
    pub duration_secs: Option<f64>,
    /// The harness the tokens were spent through. NULL when the process never
    /// launched an agent.
    pub provider: Option<String>,
    /// The configured model, when the agent launch names one.
    pub model: Option<String>,
}

/// One frame on the agent bus (`bus_messages`). `byline` is testimony — what
/// the publishing client said it was — and `channel` is evidence: where the
/// row actually arrived. Nothing derives identity server-side, so a forged
/// byline shows up as a mismatch between the two rather than being prevented.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BusMessage {
    /// Monotonic id; a subscriber's cursor is an id it has already read.
    pub id: i64,
    pub channel: String,
    pub byline: String,
    pub text: String,
    /// Unix seconds. The sweeper's window is measured against this.
    pub at: i64,
}

/// One wave's locally readable PM projection. Linear owns the payload; sync
/// replaces this row atomically so readers never observe a partial refresh.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PmSnapshotRow {
    pub repo: String,
    pub wave: String,
    pub provider: String,
    pub initiative: String,
    pub synced_at: i64,
    pub payload: String,
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("not found")]
    NotFound,
    #[error("invalid data: {0}")]
    InvalidData(String),
}

pub type StoreResult<T> = Result<T, StoreError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageConfig {
    Sqlite { path: PathBuf },
}

impl StorageConfig {
    pub fn sqlite(path: PathBuf) -> Self {
        Self::Sqlite { path }
    }
}

/// A debug build's default home. An `lf` built from a worktree must not open the
/// live ledger by accident: its schema is in flight, and applying an unreleased
/// migration to the real registry is not something the operator asked for.
///
/// This is not hypothetical. While W2-130 was being tested, a `cargo test` run of
/// this very branch applied its own unreleased migration to `~/.lf/loopflow.db` —
/// the bug reproducing itself inside its own fix. An explicit `LF_HOME` or
/// `LF_DB_PATH` still points a debug build wherever the operator says, including
/// at the live store; only the *silent default* moves.
#[cfg(debug_assertions)]
const DEFAULT_HOME_DIR: &str = ".lf-dev";
#[cfg(not(debug_assertions))]
const DEFAULT_HOME_DIR: &str = ".lf";

pub(crate) fn lf_home_dir() -> PathBuf {
    std::env::var_os("LF_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(DEFAULT_HOME_DIR)
        })
}

pub fn default_db_path() -> PathBuf {
    lf_home_dir().join("loopflow.db")
}

pub fn database_path_from_env() -> Result<PathBuf, std::io::Error> {
    let candidate = std::env::var_os("LF_DB_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(default_db_path);
    let path = if candidate.is_absolute() {
        candidate
    } else {
        if candidate.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        }) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "LF_DB_PATH must not escape LF_HOME",
            ));
        }
        lf_home_dir().join(candidate)
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(path)
}

pub fn storage_config_from_env() -> Result<StorageConfig, std::io::Error> {
    Ok(StorageConfig::sqlite(database_path_from_env()?))
}

#[derive(Debug)]
pub struct Store {
    sqlite: sqlite::SqliteStore,
}

async fn run_sqlite<T, F>(store: &sqlite::SqliteStore, func: F) -> StoreResult<T>
where
    T: Send + 'static,
    F: FnOnce(sqlite::SqliteStore) -> StoreResult<T> + Send + 'static,
{
    let store = store.clone();
    tokio::task::spawn_blocking(move || func(store))
        .await
        .map_err(|err| StoreError::InvalidData(err.to_string()))?
}

impl Store {
    pub async fn put_pm_snapshot(&self, snapshot: PmSnapshotRow) -> StoreResult<()> {
        run_sqlite(&self.sqlite, move |store| store.put_pm_snapshot(&snapshot)).await
    }

    pub async fn pm_snapshot(
        &self,
        repo: String,
        wave: String,
    ) -> StoreResult<Option<PmSnapshotRow>> {
        run_sqlite(&self.sqlite, move |store| store.pm_snapshot(&repo, &wave)).await
    }

    // The agent bus: publish is an INSERT, subscribe is a forward poll from an
    // id cursor. No server is in the path; the tables live in the baseline schema.

    /// Publish one frame, stamped now. Returns its id.
    pub async fn publish_bus(
        &self,
        channel: String,
        byline: String,
        text: String,
    ) -> StoreResult<i64> {
        let at = time::OffsetDateTime::now_utc().unix_timestamp();
        run_sqlite(&self.sqlite, move |store| {
            store.publish_bus(&channel, &byline, &text, at)
        })
        .await
    }

    /// Drop every frame published before `cutoff` (unix seconds). Production
    /// never sweeps by hand — publishing and every read carry the broom (see
    /// [`Self::swept_read`]); this aims it at a chosen cutoff so a test can
    /// age the bus out without waiting the window.
    #[cfg(test)]
    pub async fn sweep_bus(&self, cutoff: i64) -> StoreResult<usize> {
        run_sqlite(&self.sqlite, move |store| store.sweep_bus(cutoff)).await
    }

    /// Sweep the window, then read. The retention window is enforced by
    /// whoever looks, not only by whoever publishes next, so a lone report on
    /// a bus that then went quiet expires on schedule instead of waiting for a
    /// writer that may never come. Every bus read rides this broom.
    async fn swept_read<T, F>(&self, read: F) -> StoreResult<T>
    where
        T: Send + 'static,
        F: FnOnce(sqlite::SqliteStore) -> StoreResult<T> + Send + 'static,
    {
        let cutoff = time::OffsetDateTime::now_utc().unix_timestamp() - sqlite::BUS_WINDOW_SECS;
        run_sqlite(&self.sqlite, move |store| {
            store.sweep_bus(cutoff)?;
            read(store)
        })
        .await
    }

    /// Every surviving frame after `cursor`, oldest first.
    pub async fn read_bus_after(&self, cursor: i64) -> StoreResult<Vec<BusMessage>> {
        self.swept_read(move |store| store.read_bus_after(cursor))
            .await
    }

    /// The high-water mark — where a fresh subscriber tunes in.
    pub async fn bus_head(&self) -> StoreResult<i64> {
        self.swept_read(|store| store.bus_head()).await
    }

    /// The oldest readable id. A durable cursor below `floor - 1` is a gap:
    /// frames this subscriber will never see.
    pub async fn bus_floor(&self) -> StoreResult<Option<i64>> {
        self.swept_read(|store| store.bus_floor()).await
    }

    pub async fn bus_cursor(&self, subscriber: String) -> StoreResult<Option<i64>> {
        run_sqlite(&self.sqlite, move |store| store.bus_cursor(&subscriber)).await
    }

    pub async fn set_bus_cursor(&self, subscriber: String, cursor: i64) -> StoreResult<()> {
        run_sqlite(&self.sqlite, move |store| {
            store.set_bus_cursor(&subscriber, cursor)
        })
        .await
    }

    pub async fn list_waves(&self, repo: Option<&str>) -> StoreResult<Vec<Wave>> {
        let repo = repo.map(str::to_string);
        run_sqlite(&self.sqlite, move |store| store.list_waves(repo.as_deref())).await
    }

    /// A chord's contents: the waves whose `parent_wave_id` is `parent`,
    /// ordered by creation.
    pub async fn list_child_waves(&self, parent: &WaveId) -> StoreResult<Vec<Wave>> {
        let parent = parent.clone();
        run_sqlite(&self.sqlite, move |store| store.list_child_waves(&parent)).await
    }

    pub async fn get_wave(&self, wave_id: &WaveId) -> StoreResult<Option<Wave>> {
        let wave_id = wave_id.clone();
        run_sqlite(&self.sqlite, move |store| store.get_wave(&wave_id)).await
    }

    pub async fn get_wave_by_name(&self, name: &str) -> StoreResult<Option<Wave>> {
        let name = name.to_string();
        run_sqlite(&self.sqlite, move |store| store.get_wave_by_name(&name)).await
    }

    pub async fn create_wave(&self, wave: &Wave) -> StoreResult<()> {
        let wave = wave.clone();
        run_sqlite(&self.sqlite, move |store| store.create_wave(&wave)).await
    }

    pub async fn update_wave(&self, wave: &Wave) -> StoreResult<()> {
        let wave = wave.clone();
        run_sqlite(&self.sqlite, move |store| store.update_wave(&wave)).await
    }

    pub async fn delete_wave(&self, wave_id: &WaveId) -> StoreResult<()> {
        let wave_id = wave_id.clone();
        run_sqlite(&self.sqlite, move |store| store.delete_wave(&wave_id)).await
    }

    pub async fn get_provider_token(&self, provider: &str) -> StoreResult<Option<ProviderToken>> {
        let provider = provider.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.get_provider_token(&provider)
        })
        .await
    }

    pub async fn upsert_provider_token(&self, token: &ProviderToken) -> StoreResult<()> {
        let token = token.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.upsert_provider_token(&token)
        })
        .await
    }

    pub async fn delete_provider_token(&self, provider: &str) -> StoreResult<()> {
        let provider = provider.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.delete_provider_token(&provider)
        })
        .await
    }

    pub async fn list_provider_tokens(&self) -> StoreResult<Vec<ProviderToken>> {
        run_sqlite(&self.sqlite, |store| store.list_provider_tokens()).await
    }

    pub async fn health_check(&self) -> StoreResult<()> {
        run_sqlite(&self.sqlite, |store| store.health_check()).await
    }

    pub async fn schema_version(&self) -> StoreResult<String> {
        run_sqlite(&self.sqlite, |store| store.schema_version()).await
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CredentialType {
    OAuth,
    ApiKey,
}

impl CredentialType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OAuth => "oauth",
            Self::ApiKey => "apikey",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "apikey" => Self::ApiKey,
            _ => Self::OAuth,
        }
    }
}

impl std::fmt::Display for CredentialType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderToken {
    pub provider: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub oauth_client_id: Option<String>,
    pub expires_at: Option<i64>,
    pub login: Option<String>,
    pub updated_at: i64,
    pub credential_type: CredentialType,
}

pub async fn open_store(cfg: &StorageConfig) -> StoreResult<Store> {
    let StorageConfig::Sqlite { path } = cfg;
    Ok(Store {
        sqlite: sqlite::SqliteStore::new(path)?,
    })
}

/// Open the machine's shared registry store only if one already exists.
/// `None` means this machine has no registry yet; callers that instrument best-effort (lf
/// self-registration, the wave server) treat that as "not instrumented" and
/// stay silent rather than conjuring an empty db.
pub async fn open_existing_store() -> Option<Store> {
    let cfg = crate::store::storage_config_from_env().ok()?;
    let StorageConfig::Sqlite { path } = &cfg;
    if !path.exists() {
        return None;
    }
    // Opening validates the one live schema. An incompatible store is never
    // repaired in place.
    let conn = rusqlite::Connection::open(path).ok()?;
    if let Err(err) = migrations::apply_sqlite(&conn) {
        tracing::warn!(?path, %err, "local store is incompatible; delete it and rerun the command");
        return None;
    }
    open_store(&cfg).await.ok()
}

pub type SharedStore = Arc<Store>;

#[cfg(test)]
mod tests {
    use super::sqlite::SqliteStore;
    use super::{open_store, PmSnapshotRow, RunEventRow, StorageConfig};
    use crate::child_session::{
        BoundaryResult, ChildCommand, ChildCommandEffect, ChildCommandKind, ChildCommandSource,
        ChildCommandState, ChildDecisionId, ChildDirective, ChildProcessGeneration, ChildRef,
        ObservationRecipient,
    };
    use crate::id::WaveId;
    use crate::project_session::{
        ChildEventPayload, ProjectEventKind, ProjectSession, ProjectSessionId, ProjectSessionStatus,
    };
    use crate::session_context::{
        LinearIssueId, LinearIssueSnapshot, LinearProjectId, LinearProjectSnapshot,
        ProjectLaunchReceipt, TaskLaunchReceipt,
    };
    use crate::task::{
        AfterMerge, GithubPr, PmWritebackState, PrPhase, PrPublication, TaskEventKind, TaskPr,
        TaskPrId, TaskSession, TaskSessionId, TaskSessionStatus,
    };
    use crate::wave::Wave;
    use std::env;
    use std::path::PathBuf;
    use time::OffsetDateTime;

    fn make_wave(repo: &str) -> Wave {
        let id = WaveId::new();
        Wave::new(id.clone(), format!("wave-{id}"), repo.to_string())
    }

    fn make_task_session(wave: &Wave, project: &ProjectSession) -> TaskSession {
        let now = OffsetDateTime::from_unix_timestamp(OffsetDateTime::now_utc().unix_timestamp())
            .expect("current unix time");
        let id = TaskSessionId::new();
        TaskSession {
            id: id.clone(),
            launch: TaskLaunchReceipt {
                issue: LinearIssueSnapshot {
                    id: LinearIssueId::new("issue-uuid").unwrap(),
                    identifier: "INF-123".to_string(),
                    title: "Add hello world".to_string(),
                    description: "Ship one command".to_string(),
                },
                project: project.launch.project.clone(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            pm_writeback: PmWritebackState::Current,
            wave_id: wave.id().clone(),
            project_session_id: project.id.clone(),
            current_directive_version: 0,
            incorporated_directive_version: 0,
            status: TaskSessionStatus::Created,
            status_reason: "task session reserved".to_string(),
            status_at: now,
            worktree: PathBuf::from("/repo.inf-123"),
            workspace_slug: format!("task-{}", &id.as_str()[3..11]),
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: None,
            latest_process: None,
            execution: Some(crate::child_session::ChildExecutionContext::for_tests()),
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn make_task_pr(session: &TaskSession) -> TaskPr {
        TaskPr {
            id: TaskPrId::new(),
            task_session_id: session.id.clone(),
            sequence: 1,
            slug: session.workspace_slug.clone(),
            branch: format!("jack/{}", session.workspace_slug),
            base_commit: "deadbeef".to_string(),
            publication: None,
            merge_commit: None,
            abandoned_at: None,
            created_at: session.created_at,
            updated_at: session.updated_at,
        }
    }

    fn make_project_session(wave: &Wave) -> ProjectSession {
        let now = OffsetDateTime::from_unix_timestamp(OffsetDateTime::now_utc().unix_timestamp())
            .expect("current unix time");
        ProjectSession {
            id: ProjectSessionId::new(),
            launch: ProjectLaunchReceipt {
                project: LinearProjectSnapshot {
                    id: LinearProjectId::new("project-uuid").unwrap(),
                    slug: "developer-efficiency".to_string(),
                    name: "Developer Efficiency".to_string(),
                    prompt_context: "Definition:\nKeep local work fast.".to_string(),
                },
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            wave_id: wave.id().clone(),
            current_directive_version: 0,
            incorporated_directive_version: 0,
            status: ProjectSessionStatus::Running,
            status_reason: "project turn active".to_string(),
            status_at: now,
            iteration: 1,
            observation_cursor: 0,
            last_state_fingerprint: None,
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: Some("thread-project".to_string()),
            latest_process: Some(ChildProcessGeneration {
                generation: 1,
                pid: None,
                tmux_name: "lf-project-test".to_string(),
                started_at: now,
            }),
            execution: Some(crate::child_session::ChildExecutionContext::for_tests()),
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn terminal_project_history_keeps_one_current_successor() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();

        let mut predecessor = make_project_session(&wave);
        predecessor.set_status(ProjectSessionStatus::Abandoned, "legacy Session ended");
        store.create_project_session(&predecessor).await.unwrap();

        let mut successor = make_project_session(&wave);
        successor.status = ProjectSessionStatus::Created;
        successor.status_reason = format!("successor to {}", predecessor.id);
        successor.latest_process = None;
        successor.provider_session_id = None;
        successor.created_at += time::Duration::SECOND;
        successor.updated_at = successor.created_at;
        store.create_project_session(&successor).await.unwrap();

        assert_eq!(
            store
                .get_project_session_by_project("developer-efficiency")
                .await
                .unwrap()
                .unwrap()
                .id,
            successor.id
        );
        assert_eq!(
            store
                .get_project_session_by_project(predecessor.id.as_str())
                .await
                .unwrap()
                .unwrap()
                .id,
            predecessor.id
        );
        assert_eq!(
            store
                .list_project_sessions(Some(wave.id()))
                .await
                .unwrap()
                .len(),
            2
        );

        let mut parallel = successor.clone();
        parallel.id = ProjectSessionId::new();
        assert!(store.create_project_session(&parallel).await.is_err());
    }

    #[tokio::test]
    async fn child_directives_reserve_replace_and_incorporate_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();

        let mut project = make_project_session(&wave);
        project.current_directive_version = 1;
        let project_target = ChildRef::Project(project.id.clone());
        let project_initial = ChildDirective::initial(
            project_target.clone(),
            "Pursue onboarding first".to_string(),
            ChildCommandSource::Wave(wave.id().clone()),
        );
        store
            .create_project_session_with_directive(&project, &project_initial)
            .await
            .unwrap();
        let stale_project = project.clone();

        let mut task = make_task_session(&wave, &project);
        task.current_directive_version = 1;
        let task_target = ChildRef::Task(task.id.clone());
        let task_initial = ChildDirective::initial(
            task_target.clone(),
            "Fix the parser before the docs".to_string(),
            ChildCommandSource::Wave(wave.id().clone()),
        );
        store
            .reserve_task_session_with_directive(&task, &make_task_pr(&task), &task_initial)
            .await
            .unwrap();
        assert_eq!(store.child_directives(&task_target).await.unwrap().len(), 1);
        let stale_task = task.clone();

        let task_command = ChildCommand::new(
            ChildRef::Task(task.id.clone()),
            ChildCommandSource::Wave(wave.id().clone()),
            ChildCommandKind::Steer {
                text: "Fix the parser before the docs or tests".to_string(),
            },
        );
        let task_replacement = ChildDirective::replacement(
            task_target.clone(),
            2,
            "Fix the parser before the docs or tests".to_string(),
            task_command.source.clone(),
            task_command.id.clone(),
        );
        store
            .create_child_command_with_directive(&task_command, &task_replacement)
            .await
            .unwrap();
        store
            .mark_child_directive_applied(&task_target, 2)
            .await
            .unwrap();
        store
            .incorporate_child_directive(&task_target, 2, "Parser remains first")
            .await
            .unwrap();
        store.update_task_session(&stale_task).await.unwrap();
        let persisted_task = store.get_task_session(&task.id).await.unwrap().unwrap();
        assert_eq!(persisted_task.current_directive_version, 2);
        assert_eq!(persisted_task.incorporated_directive_version, 2);

        let command = ChildCommand::new(
            ChildRef::Project(project.id.clone()),
            ChildCommandSource::Wave(wave.id().clone()),
            ChildCommandKind::Steer {
                text: "Prove the parser path first".to_string(),
            },
        );
        let replacement = ChildDirective::replacement(
            project_target.clone(),
            2,
            "Prove the parser path first".to_string(),
            command.source.clone(),
            command.id.clone(),
        );
        store
            .create_child_command_with_directive(&command, &replacement)
            .await
            .unwrap();
        let persisted = store
            .get_project_session(&project.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(persisted.current_directive_version, 2);
        assert_eq!(
            store.child_directives(&project_target).await.unwrap().len(),
            2
        );
        assert!(store
            .incorporate_child_directive(&project_target, 1, "stale")
            .await
            .is_err());
        store
            .mark_child_directive_applied(&project_target, 2)
            .await
            .unwrap();
        assert!(
            store
                .incorporate_child_directive(&project_target, 2, "Parser is now first")
                .await
                .unwrap()
                .1
        );
        assert!(
            !store
                .incorporate_child_directive(&project_target, 2, "Parser is now first")
                .await
                .unwrap()
                .1
        );
        store.update_project_session(&stale_project).await.unwrap();
        assert_eq!(
            store
                .get_project_session(&project.id)
                .await
                .unwrap()
                .unwrap()
                .incorporated_directive_version,
            2
        );
    }

    #[tokio::test]
    async fn project_sessions_persist_commands_and_receive_task_observations() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project_session(&wave);
        store.create_project_session(&project).await.unwrap();

        let command = ChildCommand::new(
            ChildRef::Project(project.id.clone()),
            ChildCommandSource::Wave(wave.id().clone()),
            ChildCommandKind::FollowUp {
                text: "audit the parser".to_string(),
            },
        );
        store.create_child_command(&command).await.unwrap();
        let claimed = store
            .claim_child_commands(&ChildRef::Project(project.id.clone()), 1)
            .await
            .unwrap();
        assert_eq!(claimed.len(), 1);
        store
            .accept_child_command(&command.id, Some(ChildCommandEffect::NextTurn))
            .await
            .unwrap();
        let stored = store.get_child_command(&command.id).await.unwrap().unwrap();
        assert_eq!(stored.state, ChildCommandState::Accepted);
        assert_eq!(stored.target, ChildRef::Project(project.id.clone()));
        let decision = ChildDecisionId::new();
        let decision_command = ChildCommand::new(
            ChildRef::Project(project.id.clone()),
            ChildCommandSource::Wave(wave.id().clone()),
            ChildCommandKind::Decide {
                decision_id: decision,
                choice: "approve".to_string(),
                message: None,
            },
        );
        assert!(
            store
                .ensure_child_decision_command(&decision_command)
                .await
                .unwrap()
                .1
        );
        assert!(
            !store
                .ensure_child_decision_command(&decision_command)
                .await
                .unwrap()
                .1
        );

        let task = make_task_session(&wave, &project);
        store
            .create_task_session(&task, &make_task_pr(&task))
            .await
            .unwrap();
        store
            .sqlite
            .append_task_event(
                &task.id,
                &TaskEventKind::Failed {
                    error: "provider stopped".to_string(),
                    resumable: true,
                },
            )
            .unwrap();
        let observations = store
            .pending_observations(&ObservationRecipient::Project {
                session_id: project.id.clone(),
            })
            .await
            .unwrap();
        assert_eq!(observations.len(), 1);
        assert!(matches!(
            (&observations[0].source, &observations[0].payload),
            (
                ChildRef::Task(session_id),
                ChildEventPayload::Task { event: TaskEventKind::Failed { .. } }
            ) if session_id == &task.id
        ));
        let wave_observations = store
            .pending_observations(&ObservationRecipient::Wave {
                wave_id: wave.id().clone(),
            })
            .await
            .unwrap();
        assert!(wave_observations.iter().any(|observation| matches!(
            (&observation.source, &observation.payload),
            (
                ChildRef::Task(session_id),
                ChildEventPayload::Task { event: TaskEventKind::Failed { .. } }
            ) if session_id == &task.id
        )));
        assert!(store
            .consume_task_observation_for_project(&project.id, &observations[0])
            .await
            .unwrap());
        assert!(!store
            .consume_task_observation_for_project(&project.id, &observations[0])
            .await
            .unwrap());
        let project_events = store.project_events_after(&project.id, 0).await.unwrap();
        assert_eq!(
            project_events
                .iter()
                .filter(|event| matches!(
                    event.kind,
                    crate::project_session::ProjectEventKind::TaskObserved { .. }
                ))
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn task_session_requires_its_matching_project_session() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project_session(&wave);
        let task = make_task_session(&wave, &project);

        let missing = store
            .create_task_session(&task, &make_task_pr(&task))
            .await
            .unwrap_err();
        assert!(missing.to_string().contains("requires Project Session"));

        store.create_project_session(&project).await.unwrap();
        let mut wrong_project = make_task_session(&wave, &project);
        wrong_project.launch.project.id = LinearProjectId::new("another-project").unwrap();
        let mismatched = store
            .create_task_session(&wrong_project, &make_task_pr(&wrong_project))
            .await
            .unwrap_err();
        assert!(mismatched.to_string().contains("does not own Task"));

        let other_wave = make_wave("/other-repo");
        store.create_wave(&other_wave).await.unwrap();
        let wrong_wave = make_task_session(&other_wave, &project);
        let mismatched = store
            .create_task_session(&wrong_wave, &make_task_pr(&wrong_wave))
            .await
            .unwrap_err();
        assert!(mismatched.to_string().contains("does not own Task"));
    }

    #[tokio::test]
    async fn project_supervision_routes_task_decisions_through_one_escalation_boundary() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project_session(&wave);
        store.create_project_session(&project).await.unwrap();
        let task = make_task_session(&wave, &project);
        store
            .create_task_session(&task, &make_task_pr(&task))
            .await
            .unwrap();

        let task_decision_id = ChildDecisionId::new();
        store
            .sqlite
            .append_task_event(
                &task.id,
                &TaskEventKind::DecisionRequested {
                    decision_id: task_decision_id.clone(),
                    prompt: "Use the strict parser?".to_string(),
                    options: vec!["strict".to_string(), "permissive".to_string()],
                },
            )
            .unwrap();

        let project_observations = store
            .pending_observations(&ObservationRecipient::Project {
                session_id: project.id.clone(),
            })
            .await
            .unwrap();
        assert_eq!(project_observations.len(), 1);
        assert!(matches!(
            &project_observations[0].payload,
            ChildEventPayload::Task {
                event: TaskEventKind::DecisionRequested { decision_id, .. }
            } if decision_id == &task_decision_id
        ));
        assert!(store
            .pending_observations(&ObservationRecipient::Wave {
                wave_id: wave.id().clone(),
            })
            .await
            .unwrap()
            .is_empty());
        assert!(store
            .consume_task_observation_for_project(&project.id, &project_observations[0])
            .await
            .unwrap());
        assert!(!store
            .consume_task_observation_for_project(&project.id, &project_observations[0])
            .await
            .unwrap());

        let project_decision_id = ChildDecisionId::new();
        store
            .append_project_event(
                &project.id,
                &ProjectEventKind::DecisionRequested {
                    decision_id: project_decision_id.clone(),
                    prompt: format!(
                        "Task decision {task_decision_id} needs Wave judgment: use the strict parser?"
                    ),
                    options: vec!["strict".to_string(), "permissive".to_string()],
                },
            )
            .await
            .unwrap();
        let wave_observations = store
            .pending_observations(&ObservationRecipient::Wave {
                wave_id: wave.id().clone(),
            })
            .await
            .unwrap();
        assert_eq!(wave_observations.len(), 1);
        assert!(matches!(
            &wave_observations[0].payload,
            ChildEventPayload::Project {
                event: ProjectEventKind::DecisionRequested { decision_id, .. }
            } if decision_id == &project_decision_id
        ));
    }

    #[tokio::test]
    async fn task_session_commands_reclaim_by_generation_and_events_are_durable() {
        let dir = tempfile::tempdir().unwrap();
        let store = super::open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project_session(&wave);
        store.create_project_session(&project).await.unwrap();
        let session = make_task_session(&wave, &project);
        store
            .create_task_session(&session, &make_task_pr(&session))
            .await
            .unwrap();

        let loaded = store
            .get_task_session_by_issue("INF-123")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(loaded, session);

        let command = ChildCommand::new(
            ChildRef::Task(session.id.clone()),
            ChildCommandSource::Human,
            ChildCommandKind::Steer {
                text: "rename the flag".to_string(),
            },
        );
        store.create_child_command(&command).await.unwrap();
        let first_claim = store
            .claim_child_commands(&ChildRef::Task(session.id.clone()), 1)
            .await
            .unwrap();
        assert_eq!(first_claim.len(), 1);
        assert_eq!(first_claim[0].id, command.id);
        assert_eq!(first_claim[0].kind, command.kind);
        assert_eq!(first_claim[0].state, ChildCommandState::Claimed);
        assert_eq!(first_claim[0].claimed_by_generation, Some(1));
        assert_eq!(
            store
                .claim_child_commands(&ChildRef::Task(session.id.clone()), 2)
                .await
                .unwrap()[0]
                .claimed_by_generation,
            Some(2)
        );
        store
            .accept_child_command(&command.id, Some(ChildCommandEffect::LiveSteer))
            .await
            .unwrap();
        let accepted = store.get_child_command(&command.id).await.unwrap().unwrap();
        assert_eq!(accepted.state, ChildCommandState::Accepted);
        assert_eq!(accepted.effect, Some(ChildCommandEffect::LiveSteer));
        assert!(store
            .claim_child_commands(&ChildRef::Task(session.id.clone()), 3)
            .await
            .unwrap()
            .is_empty());

        let follow_up_a = ChildCommand::new(
            ChildRef::Task(session.id.clone()),
            ChildCommandSource::Human,
            ChildCommandKind::FollowUp {
                text: "A".to_string(),
            },
        );
        let follow_up_b = ChildCommand::new(
            ChildRef::Task(session.id.clone()),
            ChildCommandSource::Human,
            ChildCommandKind::FollowUp {
                text: "B".to_string(),
            },
        );
        store.create_child_command(&follow_up_a).await.unwrap();
        store.create_child_command(&follow_up_b).await.unwrap();
        let interrupt = ChildCommand::new(
            ChildRef::Task(session.id.clone()),
            ChildCommandSource::Human,
            ChildCommandKind::Interrupt {
                replacement: Some("C".to_string()),
            },
        );
        let superseded = store
            .supersede_and_create_child_command(&interrupt)
            .await
            .unwrap();
        assert_eq!(
            superseded,
            vec![follow_up_a.id.clone(), follow_up_b.id.clone()]
        );
        assert_eq!(
            store
                .get_child_command(&follow_up_a.id)
                .await
                .unwrap()
                .unwrap()
                .state,
            ChildCommandState::Superseded
        );
        assert_eq!(
            store
                .claim_child_commands(&ChildRef::Task(session.id.clone()), 3)
                .await
                .unwrap()
                .into_iter()
                .map(|command| command.id)
                .collect::<Vec<_>>(),
            vec![interrupt.id.clone()]
        );
        store
            .fail_child_command(
                &interrupt.id,
                Some(ChildCommandEffect::Replacement),
                "provider control failed".to_string(),
            )
            .await
            .unwrap();
        let failed = store
            .get_child_command(&interrupt.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(failed.state, ChildCommandState::Failed);
        assert_eq!(failed.effect, Some(ChildCommandEffect::Replacement));
        assert_eq!(failed.error.as_deref(), Some("provider control failed"));
        assert!(store
            .claim_child_commands(&ChildRef::Task(session.id.clone()), 4)
            .await
            .unwrap()
            .is_empty());

        let event = store
            .append_task_event(
                &session.id,
                &TaskEventKind::Progress {
                    summary: "tests pass".to_string(),
                },
            )
            .await
            .unwrap();
        assert_eq!(
            store.task_events_after(&session.id, 0).await.unwrap(),
            vec![event]
        );

        let mut second = make_task_session(&wave, &project);
        second.launch.issue.id = LinearIssueId::new("issue-two").unwrap();
        second.launch.issue.identifier = "INF-124".to_string();
        second.worktree = PathBuf::from("/repo.inf-124");
        store
            .create_task_session(&second, &make_task_pr(&second))
            .await
            .unwrap();
        assert!(store
            .get_task_session_by_issue("INF-124")
            .await
            .unwrap()
            .is_some());
    }

    #[tokio::test]
    async fn task_prs_are_ordered_and_rotation_is_atomic() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project_session(&wave);
        store.create_project_session(&project).await.unwrap();
        let session = make_task_session(&wave, &project);
        let mut first = make_task_pr(&session);
        store.create_task_session(&session, &first).await.unwrap();

        first.publication = Some(PrPublication {
            requested_at: first.updated_at,
            after_merge: AfterMerge::Review,
            next_slug: None,
            github: Some(GithubPr {
                number: 101,
                url: "https://github.com/loopflowstudio/loopflow/pull/101".to_string(),
            }),
        });
        first.merge_commit = Some("merge-101".to_string());
        first.updated_at = OffsetDateTime::now_utc();
        let now = OffsetDateTime::now_utc();
        let second = TaskPr {
            id: TaskPrId::new(),
            task_session_id: session.id.clone(),
            sequence: 2,
            slug: "released-proof".to_string(),
            branch: format!("jack/{}-released-proof", session.workspace_slug),
            base_commit: "main-after-101".to_string(),
            publication: None,
            merge_commit: None,
            abandoned_at: None,
            created_at: now,
            updated_at: now,
        };
        store.settle_task_pr(&first, Some(&second)).await.unwrap();
        store.settle_task_pr(&first, Some(&second)).await.unwrap();

        assert_eq!(
            store
                .task_prs(&session.id)
                .await
                .unwrap()
                .iter()
                .map(|pr| pr.sequence)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(
            store.active_task_pr(&session.id).await.unwrap().unwrap().id,
            second.id
        );

        let mut abandoned = second.clone();
        let abandoned_at = OffsetDateTime::now_utc();
        abandoned.abandoned_at = Some(abandoned_at);
        abandoned.updated_at = abandoned_at;
        let conflicting = TaskPr {
            id: TaskPrId::new(),
            task_session_id: session.id.clone(),
            sequence: 3,
            slug: "conflict".to_string(),
            branch: first.branch.clone(),
            base_commit: "main-after-102".to_string(),
            publication: None,
            merge_commit: None,
            abandoned_at: None,
            created_at: now,
            updated_at: now,
        };
        assert!(store
            .settle_task_pr(&abandoned, Some(&conflicting))
            .await
            .is_err());
        assert_eq!(
            store
                .active_task_pr(&session.id)
                .await
                .unwrap()
                .unwrap()
                .phase(),
            PrPhase::Working
        );
    }

    #[tokio::test]
    async fn pr_publication_round_trips_before_github_exists() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project_session(&wave);
        store.create_project_session(&project).await.unwrap();
        let session = make_task_session(&wave, &project);
        let mut pr = make_task_pr(&session);
        store.create_task_session(&session, &pr).await.unwrap();

        pr.publication = Some(PrPublication {
            requested_at: pr.updated_at,
            after_merge: AfterMerge::CompleteTask,
            next_slug: None,
            github: None,
        });
        store.update_task_pr(&pr).await.unwrap();

        let publishing = store.active_task_pr(&session.id).await.unwrap().unwrap();
        assert_eq!(publishing.phase(), PrPhase::Publishing);
        assert_eq!(publishing.publication, pr.publication);

        pr.publication.as_mut().unwrap().github = Some(GithubPr {
            number: 101,
            url: "https://github.com/loopflowstudio/loopflow/pull/101".to_string(),
        });
        store.update_task_pr(&pr).await.unwrap();

        let open = store.active_task_pr(&session.id).await.unwrap().unwrap();
        assert_eq!(open.phase(), PrPhase::Open);
        assert_eq!(open.publication, pr.publication);
    }

    #[tokio::test]
    async fn empty_pr_is_skipped_when_task_completes() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project_session(&wave);
        store.create_project_session(&project).await.unwrap();
        let mut session = make_task_session(&wave, &project);
        let pr = make_task_pr(&session);
        store.create_task_session(&session, &pr).await.unwrap();

        session.set_status(TaskSessionStatus::Completed, "investigation recorded");
        store
            .complete_task_session(&session, Some(&pr))
            .await
            .unwrap();

        assert_eq!(
            store
                .get_task_session(&session.id)
                .await
                .unwrap()
                .unwrap()
                .status,
            TaskSessionStatus::Completed
        );
        let stored = store.task_prs(&session.id).await.unwrap();
        assert!(stored.is_empty());
    }

    #[tokio::test]
    async fn provider_delivery_is_never_replayed_after_an_ambiguous_crash() {
        let dir = tempfile::tempdir().unwrap();
        let store = super::open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project_session(&wave);
        store.create_project_session(&project).await.unwrap();
        let session = make_task_session(&wave, &project);
        store
            .create_task_session(&session, &make_task_pr(&session))
            .await
            .unwrap();
        let target = ChildRef::Task(session.id.clone());
        let command = ChildCommand::new(
            target.clone(),
            ChildCommandSource::Human,
            ChildCommandKind::Steer {
                text: "change direction".to_string(),
            },
        );
        store.create_child_command(&command).await.unwrap();
        store.claim_child_commands(&target, 1).await.unwrap();
        store
            .mark_child_command_delivering(&command.id, ChildCommandEffect::LiveSteer)
            .await
            .unwrap();

        assert!(store
            .claim_child_commands(&target, 2)
            .await
            .unwrap()
            .is_empty());
        let uncertain = store
            .mark_stale_child_deliveries_uncertain(&target, 2)
            .await
            .unwrap();
        assert_eq!(uncertain.len(), 1);
        assert_eq!(uncertain[0].state, ChildCommandState::Uncertain);
        assert!(uncertain[0].state.is_terminal());
        assert!(store
            .mark_stale_child_deliveries_uncertain(&target, 3)
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn task_boundary_atomically_claims_work_or_stops_the_generation() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project_session(&wave);
        store.create_project_session(&project).await.unwrap();

        let mut with_command = make_task_session(&wave, &project);
        with_command.begin_generation("task-a".to_string());
        with_command.set_status(TaskSessionStatus::Running, "provider active");
        store
            .create_task_session(&with_command, &make_task_pr(&with_command))
            .await
            .unwrap();
        let command = ChildCommand::new(
            ChildRef::Task(with_command.id.clone()),
            ChildCommandSource::Human,
            ChildCommandKind::FollowUp {
                text: "arrived at the boundary".to_string(),
            },
        );
        store.create_child_command(&command).await.unwrap();

        let claimed = store
            .claim_task_commands_or_stop(
                &with_command.id,
                1,
                TaskSessionStatus::Waiting,
                "turn complete",
            )
            .await
            .unwrap();
        let BoundaryResult::Commands(commands) = claimed else {
            panic!("boundary stopped despite a persisted command");
        };
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].id, command.id);
        assert_eq!(
            store
                .get_task_session(&with_command.id)
                .await
                .unwrap()
                .unwrap()
                .status,
            TaskSessionStatus::Running
        );

        let mut without_command = make_task_session(&wave, &project);
        without_command.launch.issue.id = LinearIssueId::new("other-issue").unwrap();
        without_command.launch.issue.identifier = "INF-124".to_string();
        without_command.worktree = PathBuf::from("/repo.inf-124");
        without_command.begin_generation("task-b".to_string());
        without_command.set_status(TaskSessionStatus::Running, "provider active");
        store
            .create_task_session(&without_command, &make_task_pr(&without_command))
            .await
            .unwrap();
        let stopped = store
            .claim_task_commands_or_stop(
                &without_command.id,
                1,
                TaskSessionStatus::Waiting,
                "turn complete",
            )
            .await
            .unwrap();
        let BoundaryResult::Stopped(stopped) = stopped else {
            panic!("empty boundary did not stop");
        };
        assert_eq!(stopped.status, TaskSessionStatus::Waiting);
        assert_eq!(stopped.status_reason, "turn complete");
    }

    #[tokio::test]
    async fn duplicate_task_decision_reuses_one_durable_command() {
        let dir = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project_session(&wave);
        store.create_project_session(&project).await.unwrap();
        let session = make_task_session(&wave, &project);
        store
            .create_task_session(&session, &make_task_pr(&session))
            .await
            .unwrap();
        let decision_id = ChildDecisionId::new();
        let first = ChildCommand::new(
            ChildRef::Task(session.id.clone()),
            ChildCommandSource::Human,
            ChildCommandKind::Decide {
                decision_id: decision_id.clone(),
                choice: "revise".to_string(),
                message: Some("cover the boundary".to_string()),
            },
        );
        let duplicate = ChildCommand::new(
            ChildRef::Task(session.id.clone()),
            ChildCommandSource::Human,
            ChildCommandKind::Decide {
                decision_id,
                choice: "approve".to_string(),
                message: None,
            },
        );

        let (stored, created) = store.ensure_child_decision_command(&first).await.unwrap();
        assert!(created);
        assert_eq!(stored.id, first.id);
        let (stored, created) = store
            .ensure_child_decision_command(&duplicate)
            .await
            .unwrap();
        assert!(!created);
        assert_eq!(stored.id, first.id);
        assert_eq!(
            store
                .list_child_commands(&ChildRef::Task(session.id.clone()))
                .await
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn task_process_resume_reserves_each_session_generation_once() {
        let dir = tempfile::tempdir().unwrap();
        let store = super::open_store(&StorageConfig::sqlite(dir.path().join("registry.db")))
            .await
            .unwrap();
        let wave = make_wave("/repo");
        store.create_wave(&wave).await.unwrap();
        let project = make_project_session(&wave);
        store.create_project_session(&project).await.unwrap();

        let mut session = make_task_session(&wave, &project);
        session.set_status(TaskSessionStatus::Waiting, "waiting for review");
        store
            .create_task_session(&session, &make_task_pr(&session))
            .await
            .unwrap();

        let mut first_resume = session.clone();
        assert_eq!(first_resume.begin_generation("task-one".to_string()), 1);
        assert!(store
            .reserve_task_process(&first_resume, TaskSessionStatus::Waiting)
            .await
            .unwrap());

        let mut racing_resume = session.clone();
        racing_resume.begin_generation("task-one".to_string());
        assert!(!store
            .reserve_task_process(&racing_resume, TaskSessionStatus::Waiting)
            .await
            .unwrap());

        let mut second = make_task_session(&wave, &project);
        second.launch.issue.id = LinearIssueId::new("issue-two").unwrap();
        second.launch.issue.identifier = "INF-124".to_string();
        second.worktree = PathBuf::from("/repo.inf-124");
        second.set_status(TaskSessionStatus::Waiting, "waiting for work");
        store
            .create_task_session(&second, &make_task_pr(&second))
            .await
            .unwrap();
        second.begin_generation("task-two".to_string());
        assert!(store
            .reserve_task_process(&second, TaskSessionStatus::Waiting)
            .await
            .unwrap());

        let loaded = store.get_task_session(&session.id).await.unwrap().unwrap();
        assert_eq!(loaded.status, TaskSessionStatus::Starting);
        assert_eq!(loaded.latest_process.unwrap().generation, 1);
    }

    async fn run_store_basic_suite(store: &super::Store) {
        let mut wave = make_wave("/repo");
        store.create_wave(&wave).await.expect("create wave");
        assert!(store.get_wave(wave.id()).await.expect("get wave").is_some());

        wave.repo = "/repo-updated".to_string();
        store.update_wave(&wave).await.expect("update wave");
        let loaded = store
            .get_wave(wave.id())
            .await
            .expect("get wave")
            .expect("wave exists");
        assert_eq!(loaded.repo(), "/repo-updated");

        store.delete_wave(wave.id()).await.expect("delete wave");
        assert!(store
            .get_wave(wave.id())
            .await
            .expect("get deleted wave")
            .is_none());
    }

    #[tokio::test]
    async fn sqlite_store_basic_suite() {
        let db_path = env::temp_dir().join(format!("loopflow-test-{}.db", WaveId::new()));
        let config = StorageConfig::sqlite(db_path);
        let store = super::open_store(&config).await.expect("store should open");
        run_store_basic_suite(&store).await;
    }

    // A chord is a wave whose children point back at it via `parent_wave_id`.
    // `list_child_waves` returns those children (ordered, repos stitched); a
    // leaf wave returns none. This is the ancestry the WaveAgentTree needs.
    #[tokio::test]
    async fn sqlite_wave_ancestry_and_children() {
        let db_path = env::temp_dir().join(format!("loopflow-test-{}.db", WaveId::new()));
        let config = StorageConfig::sqlite(db_path);
        let store = super::open_store(&config).await.expect("store should open");

        let parent = make_wave("/chord");
        store.create_wave(&parent).await.expect("create parent");

        let child_a = make_wave("/repo-a").with_parent(parent.id().clone());
        let child_b = make_wave("/repo-b").with_parent(parent.id().clone());
        store.create_wave(&child_a).await.expect("create child a");
        store.create_wave(&child_b).await.expect("create child b");

        // Wave exposes its parent relation after a round-trip.
        let reloaded = store
            .get_wave(child_a.id())
            .await
            .expect("get child")
            .expect("child exists");
        assert_eq!(reloaded.parent_wave_id(), Some(parent.id()));

        // Chord contents = children where parent_wave_id = id, one per repo.
        let children = store
            .list_child_waves(parent.id())
            .await
            .expect("list children");
        assert_eq!(children.len(), 2);
        let repos: Vec<&str> = children.iter().map(|w| w.repo()).collect();
        assert!(repos.contains(&"/repo-a"));
        assert!(repos.contains(&"/repo-b"));

        // A leaf wave has no children.
        assert!(store
            .list_child_waves(child_a.id())
            .await
            .expect("leaf children")
            .is_empty());

        // The root wave itself has no parent.
        let root = store
            .get_wave(parent.id())
            .await
            .expect("get parent")
            .expect("parent exists");
        assert_eq!(root.parent_wave_id(), None);
    }

    /// A run_events row with no usage attached.
    fn event_row(run_id: &str, seq: i64, node: &str, event: &str) -> RunEventRow {
        RunEventRow {
            run_id: run_id.to_string(),
            process_id: run_id.to_string(),
            parent_process_id: None,
            seq,
            ts: seq,
            repo: Some("/repo".to_string()),
            worktree: None,
            wave: None,
            node: node.to_string(),
            event: event.to_string(),
            command: None,
            flow: None,
            skill: None,
            step_index: None,
            error: None,
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            cost_usd: None,
            duration_secs: None,
            provider: None,
            model: None,
        }
    }

    #[tokio::test]
    async fn pm_snapshot_replacement_is_atomic_per_wave() {
        let db_path = env::temp_dir().join(format!("loopflow-test-{}.db", WaveId::new()));
        let store = open_store(&StorageConfig::sqlite(db_path.clone()))
            .await
            .expect("store should open");
        let mut snapshot = PmSnapshotRow {
            repo: "/repo".to_string(),
            wave: "product".to_string(),
            provider: "linear".to_string(),
            initiative: "initiative-1".to_string(),
            synced_at: 1,
            payload: "{\"version\":1}".to_string(),
        };
        store
            .put_pm_snapshot(snapshot.clone())
            .await
            .expect("write snapshot");
        snapshot.synced_at = 2;
        snapshot.payload = "{\"version\":2}".to_string();
        store
            .put_pm_snapshot(snapshot.clone())
            .await
            .expect("replace snapshot");

        assert_eq!(
            store
                .pm_snapshot("/repo".to_string(), "product".to_string())
                .await
                .expect("read snapshot"),
            Some(snapshot)
        );
        let _ = std::fs::remove_file(db_path);
    }

    #[test]
    fn a_closed_vocabulary_rejects_an_unknown_node() {
        let db_path = env::temp_dir().join(format!("loopflow-test-{}.db", WaveId::new()));
        let store = SqliteStore::new(&db_path).expect("store should open");
        let row = event_row("bad-node", 0, "task", "started");
        let error = store
            .insert_run_event(&row)
            .expect_err("unknown node must violate the ledger contract");
        assert!(error.to_string().contains("CHECK constraint failed"));
    }

    #[tokio::test]
    async fn sqlite_health_check_succeeds() {
        let db_path = env::temp_dir().join(format!("loopflow-test-{}.db", WaveId::new()));
        let config = StorageConfig::sqlite(db_path);
        let store = super::open_store(&config).await.expect("store should open");

        store.health_check().await.expect("sqlite health check");
    }

    #[tokio::test]
    async fn provider_token_round_trip() {
        let db_path = env::temp_dir().join(format!("loopflow-test-{}.db", WaveId::new()));
        let config = StorageConfig::sqlite(db_path);
        let store = super::open_store(&config).await.expect("store should open");

        // Initially empty
        assert!(store
            .list_provider_tokens()
            .await
            .expect("list empty")
            .is_empty());
        assert!(store
            .get_provider_token("github")
            .await
            .expect("get missing")
            .is_none());

        // Upsert a token
        let token = super::ProviderToken {
            provider: "github".to_string(),
            access_token: "gho_abc123".to_string(),
            refresh_token: Some("ghr_refresh".to_string()),
            oauth_client_id: Some("github-client".to_string()),
            expires_at: Some(1700000000),
            login: Some("octocat".to_string()),
            updated_at: 1699000000,
            credential_type: super::CredentialType::OAuth,
        };
        store
            .upsert_provider_token(&token)
            .await
            .expect("upsert token");

        // Read it back
        let loaded = store
            .get_provider_token("github")
            .await
            .expect("get token")
            .expect("token should exist");
        assert_eq!(loaded.provider, "github");
        assert_eq!(loaded.access_token, "gho_abc123");
        assert_eq!(loaded.refresh_token.as_deref(), Some("ghr_refresh"));
        assert_eq!(loaded.oauth_client_id.as_deref(), Some("github-client"));
        assert_eq!(loaded.expires_at, Some(1700000000));
        assert_eq!(loaded.login.as_deref(), Some("octocat"));

        // Upsert overwrites
        let updated = super::ProviderToken {
            access_token: "gho_new456".to_string(),
            refresh_token: None,
            updated_at: 1699500000,
            ..token.clone()
        };
        store
            .upsert_provider_token(&updated)
            .await
            .expect("upsert update");
        let reloaded = store
            .get_provider_token("github")
            .await
            .expect("get updated")
            .expect("exists");
        assert_eq!(reloaded.access_token, "gho_new456");
        assert!(reloaded.refresh_token.is_none());

        assert_eq!(reloaded.credential_type, super::CredentialType::OAuth);

        // Add a second provider and list
        let claude_token = super::ProviderToken {
            provider: "claude".to_string(),
            access_token: "sk-ant-key".to_string(),
            refresh_token: None,
            oauth_client_id: None,
            expires_at: None,
            login: None,
            updated_at: 1699000000,
            credential_type: super::CredentialType::OAuth,
        };
        store
            .upsert_provider_token(&claude_token)
            .await
            .expect("upsert claude");
        let all = store.list_provider_tokens().await.expect("list all");
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].provider, "claude");
        assert_eq!(all[1].provider, "github");

        // Delete one
        store
            .delete_provider_token("github")
            .await
            .expect("delete github");
        assert!(store
            .get_provider_token("github")
            .await
            .expect("get deleted")
            .is_none());
        assert_eq!(
            store
                .list_provider_tokens()
                .await
                .expect("list after delete")
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn provider_tokens_are_encrypted_at_rest_in_sqlite() {
        let db_path = env::temp_dir().join(format!("loopflow-test-{}.db", WaveId::new()));
        let config = StorageConfig::sqlite(db_path.clone());
        let store = super::open_store(&config).await.expect("store should open");
        let token = super::ProviderToken {
            provider: "github".to_string(),
            access_token: "gho_secret_access".to_string(),
            refresh_token: Some("ghr_secret_refresh".to_string()),
            oauth_client_id: Some("github-client".to_string()),
            expires_at: Some(1700000000),
            login: Some("octocat".to_string()),
            updated_at: 1699000000,
            credential_type: super::CredentialType::OAuth,
        };
        store
            .upsert_provider_token(&token)
            .await
            .expect("upsert token");

        let conn = rusqlite::Connection::open(db_path).expect("open sqlite db");
        let (raw_access, raw_refresh, encrypted): (String, Option<String>, bool) = conn
            .query_row(
                "SELECT access_token, refresh_token, encrypted FROM provider_tokens WHERE provider = 'github'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("query provider token");

        assert_ne!(raw_access, "gho_secret_access");
        assert_ne!(
            raw_refresh.as_deref(),
            Some("ghr_secret_refresh"),
            "refresh token should be encrypted"
        );
        assert!(encrypted, "encrypted flag should be true");
    }

    #[tokio::test]
    async fn sqlite_open_migrates_existing_plaintext_provider_tokens() {
        let db_path = env::temp_dir().join(format!("loopflow-test-{}.db", WaveId::new()));
        {
            let conn = rusqlite::Connection::open(&db_path).expect("open sqlite db");
            super::migrations::apply_sqlite(&conn).expect("apply migrations");
            conn.execute(
                "INSERT INTO provider_tokens
                 (provider, access_token, refresh_token, expires_at, login, updated_at, credential_type, encrypted)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0)",
                rusqlite::params![
                    "github",
                    "gho_plaintext",
                    "ghr_plaintext",
                    1700000000_i64,
                    "octocat",
                    1699000000_i64,
                    "oauth",
                ],
            )
            .expect("insert plaintext token");
        }

        let store = super::open_store(&StorageConfig::sqlite(db_path.clone()))
            .await
            .expect("open store");

        let loaded = store
            .get_provider_token("github")
            .await
            .expect("get token")
            .expect("token exists");
        assert_eq!(loaded.access_token, "gho_plaintext");
        assert_eq!(loaded.refresh_token.as_deref(), Some("ghr_plaintext"));

        let conn = rusqlite::Connection::open(db_path).expect("open sqlite db");
        let (raw_access, encrypted): (String, bool) = conn
            .query_row(
                "SELECT access_token, encrypted FROM provider_tokens WHERE provider = 'github'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("query provider token");
        assert_ne!(raw_access, "gho_plaintext");
        assert!(encrypted);
    }
}
