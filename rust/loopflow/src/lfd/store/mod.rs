use std::path::PathBuf;
use std::sync::Arc;

use crate::lfd::id::LfdId;
use crate::lfd::types::{
    Agent, LivePrState, LivePullRequestState, PendingActivation, Stimulus, Summary, Wave, WaveRun,
    WaveRunStackStatus,
};

pub mod migrations;
pub mod postgres;
pub mod rows;
pub mod sqlite;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForkRunStatus {
    Pending = 0,
    Running = 1,
    Completed = 2,
    Failed = 3,
}

impl ForkRunStatus {
    pub fn from_i64(value: i64) -> Option<Self> {
        match value {
            0 => Some(Self::Pending),
            1 => Some(Self::Running),
            2 => Some(Self::Completed),
            3 => Some(Self::Failed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ForkRun {
    pub id: LfdId,
    pub wave_run_id: LfdId,
    pub step_index: u32,
    pub branch_index: u32,
    pub status: ForkRunStatus,
    pub worktree: String,
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("postgres error: {0}")]
    Postgres(#[from] tokio_postgres::Error),
    #[error("postgres pool error: {0}")]
    PostgresPool(#[from] deadpool_postgres::PoolError),
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
    Postgres { database_url: String },
}

impl StorageConfig {
    pub fn sqlite(path: PathBuf) -> Self {
        Self::Sqlite { path }
    }

    pub fn postgres(database_url: impl Into<String>) -> Self {
        Self::Postgres {
            database_url: database_url.into(),
        }
    }
}

#[derive(Debug)]
pub struct Store {
    backend: StoreBackend,
}

#[derive(Debug)]
enum StoreBackend {
    Sqlite(sqlite::SqliteStore),
    Postgres(postgres::PostgresStore),
}

impl Store {
    pub fn into_shared(self) -> SharedStore {
        match self.backend {
            StoreBackend::Sqlite(store) => Arc::new(store) as SharedStore,
            StoreBackend::Postgres(store) => Arc::new(store) as SharedStore,
        }
    }

    fn as_run_store(&self) -> &dyn RunStore {
        match &self.backend {
            StoreBackend::Sqlite(store) => store,
            StoreBackend::Postgres(store) => store,
        }
    }
}

#[async_trait::async_trait]
pub trait WaveStateStore: Send + Sync {
    async fn list_waves(&self, repo: Option<&str>) -> StoreResult<Vec<Wave>>;
    async fn get_wave(&self, wave_id: &LfdId) -> StoreResult<Option<Wave>>;
    async fn get_wave_by_name(&self, name: &str) -> StoreResult<Option<Wave>>;
    async fn create_wave(&self, wave: &Wave) -> StoreResult<()>;
    async fn update_wave(&self, wave: &Wave) -> StoreResult<()>;
    async fn delete_wave(&self, wave_id: &LfdId) -> StoreResult<()>;

    async fn list_wave_runs(
        &self,
        wave_id: Option<&LfdId>,
        limit: Option<u32>,
    ) -> StoreResult<Vec<WaveRun>>;
    async fn get_wave_run(&self, wave_run_id: &LfdId) -> StoreResult<Option<WaveRun>>;
    async fn get_active_wave_run(&self, wave_id: &LfdId) -> StoreResult<Option<WaveRun>>;
    async fn get_latest_wave_run(&self, wave_id: &LfdId) -> StoreResult<Option<WaveRun>>;
    async fn create_wave_run(&self, run: &WaveRun) -> StoreResult<()>;
    async fn update_wave_run(&self, run: &WaveRun) -> StoreResult<()>;
    async fn list_stack_runs(&self, wave_id: &LfdId) -> StoreResult<Vec<WaveRun>>;
    async fn find_next_unmerged_run(&self, wave_id: &LfdId) -> StoreResult<Option<WaveRun>>;
    async fn find_descendants(&self, run_id: &LfdId) -> StoreResult<Vec<WaveRun>>;
    async fn get_live_pr_state(
        &self,
        repo_id: &str,
        pr_number: u32,
    ) -> StoreResult<Option<LivePullRequestState>>;
    async fn upsert_live_pr_state(&self, state: &LivePullRequestState) -> StoreResult<()>;

    async fn list_stimuli(&self, wave_id: Option<&LfdId>) -> StoreResult<Vec<Stimulus>>;
    async fn list_stimuli_by_kind(&self, kind: i32) -> StoreResult<Vec<Stimulus>>;
    async fn get_stimulus(&self, stimulus_id: &LfdId) -> StoreResult<Option<Stimulus>>;
    async fn create_stimulus(&self, stimulus: &Stimulus) -> StoreResult<()>;
    async fn update_stimulus(&self, stimulus: &Stimulus) -> StoreResult<()>;
    async fn delete_stimulus(&self, stimulus_id: &LfdId) -> StoreResult<()>;
    async fn delete_stimuli_for_wave(&self, wave_id: &LfdId) -> StoreResult<u32>;

    async fn list_pending_activations(
        &self,
        wave_id: &LfdId,
    ) -> StoreResult<Vec<PendingActivation>>;
    async fn create_pending_activation(&self, activation: &PendingActivation) -> StoreResult<()>;
    async fn update_pending_activation(&self, activation: &PendingActivation) -> StoreResult<()>;
    async fn delete_pending_activations(&self, wave_id: &LfdId) -> StoreResult<u32>;
    async fn get_pending_for_stimulus(
        &self,
        wave_id: &LfdId,
        stimulus_id: &LfdId,
    ) -> StoreResult<Option<PendingActivation>>;

    async fn get_summary(&self, wave_id: &LfdId) -> StoreResult<Option<Summary>>;
    async fn upsert_summary(&self, summary: &Summary) -> StoreResult<()>;
}

#[async_trait::async_trait]
pub trait ExecutionStore: Send + Sync {
    async fn list_fork_runs(
        &self,
        wave_run_id: &LfdId,
        step_index: u32,
    ) -> StoreResult<Vec<ForkRun>>;
    async fn upsert_fork_run(&self, fork_run: &ForkRun) -> StoreResult<()>;
    async fn delete_fork_runs(&self, wave_run_id: &LfdId, step_index: u32) -> StoreResult<u32>;

    async fn list_agents(&self) -> StoreResult<Vec<Agent>>;
    async fn list_agent_history(
        &self,
        worktree: Option<&str>,
        repo: Option<&str>,
        limit: Option<u32>,
    ) -> StoreResult<Vec<Agent>>;
    async fn get_agent(&self, agent_id: &LfdId) -> StoreResult<Option<Agent>>;
    async fn get_waiting_agent_for_wave(&self, wave_id: &LfdId) -> StoreResult<Option<Agent>>;
    async fn start_agent(&self, agent: &Agent) -> StoreResult<()>;
    async fn update_agent_status(
        &self,
        agent_id: &LfdId,
        status: i32,
        pid: Option<u32>,
        container_id: Option<&str>,
    ) -> StoreResult<()>;
    async fn end_agent(&self, agent_id: &LfdId, status: i32, ended_at: i64) -> StoreResult<()>;
    async fn get_active_agents_for_wave(&self, wave_id: &LfdId) -> StoreResult<Vec<Agent>>;
    async fn end_active_agent_for_wave(
        &self,
        wave_id: &LfdId,
        status: i32,
        ended_at: i64,
    ) -> StoreResult<()>;
    async fn get_stuck_agents(&self, older_than_secs: u64) -> StoreResult<Vec<Agent>>;

    async fn fail_orphaned_runs(&self) -> StoreResult<u32>;
}

#[async_trait::async_trait]
pub trait StoreAdmin: Send + Sync {
    async fn health_check(&self) -> StoreResult<()>;
    async fn schema_version(&self) -> StoreResult<String>;
}

#[async_trait::async_trait]
impl WaveStateStore for Store {
    async fn list_waves(&self, repo: Option<&str>) -> StoreResult<Vec<Wave>> {
        self.as_run_store().list_waves(repo)
    }

    async fn get_wave(&self, wave_id: &LfdId) -> StoreResult<Option<Wave>> {
        self.as_run_store().get_wave(wave_id)
    }

    async fn get_wave_by_name(&self, name: &str) -> StoreResult<Option<Wave>> {
        self.as_run_store().get_wave_by_name(name)
    }

    async fn create_wave(&self, wave: &Wave) -> StoreResult<()> {
        self.as_run_store().create_wave(wave)
    }

    async fn update_wave(&self, wave: &Wave) -> StoreResult<()> {
        self.as_run_store().update_wave(wave)
    }

    async fn delete_wave(&self, wave_id: &LfdId) -> StoreResult<()> {
        self.as_run_store().delete_wave(wave_id)
    }

    async fn list_wave_runs(
        &self,
        wave_id: Option<&LfdId>,
        limit: Option<u32>,
    ) -> StoreResult<Vec<WaveRun>> {
        self.as_run_store().list_wave_runs(wave_id, limit)
    }

    async fn get_wave_run(&self, wave_run_id: &LfdId) -> StoreResult<Option<WaveRun>> {
        self.as_run_store().get_wave_run(wave_run_id)
    }

    async fn get_active_wave_run(&self, wave_id: &LfdId) -> StoreResult<Option<WaveRun>> {
        self.as_run_store().get_active_wave_run(wave_id)
    }

    async fn get_latest_wave_run(&self, wave_id: &LfdId) -> StoreResult<Option<WaveRun>> {
        self.as_run_store().get_latest_wave_run(wave_id)
    }

    async fn create_wave_run(&self, run: &WaveRun) -> StoreResult<()> {
        self.as_run_store().create_wave_run(run)
    }

    async fn update_wave_run(&self, run: &WaveRun) -> StoreResult<()> {
        self.as_run_store().update_wave_run(run)
    }

    async fn list_stack_runs(&self, wave_id: &LfdId) -> StoreResult<Vec<WaveRun>> {
        self.as_run_store().list_stack_runs(wave_id)
    }

    async fn find_next_unmerged_run(&self, wave_id: &LfdId) -> StoreResult<Option<WaveRun>> {
        self.as_run_store().find_next_unmerged_run(wave_id)
    }

    async fn find_descendants(&self, run_id: &LfdId) -> StoreResult<Vec<WaveRun>> {
        self.as_run_store().find_descendants(run_id)
    }

    async fn get_live_pr_state(
        &self,
        repo_id: &str,
        pr_number: u32,
    ) -> StoreResult<Option<LivePullRequestState>> {
        self.as_run_store().get_live_pr_state(repo_id, pr_number)
    }

    async fn upsert_live_pr_state(&self, state: &LivePullRequestState) -> StoreResult<()> {
        self.as_run_store().upsert_live_pr_state(state)
    }

    async fn list_stimuli(&self, wave_id: Option<&LfdId>) -> StoreResult<Vec<Stimulus>> {
        self.as_run_store().list_stimuli(wave_id)
    }

    async fn list_stimuli_by_kind(&self, kind: i32) -> StoreResult<Vec<Stimulus>> {
        self.as_run_store().list_stimuli_by_kind(kind)
    }

    async fn get_stimulus(&self, stimulus_id: &LfdId) -> StoreResult<Option<Stimulus>> {
        self.as_run_store().get_stimulus(stimulus_id)
    }

    async fn create_stimulus(&self, stimulus: &Stimulus) -> StoreResult<()> {
        self.as_run_store().create_stimulus(stimulus)
    }

    async fn update_stimulus(&self, stimulus: &Stimulus) -> StoreResult<()> {
        self.as_run_store().update_stimulus(stimulus)
    }

    async fn delete_stimulus(&self, stimulus_id: &LfdId) -> StoreResult<()> {
        self.as_run_store().delete_stimulus(stimulus_id)
    }

    async fn delete_stimuli_for_wave(&self, wave_id: &LfdId) -> StoreResult<u32> {
        self.as_run_store().delete_stimuli_for_wave(wave_id)
    }

    async fn list_pending_activations(
        &self,
        wave_id: &LfdId,
    ) -> StoreResult<Vec<PendingActivation>> {
        self.as_run_store().list_pending_activations(wave_id)
    }

    async fn create_pending_activation(&self, activation: &PendingActivation) -> StoreResult<()> {
        self.as_run_store().create_pending_activation(activation)
    }

    async fn update_pending_activation(&self, activation: &PendingActivation) -> StoreResult<()> {
        self.as_run_store().update_pending_activation(activation)
    }

    async fn delete_pending_activations(&self, wave_id: &LfdId) -> StoreResult<u32> {
        self.as_run_store().delete_pending_activations(wave_id)
    }

    async fn get_pending_for_stimulus(
        &self,
        wave_id: &LfdId,
        stimulus_id: &LfdId,
    ) -> StoreResult<Option<PendingActivation>> {
        self.as_run_store()
            .get_pending_for_stimulus(wave_id, stimulus_id)
    }

    async fn get_summary(&self, wave_id: &LfdId) -> StoreResult<Option<Summary>> {
        self.as_run_store().get_summary(wave_id)
    }

    async fn upsert_summary(&self, summary: &Summary) -> StoreResult<()> {
        self.as_run_store().upsert_summary(summary)
    }
}

#[async_trait::async_trait]
impl ExecutionStore for Store {
    async fn list_fork_runs(
        &self,
        wave_run_id: &LfdId,
        step_index: u32,
    ) -> StoreResult<Vec<ForkRun>> {
        self.as_run_store().list_fork_runs(wave_run_id, step_index)
    }

    async fn upsert_fork_run(&self, fork_run: &ForkRun) -> StoreResult<()> {
        self.as_run_store().upsert_fork_run(fork_run)
    }

    async fn delete_fork_runs(&self, wave_run_id: &LfdId, step_index: u32) -> StoreResult<u32> {
        self.as_run_store()
            .delete_fork_runs(wave_run_id, step_index)
    }

    async fn list_agents(&self) -> StoreResult<Vec<Agent>> {
        self.as_run_store().list_agents()
    }

    async fn list_agent_history(
        &self,
        worktree: Option<&str>,
        repo: Option<&str>,
        limit: Option<u32>,
    ) -> StoreResult<Vec<Agent>> {
        self.as_run_store()
            .list_agent_history(worktree, repo, limit)
    }

    async fn get_agent(&self, agent_id: &LfdId) -> StoreResult<Option<Agent>> {
        self.as_run_store().get_agent(agent_id)
    }

    async fn get_waiting_agent_for_wave(&self, wave_id: &LfdId) -> StoreResult<Option<Agent>> {
        self.as_run_store().get_waiting_agent_for_wave(wave_id)
    }

    async fn start_agent(&self, agent: &Agent) -> StoreResult<()> {
        self.as_run_store().start_agent(agent)
    }

    async fn update_agent_status(
        &self,
        agent_id: &LfdId,
        status: i32,
        pid: Option<u32>,
        container_id: Option<&str>,
    ) -> StoreResult<()> {
        self.as_run_store()
            .update_agent_status(agent_id, status, pid, container_id)
    }

    async fn end_agent(&self, agent_id: &LfdId, status: i32, ended_at: i64) -> StoreResult<()> {
        self.as_run_store().end_agent(agent_id, status, ended_at)
    }

    async fn get_active_agents_for_wave(&self, wave_id: &LfdId) -> StoreResult<Vec<Agent>> {
        self.as_run_store().get_active_agents_for_wave(wave_id)
    }

    async fn end_active_agent_for_wave(
        &self,
        wave_id: &LfdId,
        status: i32,
        ended_at: i64,
    ) -> StoreResult<()> {
        self.as_run_store()
            .end_active_agent_for_wave(wave_id, status, ended_at)
    }

    async fn get_stuck_agents(&self, older_than_secs: u64) -> StoreResult<Vec<Agent>> {
        self.as_run_store().get_stuck_agents(older_than_secs)
    }

    async fn fail_orphaned_runs(&self) -> StoreResult<u32> {
        self.as_run_store().fail_orphaned_runs()
    }
}

#[async_trait::async_trait]
impl StoreAdmin for Store {
    async fn health_check(&self) -> StoreResult<()> {
        self.as_run_store().health_check()
    }

    async fn schema_version(&self) -> StoreResult<String> {
        self.as_run_store().schema_version()
    }
}

pub async fn open_store(cfg: &StorageConfig) -> StoreResult<Store> {
    match cfg {
        StorageConfig::Sqlite { path } => Ok(Store {
            backend: StoreBackend::Sqlite(sqlite::SqliteStore::new(path)?),
        }),
        StorageConfig::Postgres { database_url } => Ok(Store {
            backend: StoreBackend::Postgres(
                postgres::PostgresStore::connect_async(database_url).await?,
            ),
        }),
    }
}

pub async fn migrate_store(cfg: &StorageConfig, status_only: bool) -> StoreResult<String> {
    match cfg {
        StorageConfig::Sqlite { path } => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|err| {
                    StoreError::InvalidData(format!("failed to create db dir: {err}"))
                })?;
            }
            let conn = rusqlite::Connection::open(path)?;
            if !status_only {
                migrations::apply_sqlite(&conn)?;
            }
            migrations::latest_version_sqlite(&conn)
        }
        StorageConfig::Postgres { database_url } => {
            if status_only {
                postgres::PostgresStore::migrate_status_async(database_url).await
            } else {
                postgres::PostgresStore::migrate_async(database_url).await
            }
        }
    }
}

#[allow(dead_code)]
pub trait RunStore: Send + Sync {
    fn health_check(&self) -> StoreResult<()>;
    fn schema_version(&self) -> StoreResult<String>;

    // Wave management
    fn list_waves(&self, repo: Option<&str>) -> StoreResult<Vec<Wave>>;
    fn get_wave(&self, wave_id: &LfdId) -> StoreResult<Option<Wave>>;
    fn get_wave_by_name(&self, name: &str) -> StoreResult<Option<Wave>>;
    fn create_wave(&self, wave: &Wave) -> StoreResult<()>;
    fn update_wave(&self, wave: &Wave) -> StoreResult<()>;
    fn delete_wave(&self, wave_id: &LfdId) -> StoreResult<()>;

    // Wave runs
    fn list_wave_runs(
        &self,
        wave_id: Option<&LfdId>,
        limit: Option<u32>,
    ) -> StoreResult<Vec<WaveRun>>;
    fn get_wave_run(&self, wave_run_id: &LfdId) -> StoreResult<Option<WaveRun>>;
    fn get_active_wave_run(&self, wave_id: &LfdId) -> StoreResult<Option<WaveRun>>;
    fn get_latest_wave_run(&self, wave_id: &LfdId) -> StoreResult<Option<WaveRun>>;
    fn create_wave_run(&self, run: &WaveRun) -> StoreResult<()>;
    fn update_wave_run(&self, run: &WaveRun) -> StoreResult<()>;
    fn list_stack_runs(&self, wave_id: &LfdId) -> StoreResult<Vec<WaveRun>>;
    fn find_next_unmerged_run(&self, wave_id: &LfdId) -> StoreResult<Option<WaveRun>> {
        let runs = self.list_stack_runs(wave_id)?;
        for run in runs {
            if matches!(
                run.stack_status,
                WaveRunStackStatus::Merged | WaveRunStackStatus::Superseded
            ) {
                continue;
            }

            let Some(pr_number) = run.snapshot.pr.as_ref().and_then(|pr| pr.number) else {
                return Ok(Some(run));
            };

            let Some(live_state) = self.get_live_pr_state(&run.snapshot.repo, pr_number)? else {
                return Ok(Some(run));
            };
            if live_state.state != LivePrState::Merged {
                return Ok(Some(run));
            }
        }
        Ok(None)
    }

    fn find_descendants(&self, run_id: &LfdId) -> StoreResult<Vec<WaveRun>> {
        let Some(parent) = self.get_wave_run(run_id)? else {
            return Ok(Vec::new());
        };
        let descendants = self
            .list_stack_runs(&parent.wave_id)?
            .into_iter()
            .filter(|run| {
                run.stack_group_id == parent.stack_group_id
                    && run.stack_position > parent.stack_position
            })
            .collect();
        Ok(descendants)
    }
    fn get_live_pr_state(
        &self,
        repo_id: &str,
        pr_number: u32,
    ) -> StoreResult<Option<LivePullRequestState>>;
    fn upsert_live_pr_state(&self, state: &LivePullRequestState) -> StoreResult<()>;
    /// Mark all Running/Pending/Waiting runs as Failed. Called on startup to
    /// clean up orphaned runs from a previous lfd process.
    fn fail_orphaned_runs(&self) -> StoreResult<u32>;

    // Stimulus management (many:1 with waves)
    fn list_stimuli(&self, wave_id: Option<&LfdId>) -> StoreResult<Vec<Stimulus>>;
    fn list_stimuli_by_kind(&self, kind: i32) -> StoreResult<Vec<Stimulus>>;
    fn get_stimulus(&self, stimulus_id: &LfdId) -> StoreResult<Option<Stimulus>>;
    fn create_stimulus(&self, stimulus: &Stimulus) -> StoreResult<()>;
    fn update_stimulus(&self, stimulus: &Stimulus) -> StoreResult<()>;
    fn delete_stimulus(&self, stimulus_id: &LfdId) -> StoreResult<()>;
    fn delete_stimuli_for_wave(&self, wave_id: &LfdId) -> StoreResult<u32>;

    // Pending activations (for coalescing triggers)
    fn list_pending_activations(&self, wave_id: &LfdId) -> StoreResult<Vec<PendingActivation>>;
    fn create_pending_activation(&self, activation: &PendingActivation) -> StoreResult<()>;
    fn update_pending_activation(&self, activation: &PendingActivation) -> StoreResult<()>;
    fn delete_pending_activations(&self, wave_id: &LfdId) -> StoreResult<u32>;
    fn get_pending_for_stimulus(
        &self,
        wave_id: &LfdId,
        stimulus_id: &LfdId,
    ) -> StoreResult<Option<PendingActivation>>;

    // Fork runs
    fn list_fork_runs(&self, wave_run_id: &LfdId, step_index: u32) -> StoreResult<Vec<ForkRun>>;
    fn upsert_fork_run(&self, fork_run: &ForkRun) -> StoreResult<()>;
    fn delete_fork_runs(&self, wave_run_id: &LfdId, step_index: u32) -> StoreResult<u32>;

    // Step runs
    fn list_agents(&self) -> StoreResult<Vec<Agent>>;
    fn list_agent_history(
        &self,
        worktree: Option<&str>,
        repo: Option<&str>,
        limit: Option<u32>,
    ) -> StoreResult<Vec<Agent>>;
    fn get_agent(&self, agent_id: &LfdId) -> StoreResult<Option<Agent>>;
    fn get_waiting_agent_for_wave(&self, wave_id: &LfdId) -> StoreResult<Option<Agent>>;
    fn start_agent(&self, agent: &Agent) -> StoreResult<()>;
    fn update_agent_status(
        &self,
        agent_id: &LfdId,
        status: i32,
        pid: Option<u32>,
        container_id: Option<&str>,
    ) -> StoreResult<()>;
    fn end_agent(&self, agent_id: &LfdId, status: i32, ended_at: i64) -> StoreResult<()>;
    fn get_active_agents_for_wave(&self, wave_id: &LfdId) -> StoreResult<Vec<Agent>>;
    fn end_active_agent_for_wave(
        &self,
        wave_id: &LfdId,
        status: i32,
        ended_at: i64,
    ) -> StoreResult<()>;
    fn get_stuck_agents(&self, older_than_secs: u64) -> StoreResult<Vec<Agent>>;

    // Summaries
    fn get_summary(&self, wave_id: &LfdId) -> StoreResult<Option<Summary>>;
    fn upsert_summary(&self, summary: &Summary) -> StoreResult<()>;
}

pub type SharedStore = Arc<dyn RunStore>;

#[cfg(test)]
mod tests {
    use super::{ForkRun, ForkRunStatus, RunStore};
    use crate::lfd::id::LfdId;
    use crate::lfd::types::{
        Agent, AgentStatus, LivePrState, LivePullRequestState, PendingActivation, PullRequest,
        SidecarKind, Stimulus, StimulusKind, Summary, Wave, WaveRun, WaveRunKind, WaveRunSnapshot,
        WaveRunStackStatus, WaveRunStatus, WaveStatus,
    };
    use std::env;
    use time::OffsetDateTime;

    fn make_wave(repo: &str) -> Wave {
        let id = LfdId::new();
        Wave {
            id: id.clone(),
            name: format!("wave-{id}"),
            repo: repo.to_string(),
            flow: "default".to_string(),
            direction: vec!["focus".to_string()],
            area: vec!["src".to_string()],
            status: WaveStatus::Idle,
            iteration: 0,
            created_at: Some(OffsetDateTime::now_utc()),
        }
    }

    fn make_stimulus(wave_id: &LfdId) -> Stimulus {
        Stimulus {
            id: LfdId::new(),
            wave_id: wave_id.clone(),
            kind: StimulusKind::Watch,
            cron: "".to_string(),
            last_main_sha: Some("abc123".to_string()),
            last_triggered_at: Some(100),
            created_at: Some(OffsetDateTime::now_utc()),
            enabled: true,
        }
    }

    fn make_pending_activation(wave_id: &LfdId, stimulus_id: &LfdId) -> PendingActivation {
        PendingActivation {
            id: LfdId::new(),
            wave_id: wave_id.clone(),
            stimulus_id: stimulus_id.clone(),
            from_sha: "aaa".to_string(),
            to_sha: "bbb".to_string(),
            queued_at: OffsetDateTime::now_utc().unix_timestamp(),
        }
    }

    fn make_agent(wave_run_id: Option<&LfdId>, status: AgentStatus, started_at: i64) -> Agent {
        Agent {
            id: LfdId::new(),
            step: "plan".to_string(),
            repo: "/repo".to_string(),
            worktree: "/repo".to_string(),
            wave_run_id: wave_run_id.cloned(),
            status,
            started_at: Some(OffsetDateTime::from_unix_timestamp(started_at).unwrap()),
            ended_at: None,
            pid: None,
            container_id: None,
            model: "claude-code".to_string(),
            run_mode: "auto".to_string(),
        }
    }

    fn run_store_suite(store: &dyn RunStore) {
        let mut wave = make_wave("/repo");
        store.create_wave(&wave).unwrap();

        let loaded = store.get_wave(&wave.id).unwrap().unwrap();
        assert_eq!(loaded.name, wave.name);

        wave.status = WaveStatus::Paused;
        store.update_wave(&wave).unwrap();
        let updated = store.get_wave(&wave.id).unwrap().unwrap();
        assert_eq!(updated.status, WaveStatus::Paused);

        let repo_waves = store.list_waves(Some(&wave.repo)).unwrap();
        assert!(!repo_waves.is_empty());

        let stimulus = make_stimulus(&wave.id);
        store.create_stimulus(&stimulus).unwrap();
        let listed = store.list_stimuli(Some(&wave.id)).unwrap();
        assert_eq!(listed.len(), 1);
        let by_kind = store
            .list_stimuli_by_kind(StimulusKind::Watch.as_i32())
            .unwrap();
        assert_eq!(by_kind.len(), 1);

        let mut stimulus_updated = stimulus.clone();
        stimulus_updated.cron = "0 9 * * *".to_string();
        store.update_stimulus(&stimulus_updated).unwrap();
        let loaded_stimulus = store.get_stimulus(&stimulus.id).unwrap().unwrap();
        assert_eq!(loaded_stimulus.cron, "0 9 * * *");

        let run = WaveRun {
            id: LfdId::new(),
            wave_id: wave.id.clone(),
            snapshot: WaveRunSnapshot {
                repo: wave.repo.clone(),
                flow: wave.flow.clone(),
                direction: wave.direction.clone(),
                area: wave.area.clone(),
                pr: None,
            },
            iteration: 1,
            step_index: 0,
            status: WaveRunStatus::Running,
            worktree: "/repo".to_string(),
            branch: "main".to_string(),
            started_at: Some(OffsetDateTime::now_utc()),
            ended_at: None,
            error: None,
            flow_parents: Vec::new(),
            run_kind: WaveRunKind::Main,
            sidecar_kind: None,
            parent_run_id: None,
            parent_pr_number: None,
            stack_position: 0,
            stack_group_id: wave.id.to_string(),
            stack_status: crate::lfd::types::WaveRunStackStatus::Active,
            lineage_inferred: false,
        };
        store.create_wave_run(&run).unwrap();
        let loaded_run = store.get_wave_run(&run.id).unwrap().unwrap();
        assert_eq!(loaded_run.wave_id, wave.id);
        assert_eq!(loaded_run.stack_position, 0);
        assert_eq!(loaded_run.stack_status, WaveRunStackStatus::Active);

        let run_with_pr = WaveRun {
            id: LfdId::new(),
            wave_id: wave.id.clone(),
            snapshot: WaveRunSnapshot {
                repo: wave.repo.clone(),
                flow: wave.flow.clone(),
                direction: wave.direction.clone(),
                area: wave.area.clone(),
                pr: Some(PullRequest {
                    url: "https://example.test/pr/42".to_string(),
                    number: Some(42),
                    state: Some("open".to_string()),
                    title: Some("feature".to_string()),
                    branch: Some("feature-42".to_string()),
                }),
            },
            iteration: 2,
            step_index: 0,
            status: WaveRunStatus::Running,
            worktree: "/repo".to_string(),
            branch: "feature-42".to_string(),
            started_at: Some(OffsetDateTime::now_utc()),
            ended_at: None,
            error: None,
            flow_parents: Vec::new(),
            run_kind: WaveRunKind::Main,
            sidecar_kind: None,
            parent_run_id: Some(run.id.clone()),
            parent_pr_number: None,
            stack_position: 1,
            stack_group_id: wave.id.to_string(),
            stack_status: WaveRunStackStatus::Active,
            lineage_inferred: false,
        };
        store.create_wave_run(&run_with_pr).unwrap();

        let stack_runs = store.list_stack_runs(&wave.id).unwrap();
        assert_eq!(stack_runs.len(), 2);
        assert_eq!(stack_runs[0].id, run.id);
        assert_eq!(stack_runs[1].id, run_with_pr.id);
        assert_eq!(stack_runs[1].parent_run_id, Some(run.id.clone()));

        let descendants = store.find_descendants(&run.id).unwrap();
        assert_eq!(descendants.len(), 1);
        assert_eq!(descendants[0].id, run_with_pr.id);

        let next_before_live = store.find_next_unmerged_run(&wave.id).unwrap();
        assert_eq!(next_before_live.map(|value| value.id), Some(run.id.clone()));

        let live_state = LivePullRequestState {
            repo_id: wave.repo.clone(),
            pr_number: 42,
            state: LivePrState::Merged,
            is_draft: false,
            head_ref: "feature-42".to_string(),
            head_sha: "abc123".to_string(),
            base_ref: "main".to_string(),
            updated_at: OffsetDateTime::now_utc(),
            merged_at: Some(OffsetDateTime::now_utc()),
            synced_at: OffsetDateTime::now_utc(),
        };
        store.upsert_live_pr_state(&live_state).unwrap();
        let loaded_live = store.get_live_pr_state(&wave.repo, 42).unwrap().unwrap();
        assert_eq!(loaded_live.state, LivePrState::Merged);

        let activation = make_pending_activation(&wave.id, &stimulus.id);
        store.create_pending_activation(&activation).unwrap();
        let activations = store.list_pending_activations(&wave.id).unwrap();
        assert_eq!(activations.len(), 1);
        let pending = store
            .get_pending_for_stimulus(&wave.id, &stimulus.id)
            .unwrap()
            .unwrap();
        assert_eq!(pending.id, activation.id);

        let wave_after_pending = store.get_wave(&wave.id).unwrap().unwrap();
        assert_eq!(wave_after_pending.id, wave.id);

        let mut activation_updated = activation.clone();
        activation_updated.to_sha = "ccc".to_string();
        store
            .update_pending_activation(&activation_updated)
            .unwrap();

        let deleted = store.delete_pending_activations(&wave.id).unwrap();
        assert_eq!(deleted, 1);
        let wave_after_delete = store.get_wave(&wave.id).unwrap().unwrap();
        assert_eq!(wave_after_delete.id, wave.id);

        let fork_run = ForkRun {
            id: LfdId::new(),
            wave_run_id: run.id.clone(),
            step_index: 0,
            branch_index: 0,
            status: ForkRunStatus::Pending,
            worktree: "/tmp/branch".to_string(),
        };
        store.upsert_fork_run(&fork_run).unwrap();
        let forks = store.list_fork_runs(&run.id, 0).unwrap();
        assert_eq!(forks.len(), 1);
        let deleted_forks = store.delete_fork_runs(&run.id, 0).unwrap();
        assert_eq!(deleted_forks, 1);

        let now = OffsetDateTime::now_utc().unix_timestamp();
        let agent = make_agent(Some(&run.id), AgentStatus::Waiting, now);
        store.start_agent(&agent).unwrap();
        let waiting = store.get_waiting_agent_for_wave(&wave.id).unwrap().unwrap();
        assert_eq!(waiting.id, agent.id);
        store
            .update_agent_status(
                &agent.id,
                AgentStatus::Running.as_i32(),
                Some(123),
                Some("container-123"),
            )
            .unwrap();
        let updated_agent = store.get_agent(&agent.id).unwrap().unwrap();
        assert_eq!(updated_agent.pid, Some(123));
        assert_eq!(updated_agent.container_id.as_deref(), Some("container-123"));
        store
            .end_agent(&agent.id, AgentStatus::Completed.as_i32(), now + 10)
            .unwrap();

        let old_run = make_agent(Some(&run.id), AgentStatus::Running, now - 3600);
        store.start_agent(&old_run).unwrap();
        let stuck = store.get_stuck_agents(60).unwrap();
        assert!(stuck.iter().any(|run| run.id == old_run.id));

        // Summary CRUD
        let summary = Summary {
            id: LfdId::new(),
            wave_id: wave.id.clone(),
            content: "# Summary\nCore types and APIs...".to_string(),
            source_hash: "abc123".to_string(),
            token_budget: 10000,
            model: "claude-code".to_string(),
            created_at: Some(OffsetDateTime::now_utc()),
        };
        store.upsert_summary(&summary).unwrap();
        let loaded_summary = store.get_summary(&wave.id).unwrap().unwrap();
        assert_eq!(loaded_summary.content, summary.content);
        assert_eq!(loaded_summary.source_hash, "abc123");

        // Upsert replaces on same wave_id
        let updated_summary = Summary {
            id: LfdId::new(),
            wave_id: wave.id.clone(),
            content: "# Updated summary".to_string(),
            source_hash: "def456".to_string(),
            token_budget: 10000,
            model: "claude-code".to_string(),
            created_at: Some(OffsetDateTime::now_utc()),
        };
        store.upsert_summary(&updated_summary).unwrap();
        let reloaded = store.get_summary(&wave.id).unwrap().unwrap();
        assert_eq!(reloaded.content, "# Updated summary");
        assert_eq!(reloaded.source_hash, "def456");

        store.delete_wave(&wave.id).unwrap();
        assert!(store.get_wave(&wave.id).unwrap().is_none());
    }

    /// get_active_wave_run excludes failed runs (they're not active).
    /// get_latest_wave_run returns the most recent run regardless of status,
    /// so the UI can still display error details for failed waves.
    fn run_active_excludes_failed_latest_includes_suite(store: &dyn RunStore) {
        let wave = make_wave("/repo-fail-test");
        store.create_wave(&wave).unwrap();

        let mut run = WaveRun {
            id: LfdId::new(),
            wave_id: wave.id.clone(),
            snapshot: WaveRunSnapshot {
                repo: wave.repo.clone(),
                flow: wave.flow.clone(),
                direction: wave.direction.clone(),
                area: wave.area.clone(),
                pr: None,
            },
            iteration: 0,
            step_index: 1,
            status: WaveRunStatus::Running,
            worktree: "/repo".to_string(),
            branch: "main".to_string(),
            started_at: Some(OffsetDateTime::now_utc()),
            ended_at: None,
            error: None,
            flow_parents: Vec::new(),
            run_kind: WaveRunKind::Main,
            sidecar_kind: None,
            parent_run_id: None,
            parent_pr_number: None,
            stack_position: 0,
            stack_group_id: wave.id.to_string(),
            stack_status: crate::lfd::types::WaveRunStackStatus::Active,
            lineage_inferred: false,
        };
        store.create_wave_run(&run).unwrap();

        // Should find the running run as active
        let active = store.get_active_wave_run(&wave.id).unwrap();
        assert!(active.is_some(), "should find running run");

        // Mark as failed
        run.status = WaveRunStatus::Failed;
        run.error = Some("step reduce failed".to_string());
        run.ended_at = Some(OffsetDateTime::now_utc());
        store.update_wave_run(&run).unwrap();

        // get_active_wave_run should NOT return failed runs
        let active = store.get_active_wave_run(&wave.id).unwrap();
        assert!(active.is_none(), "failed run should not be active");

        // get_latest_wave_run should still return the failed run
        let latest = store.get_latest_wave_run(&wave.id).unwrap();
        assert!(latest.is_some(), "should find failed run via latest");
        let latest = latest.unwrap();
        assert_eq!(latest.status, WaveRunStatus::Failed);
        assert_eq!(latest.error.as_deref(), Some("step reduce failed"));

        // Sidecar runs should not count as active main runs.
        let sidecar = WaveRun {
            id: LfdId::new(),
            wave_id: wave.id.clone(),
            snapshot: WaveRunSnapshot {
                repo: wave.repo.clone(),
                flow: "debug".to_string(),
                direction: wave.direction.clone(),
                area: wave.area.clone(),
                pr: None,
            },
            iteration: 0,
            step_index: 0,
            status: WaveRunStatus::Running,
            worktree: "/repo.sidecar".to_string(),
            branch: "ci-fix-temp".to_string(),
            started_at: Some(OffsetDateTime::now_utc()),
            ended_at: None,
            error: None,
            flow_parents: Vec::new(),
            run_kind: WaveRunKind::Sidecar,
            sidecar_kind: Some(SidecarKind::CiFix),
            parent_run_id: None,
            parent_pr_number: None,
            stack_position: 0,
            stack_group_id: wave.id.to_string(),
            stack_status: crate::lfd::types::WaveRunStackStatus::Active,
            lineage_inferred: false,
        };
        store.create_wave_run(&sidecar).unwrap();
        let active = store.get_active_wave_run(&wave.id).unwrap();
        assert!(
            active.is_none(),
            "sidecar run should not be returned as active main run"
        );

        store.delete_wave(&wave.id).unwrap();
    }

    #[test]
    fn sqlite_store_suite() {
        let db_path = env::temp_dir().join(format!("lfd-test-{}.db", LfdId::new()));
        let config = super::StorageConfig::sqlite(db_path);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let store = runtime.block_on(super::open_store(&config)).unwrap();

        let shared = store.into_shared();
        run_store_suite(shared.as_ref());
        run_active_excludes_failed_latest_includes_suite(shared.as_ref());
    }

    #[test]
    fn run_active_excludes_failed_latest_includes() {
        let path = env::temp_dir().join(format!("lfd-test-{}.db", LfdId::new()));
        let store = super::sqlite::SqliteStore::new(&path).unwrap();
        run_active_excludes_failed_latest_includes_suite(&store);
    }
}
