use std::path::PathBuf;
use std::sync::Arc;

use crate::lfd::id::LfdId;
use crate::lfd::types::{
    Agent, ChatMemoryBlock, ChatMessage, LivePrState, LivePullRequestState, PendingActivation,
    QueueBlock, QueueMergeEvent, Stimulus, Summary, Wave, WaveRun, WaveRunStackStatus,
};

pub mod catalog;
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
    pub async fn list_waves(&self, repo: Option<&str>) -> StoreResult<Vec<Wave>> {
        WaveStateStore::list_waves(self, repo).await
    }

    pub async fn get_wave(&self, wave_id: &LfdId) -> StoreResult<Option<Wave>> {
        WaveStateStore::get_wave(self, wave_id).await
    }

    pub async fn get_wave_by_name(&self, name: &str) -> StoreResult<Option<Wave>> {
        WaveStateStore::get_wave_by_name(self, name).await
    }

    pub async fn create_wave(&self, wave: &Wave) -> StoreResult<()> {
        WaveStateStore::create_wave(self, wave).await
    }

    pub async fn update_wave(&self, wave: &Wave) -> StoreResult<()> {
        WaveStateStore::update_wave(self, wave).await
    }

    pub async fn delete_wave(&self, wave_id: &LfdId) -> StoreResult<()> {
        WaveStateStore::delete_wave(self, wave_id).await
    }

    pub async fn list_wave_runs(
        &self,
        wave_id: Option<&LfdId>,
        limit: Option<u32>,
    ) -> StoreResult<Vec<WaveRun>> {
        WaveStateStore::list_wave_runs(self, wave_id, limit).await
    }

    pub async fn get_wave_run(&self, wave_run_id: &LfdId) -> StoreResult<Option<WaveRun>> {
        WaveStateStore::get_wave_run(self, wave_run_id).await
    }

    pub async fn get_active_wave_run(&self, wave_id: &LfdId) -> StoreResult<Option<WaveRun>> {
        WaveStateStore::get_active_wave_run(self, wave_id).await
    }

    pub async fn get_latest_wave_run(&self, wave_id: &LfdId) -> StoreResult<Option<WaveRun>> {
        WaveStateStore::get_latest_wave_run(self, wave_id).await
    }

    pub async fn create_wave_run(&self, run: &WaveRun) -> StoreResult<()> {
        WaveStateStore::create_wave_run(self, run).await
    }

    pub async fn update_wave_run(&self, run: &WaveRun) -> StoreResult<()> {
        WaveStateStore::update_wave_run(self, run).await
    }

    pub async fn list_stack_runs(&self, wave_id: &LfdId) -> StoreResult<Vec<WaveRun>> {
        WaveStateStore::list_stack_runs(self, wave_id).await
    }

    pub async fn find_next_unmerged_run(&self, wave_id: &LfdId) -> StoreResult<Option<WaveRun>> {
        WaveStateStore::find_next_unmerged_run(self, wave_id).await
    }

    pub async fn find_descendants(&self, run_id: &LfdId) -> StoreResult<Vec<WaveRun>> {
        WaveStateStore::find_descendants(self, run_id).await
    }

    pub async fn get_live_pr_state(
        &self,
        repo_id: &str,
        pr_number: u32,
    ) -> StoreResult<Option<LivePullRequestState>> {
        WaveStateStore::get_live_pr_state(self, repo_id, pr_number).await
    }

    pub async fn upsert_live_pr_state(&self, state: &LivePullRequestState) -> StoreResult<()> {
        WaveStateStore::upsert_live_pr_state(self, state).await
    }

    pub async fn list_queue_blocks(&self, wave_id: &LfdId) -> StoreResult<Vec<QueueBlock>> {
        WaveStateStore::list_queue_blocks(self, wave_id).await
    }

    pub async fn upsert_queue_block(&self, block: &QueueBlock) -> StoreResult<()> {
        WaveStateStore::upsert_queue_block(self, block).await
    }

    pub async fn delete_queue_block(
        &self,
        wave_id: &LfdId,
        run_id: &LfdId,
    ) -> StoreResult<u32> {
        WaveStateStore::delete_queue_block(self, wave_id, run_id).await
    }

    pub async fn record_merge_event(&self, event: &QueueMergeEvent) -> StoreResult<bool> {
        WaveStateStore::record_merge_event(self, event).await
    }

    pub async fn list_stimuli(&self, wave_id: Option<&LfdId>) -> StoreResult<Vec<Stimulus>> {
        WaveStateStore::list_stimuli(self, wave_id).await
    }

    pub async fn list_stimuli_by_kind(&self, kind: i32) -> StoreResult<Vec<Stimulus>> {
        WaveStateStore::list_stimuli_by_kind(self, kind).await
    }

    pub async fn get_stimulus(&self, stimulus_id: &LfdId) -> StoreResult<Option<Stimulus>> {
        WaveStateStore::get_stimulus(self, stimulus_id).await
    }

    pub async fn create_stimulus(&self, stimulus: &Stimulus) -> StoreResult<()> {
        WaveStateStore::create_stimulus(self, stimulus).await
    }

    pub async fn update_stimulus(&self, stimulus: &Stimulus) -> StoreResult<()> {
        WaveStateStore::update_stimulus(self, stimulus).await
    }

    pub async fn delete_stimulus(&self, stimulus_id: &LfdId) -> StoreResult<()> {
        WaveStateStore::delete_stimulus(self, stimulus_id).await
    }

    pub async fn delete_stimuli_for_wave(&self, wave_id: &LfdId) -> StoreResult<u32> {
        WaveStateStore::delete_stimuli_for_wave(self, wave_id).await
    }

    pub async fn list_pending_activations(
        &self,
        wave_id: &LfdId,
    ) -> StoreResult<Vec<PendingActivation>> {
        WaveStateStore::list_pending_activations(self, wave_id).await
    }

    pub async fn create_pending_activation(
        &self,
        activation: &PendingActivation,
    ) -> StoreResult<()> {
        WaveStateStore::create_pending_activation(self, activation).await
    }

    pub async fn update_pending_activation(
        &self,
        activation: &PendingActivation,
    ) -> StoreResult<()> {
        WaveStateStore::update_pending_activation(self, activation).await
    }

    pub async fn delete_pending_activations(&self, wave_id: &LfdId) -> StoreResult<u32> {
        WaveStateStore::delete_pending_activations(self, wave_id).await
    }

    pub async fn get_pending_for_stimulus(
        &self,
        wave_id: &LfdId,
        stimulus_id: &LfdId,
    ) -> StoreResult<Option<PendingActivation>> {
        WaveStateStore::get_pending_for_stimulus(self, wave_id, stimulus_id).await
    }

    pub async fn get_summary(&self, wave_id: &LfdId) -> StoreResult<Option<Summary>> {
        WaveStateStore::get_summary(self, wave_id).await
    }

    pub async fn upsert_summary(&self, summary: &Summary) -> StoreResult<()> {
        WaveStateStore::upsert_summary(self, summary).await
    }

    pub async fn list_chat_memory_blocks(
        &self,
        wave_id: &LfdId,
    ) -> StoreResult<Vec<ChatMemoryBlock>> {
        WaveStateStore::list_chat_memory_blocks(self, wave_id).await
    }

    pub async fn upsert_chat_memory_block(&self, block: &ChatMemoryBlock) -> StoreResult<()> {
        WaveStateStore::upsert_chat_memory_block(self, block).await
    }

    pub async fn delete_chat_memory_block(&self, wave_id: &LfdId, name: &str) -> StoreResult<()> {
        WaveStateStore::delete_chat_memory_block(self, wave_id, name).await
    }

    pub async fn list_chat_messages(&self, wave_id: &LfdId) -> StoreResult<Vec<ChatMessage>> {
        WaveStateStore::list_chat_messages(self, wave_id).await
    }

    pub async fn create_chat_message(&self, message: &ChatMessage) -> StoreResult<()> {
        WaveStateStore::create_chat_message(self, message).await
    }

    pub async fn list_fork_runs(
        &self,
        wave_run_id: &LfdId,
        step_index: u32,
    ) -> StoreResult<Vec<ForkRun>> {
        ExecutionStore::list_fork_runs(self, wave_run_id, step_index).await
    }

    pub async fn upsert_fork_run(&self, fork_run: &ForkRun) -> StoreResult<()> {
        ExecutionStore::upsert_fork_run(self, fork_run).await
    }

    pub async fn delete_fork_runs(&self, wave_run_id: &LfdId, step_index: u32) -> StoreResult<u32> {
        ExecutionStore::delete_fork_runs(self, wave_run_id, step_index).await
    }

    pub async fn list_agents(&self) -> StoreResult<Vec<Agent>> {
        ExecutionStore::list_agents(self).await
    }

    pub async fn list_agent_history(
        &self,
        worktree: Option<&str>,
        repo: Option<&str>,
        limit: Option<u32>,
    ) -> StoreResult<Vec<Agent>> {
        ExecutionStore::list_agent_history(self, worktree, repo, limit).await
    }

    pub async fn get_agent(&self, agent_id: &LfdId) -> StoreResult<Option<Agent>> {
        ExecutionStore::get_agent(self, agent_id).await
    }

    pub async fn get_waiting_agent_for_wave(&self, wave_id: &LfdId) -> StoreResult<Option<Agent>> {
        ExecutionStore::get_waiting_agent_for_wave(self, wave_id).await
    }

    pub async fn start_agent(&self, agent: &Agent) -> StoreResult<()> {
        ExecutionStore::start_agent(self, agent).await
    }

    pub async fn update_agent_status(
        &self,
        agent_id: &LfdId,
        status: i32,
        pid: Option<u32>,
        container_id: Option<&str>,
    ) -> StoreResult<()> {
        ExecutionStore::update_agent_status(self, agent_id, status, pid, container_id).await
    }

    pub async fn end_agent(&self, agent_id: &LfdId, status: i32, ended_at: i64) -> StoreResult<()> {
        ExecutionStore::end_agent(self, agent_id, status, ended_at).await
    }

    pub async fn get_active_agents_for_wave(&self, wave_id: &LfdId) -> StoreResult<Vec<Agent>> {
        ExecutionStore::get_active_agents_for_wave(self, wave_id).await
    }

    pub async fn end_active_agent_for_wave(
        &self,
        wave_id: &LfdId,
        status: i32,
        ended_at: i64,
    ) -> StoreResult<()> {
        ExecutionStore::end_active_agent_for_wave(self, wave_id, status, ended_at).await
    }

    pub async fn get_stuck_agents(&self, older_than_secs: u64) -> StoreResult<Vec<Agent>> {
        ExecutionStore::get_stuck_agents(self, older_than_secs).await
    }

    pub async fn fail_orphaned_runs(&self) -> StoreResult<u32> {
        ExecutionStore::fail_orphaned_runs(self).await
    }

    pub async fn health_check(&self) -> StoreResult<()> {
        StoreAdmin::health_check(self).await
    }

    pub async fn schema_version(&self) -> StoreResult<String> {
        StoreAdmin::schema_version(self).await
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
    async fn list_queue_blocks(&self, wave_id: &LfdId) -> StoreResult<Vec<QueueBlock>>;
    async fn upsert_queue_block(&self, block: &QueueBlock) -> StoreResult<()>;
    async fn delete_queue_block(&self, wave_id: &LfdId, run_id: &LfdId) -> StoreResult<u32>;
    async fn record_merge_event(&self, event: &QueueMergeEvent) -> StoreResult<bool>;

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

    async fn list_chat_memory_blocks(&self, wave_id: &LfdId) -> StoreResult<Vec<ChatMemoryBlock>>;
    async fn upsert_chat_memory_block(&self, block: &ChatMemoryBlock) -> StoreResult<()>;
    async fn delete_chat_memory_block(&self, wave_id: &LfdId, name: &str) -> StoreResult<()>;

    async fn list_chat_messages(&self, wave_id: &LfdId) -> StoreResult<Vec<ChatMessage>>;
    async fn create_chat_message(&self, message: &ChatMessage) -> StoreResult<()>;
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
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let repo = repo.map(str::to_string);
                run_sqlite(store, move |store| store.list_waves(repo.as_deref())).await
            }
            StoreBackend::Postgres(store) => store.list_waves(repo).await,
        }
    }

    async fn get_wave(&self, wave_id: &LfdId) -> StoreResult<Option<Wave>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let wave_id = wave_id.clone();
                run_sqlite(store, move |store| store.get_wave(&wave_id)).await
            }
            StoreBackend::Postgres(store) => store.get_wave(wave_id).await,
        }
    }

    async fn get_wave_by_name(&self, name: &str) -> StoreResult<Option<Wave>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let name = name.to_string();
                run_sqlite(store, move |store| store.get_wave_by_name(&name)).await
            }
            StoreBackend::Postgres(store) => store.get_wave_by_name(name).await,
        }
    }

    async fn create_wave(&self, wave: &Wave) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let wave = wave.clone();
                run_sqlite(store, move |store| store.create_wave(&wave)).await
            }
            StoreBackend::Postgres(store) => store.create_wave(wave).await,
        }
    }

    async fn update_wave(&self, wave: &Wave) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let wave = wave.clone();
                run_sqlite(store, move |store| store.update_wave(&wave)).await
            }
            StoreBackend::Postgres(store) => store.update_wave(wave).await,
        }
    }

    async fn delete_wave(&self, wave_id: &LfdId) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let wave_id = wave_id.clone();
                run_sqlite(store, move |store| store.delete_wave(&wave_id)).await
            }
            StoreBackend::Postgres(store) => store.delete_wave(wave_id).await,
        }
    }

    async fn list_wave_runs(
        &self,
        wave_id: Option<&LfdId>,
        limit: Option<u32>,
    ) -> StoreResult<Vec<WaveRun>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let wave_id = wave_id.cloned();
                run_sqlite(store, move |store| {
                    store.list_wave_runs(wave_id.as_ref(), limit)
                })
                .await
            }
            StoreBackend::Postgres(store) => store.list_wave_runs(wave_id, limit).await,
        }
    }

    async fn get_wave_run(&self, wave_run_id: &LfdId) -> StoreResult<Option<WaveRun>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let wave_run_id = wave_run_id.clone();
                run_sqlite(store, move |store| store.get_wave_run(&wave_run_id)).await
            }
            StoreBackend::Postgres(store) => store.get_wave_run(wave_run_id).await,
        }
    }

    async fn get_active_wave_run(&self, wave_id: &LfdId) -> StoreResult<Option<WaveRun>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let wave_id = wave_id.clone();
                run_sqlite(store, move |store| store.get_active_wave_run(&wave_id)).await
            }
            StoreBackend::Postgres(store) => store.get_active_wave_run(wave_id).await,
        }
    }

    async fn get_latest_wave_run(&self, wave_id: &LfdId) -> StoreResult<Option<WaveRun>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let wave_id = wave_id.clone();
                run_sqlite(store, move |store| store.get_latest_wave_run(&wave_id)).await
            }
            StoreBackend::Postgres(store) => store.get_latest_wave_run(wave_id).await,
        }
    }

    async fn create_wave_run(&self, run: &WaveRun) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let run = run.clone();
                run_sqlite(store, move |store| store.create_wave_run(&run)).await
            }
            StoreBackend::Postgres(store) => store.create_wave_run(run).await,
        }
    }

    async fn update_wave_run(&self, run: &WaveRun) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let run = run.clone();
                run_sqlite(store, move |store| store.update_wave_run(&run)).await
            }
            StoreBackend::Postgres(store) => store.update_wave_run(run).await,
        }
    }

    async fn list_stack_runs(&self, wave_id: &LfdId) -> StoreResult<Vec<WaveRun>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let wave_id = wave_id.clone();
                run_sqlite(store, move |store| store.list_stack_runs(&wave_id)).await
            }
            StoreBackend::Postgres(store) => store.list_stack_runs(wave_id).await,
        }
    }

    async fn find_next_unmerged_run(&self, wave_id: &LfdId) -> StoreResult<Option<WaveRun>> {
        let runs = self.list_stack_runs(wave_id).await?;
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

            let Some(live_state) = self
                .get_live_pr_state(&run.snapshot.repo, pr_number)
                .await?
            else {
                return Ok(Some(run));
            };
            if live_state.state != LivePrState::Merged {
                return Ok(Some(run));
            }
        }
        Ok(None)
    }

    async fn find_descendants(&self, run_id: &LfdId) -> StoreResult<Vec<WaveRun>> {
        let Some(parent) = self.get_wave_run(run_id).await? else {
            return Ok(Vec::new());
        };
        let descendants = self
            .list_stack_runs(&parent.wave_id)
            .await?
            .into_iter()
            .filter(|run| {
                run.stack_group_id == parent.stack_group_id
                    && run.stack_position > parent.stack_position
            })
            .collect();
        Ok(descendants)
    }

    async fn get_live_pr_state(
        &self,
        repo_id: &str,
        pr_number: u32,
    ) -> StoreResult<Option<LivePullRequestState>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let repo_id = repo_id.to_string();
                run_sqlite(store, move |store| {
                    store.get_live_pr_state(&repo_id, pr_number)
                })
                .await
            }
            StoreBackend::Postgres(store) => store.get_live_pr_state(repo_id, pr_number).await,
        }
    }

    async fn upsert_live_pr_state(&self, state: &LivePullRequestState) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let state = state.clone();
                run_sqlite(store, move |store| store.upsert_live_pr_state(&state)).await
            }
            StoreBackend::Postgres(store) => store.upsert_live_pr_state(state).await,
        }
    }

    async fn list_queue_blocks(&self, wave_id: &LfdId) -> StoreResult<Vec<QueueBlock>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let wave_id = wave_id.clone();
                run_sqlite(store, move |store| store.list_queue_blocks(&wave_id)).await
            }
            StoreBackend::Postgres(store) => store.list_queue_blocks(wave_id).await,
        }
    }

    async fn upsert_queue_block(&self, block: &QueueBlock) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let block = block.clone();
                run_sqlite(store, move |store| store.upsert_queue_block(&block)).await
            }
            StoreBackend::Postgres(store) => store.upsert_queue_block(block).await,
        }
    }

    async fn delete_queue_block(&self, wave_id: &LfdId, run_id: &LfdId) -> StoreResult<u32> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let wave_id = wave_id.clone();
                let run_id = run_id.clone();
                run_sqlite(store, move |store| {
                    store.delete_queue_block(&wave_id, &run_id)
                })
                .await
            }
            StoreBackend::Postgres(store) => store.delete_queue_block(wave_id, run_id).await,
        }
    }

    async fn record_merge_event(&self, event: &QueueMergeEvent) -> StoreResult<bool> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let event = event.clone();
                run_sqlite(store, move |store| store.record_merge_event(&event)).await
            }
            StoreBackend::Postgres(store) => store.record_merge_event(event).await,
        }
    }

    async fn list_stimuli(&self, wave_id: Option<&LfdId>) -> StoreResult<Vec<Stimulus>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let wave_id = wave_id.cloned();
                run_sqlite(store, move |store| store.list_stimuli(wave_id.as_ref())).await
            }
            StoreBackend::Postgres(store) => store.list_stimuli(wave_id).await,
        }
    }

    async fn list_stimuli_by_kind(&self, kind: i32) -> StoreResult<Vec<Stimulus>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                run_sqlite(store, move |store| store.list_stimuli_by_kind(kind)).await
            }
            StoreBackend::Postgres(store) => store.list_stimuli_by_kind(kind).await,
        }
    }

    async fn get_stimulus(&self, stimulus_id: &LfdId) -> StoreResult<Option<Stimulus>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let stimulus_id = stimulus_id.clone();
                run_sqlite(store, move |store| store.get_stimulus(&stimulus_id)).await
            }
            StoreBackend::Postgres(store) => store.get_stimulus(stimulus_id).await,
        }
    }

    async fn create_stimulus(&self, stimulus: &Stimulus) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let stimulus = stimulus.clone();
                run_sqlite(store, move |store| store.create_stimulus(&stimulus)).await
            }
            StoreBackend::Postgres(store) => store.create_stimulus(stimulus).await,
        }
    }

    async fn update_stimulus(&self, stimulus: &Stimulus) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let stimulus = stimulus.clone();
                run_sqlite(store, move |store| store.update_stimulus(&stimulus)).await
            }
            StoreBackend::Postgres(store) => store.update_stimulus(stimulus).await,
        }
    }

    async fn delete_stimulus(&self, stimulus_id: &LfdId) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let stimulus_id = stimulus_id.clone();
                run_sqlite(store, move |store| store.delete_stimulus(&stimulus_id)).await
            }
            StoreBackend::Postgres(store) => store.delete_stimulus(stimulus_id).await,
        }
    }

    async fn delete_stimuli_for_wave(&self, wave_id: &LfdId) -> StoreResult<u32> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let wave_id = wave_id.clone();
                run_sqlite(store, move |store| store.delete_stimuli_for_wave(&wave_id)).await
            }
            StoreBackend::Postgres(store) => store.delete_stimuli_for_wave(wave_id).await,
        }
    }

    async fn list_pending_activations(
        &self,
        wave_id: &LfdId,
    ) -> StoreResult<Vec<PendingActivation>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let wave_id = wave_id.clone();
                run_sqlite(store, move |store| store.list_pending_activations(&wave_id)).await
            }
            StoreBackend::Postgres(store) => store.list_pending_activations(wave_id).await,
        }
    }

    async fn create_pending_activation(&self, activation: &PendingActivation) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let activation = activation.clone();
                run_sqlite(store, move |store| {
                    store.create_pending_activation(&activation)
                })
                .await
            }
            StoreBackend::Postgres(store) => store.create_pending_activation(activation).await,
        }
    }

    async fn update_pending_activation(&self, activation: &PendingActivation) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let activation = activation.clone();
                run_sqlite(store, move |store| {
                    store.update_pending_activation(&activation)
                })
                .await
            }
            StoreBackend::Postgres(store) => store.update_pending_activation(activation).await,
        }
    }

    async fn delete_pending_activations(&self, wave_id: &LfdId) -> StoreResult<u32> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let wave_id = wave_id.clone();
                run_sqlite(store, move |store| {
                    store.delete_pending_activations(&wave_id)
                })
                .await
            }
            StoreBackend::Postgres(store) => store.delete_pending_activations(wave_id).await,
        }
    }

    async fn get_pending_for_stimulus(
        &self,
        wave_id: &LfdId,
        stimulus_id: &LfdId,
    ) -> StoreResult<Option<PendingActivation>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let wave_id = wave_id.clone();
                let stimulus_id = stimulus_id.clone();
                run_sqlite(store, move |store| {
                    store.get_pending_for_stimulus(&wave_id, &stimulus_id)
                })
                .await
            }
            StoreBackend::Postgres(store) => {
                store.get_pending_for_stimulus(wave_id, stimulus_id).await
            }
        }
    }

    async fn get_summary(&self, wave_id: &LfdId) -> StoreResult<Option<Summary>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let wave_id = wave_id.clone();
                run_sqlite(store, move |store| store.get_summary(&wave_id)).await
            }
            StoreBackend::Postgres(store) => store.get_summary(wave_id).await,
        }
    }

    async fn upsert_summary(&self, summary: &Summary) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let summary = summary.clone();
                run_sqlite(store, move |store| store.upsert_summary(&summary)).await
            }
            StoreBackend::Postgres(store) => store.upsert_summary(summary).await,
        }
    }

    async fn list_chat_memory_blocks(&self, wave_id: &LfdId) -> StoreResult<Vec<ChatMemoryBlock>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let wave_id = wave_id.clone();
                run_sqlite(store, move |store| store.list_chat_memory_blocks(&wave_id)).await
            }
            StoreBackend::Postgres(store) => store.list_chat_memory_blocks(wave_id).await,
        }
    }

    async fn upsert_chat_memory_block(&self, block: &ChatMemoryBlock) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let block = block.clone();
                run_sqlite(store, move |store| store.upsert_chat_memory_block(&block)).await
            }
            StoreBackend::Postgres(store) => store.upsert_chat_memory_block(block).await,
        }
    }

    async fn delete_chat_memory_block(&self, wave_id: &LfdId, name: &str) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let wave_id = wave_id.clone();
                let name = name.to_string();
                run_sqlite(store, move |store| {
                    store.delete_chat_memory_block(&wave_id, &name)
                })
                .await
            }
            StoreBackend::Postgres(store) => store.delete_chat_memory_block(wave_id, name).await,
        }
    }

    async fn list_chat_messages(&self, wave_id: &LfdId) -> StoreResult<Vec<ChatMessage>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let wave_id = wave_id.clone();
                run_sqlite(store, move |store| store.list_chat_messages(&wave_id)).await
            }
            StoreBackend::Postgres(store) => store.list_chat_messages(wave_id).await,
        }
    }

    async fn create_chat_message(&self, message: &ChatMessage) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let message = message.clone();
                run_sqlite(store, move |store| store.create_chat_message(&message)).await
            }
            StoreBackend::Postgres(store) => store.create_chat_message(message).await,
        }
    }
}

#[async_trait::async_trait]
impl ExecutionStore for Store {
    async fn list_fork_runs(
        &self,
        wave_run_id: &LfdId,
        step_index: u32,
    ) -> StoreResult<Vec<ForkRun>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let wave_run_id = wave_run_id.clone();
                run_sqlite(store, move |store| {
                    store.list_fork_runs(&wave_run_id, step_index)
                })
                .await
            }
            StoreBackend::Postgres(store) => store.list_fork_runs(wave_run_id, step_index).await,
        }
    }

    async fn upsert_fork_run(&self, fork_run: &ForkRun) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let fork_run = fork_run.clone();
                run_sqlite(store, move |store| store.upsert_fork_run(&fork_run)).await
            }
            StoreBackend::Postgres(store) => store.upsert_fork_run(fork_run).await,
        }
    }

    async fn delete_fork_runs(&self, wave_run_id: &LfdId, step_index: u32) -> StoreResult<u32> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let wave_run_id = wave_run_id.clone();
                run_sqlite(store, move |store| {
                    store.delete_fork_runs(&wave_run_id, step_index)
                })
                .await
            }
            StoreBackend::Postgres(store) => store.delete_fork_runs(wave_run_id, step_index).await,
        }
    }

    async fn list_agents(&self) -> StoreResult<Vec<Agent>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => run_sqlite(store, |store| store.list_agents()).await,
            StoreBackend::Postgres(store) => store.list_agents().await,
        }
    }

    async fn list_agent_history(
        &self,
        worktree: Option<&str>,
        repo: Option<&str>,
        limit: Option<u32>,
    ) -> StoreResult<Vec<Agent>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let worktree = worktree.map(str::to_string);
                let repo = repo.map(str::to_string);
                run_sqlite(store, move |store| {
                    store.list_agent_history(worktree.as_deref(), repo.as_deref(), limit)
                })
                .await
            }
            StoreBackend::Postgres(store) => store.list_agent_history(worktree, repo, limit).await,
        }
    }

    async fn get_agent(&self, agent_id: &LfdId) -> StoreResult<Option<Agent>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let agent_id = agent_id.clone();
                run_sqlite(store, move |store| store.get_agent(&agent_id)).await
            }
            StoreBackend::Postgres(store) => store.get_agent(agent_id).await,
        }
    }

    async fn get_waiting_agent_for_wave(&self, wave_id: &LfdId) -> StoreResult<Option<Agent>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let wave_id = wave_id.clone();
                run_sqlite(store, move |store| {
                    store.get_waiting_agent_for_wave(&wave_id)
                })
                .await
            }
            StoreBackend::Postgres(store) => store.get_waiting_agent_for_wave(wave_id).await,
        }
    }

    async fn start_agent(&self, agent: &Agent) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let agent = agent.clone();
                run_sqlite(store, move |store| store.start_agent(&agent)).await
            }
            StoreBackend::Postgres(store) => store.start_agent(agent).await,
        }
    }

    async fn update_agent_status(
        &self,
        agent_id: &LfdId,
        status: i32,
        pid: Option<u32>,
        container_id: Option<&str>,
    ) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let agent_id = agent_id.clone();
                let container_id = container_id.map(str::to_string);
                run_sqlite(store, move |store| {
                    store.update_agent_status(&agent_id, status, pid, container_id.as_deref())
                })
                .await
            }
            StoreBackend::Postgres(store) => {
                store
                    .update_agent_status(agent_id, status, pid, container_id)
                    .await
            }
        }
    }

    async fn end_agent(&self, agent_id: &LfdId, status: i32, ended_at: i64) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let agent_id = agent_id.clone();
                run_sqlite(store, move |store| {
                    store.end_agent(&agent_id, status, ended_at)
                })
                .await
            }
            StoreBackend::Postgres(store) => store.end_agent(agent_id, status, ended_at).await,
        }
    }

    async fn get_active_agents_for_wave(&self, wave_id: &LfdId) -> StoreResult<Vec<Agent>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let wave_id = wave_id.clone();
                run_sqlite(store, move |store| {
                    store.get_active_agents_for_wave(&wave_id)
                })
                .await
            }
            StoreBackend::Postgres(store) => store.get_active_agents_for_wave(wave_id).await,
        }
    }

    async fn end_active_agent_for_wave(
        &self,
        wave_id: &LfdId,
        status: i32,
        ended_at: i64,
    ) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let wave_id = wave_id.clone();
                run_sqlite(store, move |store| {
                    store.end_active_agent_for_wave(&wave_id, status, ended_at)
                })
                .await
            }
            StoreBackend::Postgres(store) => {
                store
                    .end_active_agent_for_wave(wave_id, status, ended_at)
                    .await
            }
        }
    }

    async fn get_stuck_agents(&self, older_than_secs: u64) -> StoreResult<Vec<Agent>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                run_sqlite(store, move |store| store.get_stuck_agents(older_than_secs)).await
            }
            StoreBackend::Postgres(store) => store.get_stuck_agents(older_than_secs).await,
        }
    }

    async fn fail_orphaned_runs(&self) -> StoreResult<u32> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                run_sqlite(store, |store| store.fail_orphaned_runs()).await
            }
            StoreBackend::Postgres(store) => store.fail_orphaned_runs().await,
        }
    }
}

#[async_trait::async_trait]
impl StoreAdmin for Store {
    async fn health_check(&self) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => run_sqlite(store, |store| store.health_check()).await,
            StoreBackend::Postgres(store) => store.health_check().await,
        }
    }

    async fn schema_version(&self) -> StoreResult<String> {
        match &self.backend {
            StoreBackend::Sqlite(store) => run_sqlite(store, |store| store.schema_version()).await,
            StoreBackend::Postgres(store) => store.schema_version().await,
        }
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
pub type SharedStore = Arc<Store>;

#[cfg(test)]
mod tests {
    use super::{ForkRun, ForkRunStatus, StorageConfig};
    use crate::lfd::id::LfdId;
    use crate::lfd::types::{
        Agent, AgentStatus, ChatMemoryBlock, LivePrState, LivePullRequestState, PendingActivation,
        PullRequest, QueueBlock, QueueMergeEvent, SidecarKind, Stimulus, StimulusKind, Summary,
        Wave, WaveRun, WaveRunKind, WaveRunSnapshot, WaveRunStackStatus, WaveRunStatus, WaveStatus,
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
            schema_ref: None,
            schema_name: None,
            created_at: Some(OffsetDateTime::now_utc()),
        }
    }

    fn make_run(wave: &Wave, status: WaveRunStatus, kind: WaveRunKind) -> WaveRun {
        WaveRun {
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
            step_index: 0,
            status,
            worktree: "/repo".to_string(),
            branch: "main".to_string(),
            started_at: Some(OffsetDateTime::now_utc()),
            ended_at: None,
            error: None,
            flow_parents: Vec::new(),
            run_kind: kind,
            sidecar_kind: if kind == WaveRunKind::Sidecar {
                Some(SidecarKind::CiFix)
            } else {
                None
            },
            parent_run_id: None,
            parent_pr_number: None,
            stack_position: 0,
            stack_group_id: wave.id.to_string(),
            stack_status: WaveRunStackStatus::Active,
            lineage_inferred: false,
        }
    }

    #[tokio::test]
    async fn sqlite_store_basic_suite() {
        let db_path = env::temp_dir().join(format!("lfd-test-{}.db", LfdId::new()));
        let config = StorageConfig::sqlite(db_path);
        let store = super::open_store(&config).await.expect("store should open");

        let mut wave = make_wave("/repo");
        store.create_wave(&wave).await.expect("create wave");
        assert!(store.get_wave(&wave.id).await.expect("get wave").is_some());

        wave.status = WaveStatus::Paused;
        store.update_wave(&wave).await.expect("update wave");
        let loaded = store
            .get_wave(&wave.id)
            .await
            .expect("get wave")
            .expect("wave exists");
        assert_eq!(loaded.status, WaveStatus::Paused);

        let stimulus = Stimulus {
            id: LfdId::new(),
            wave_id: wave.id.clone(),
            kind: StimulusKind::Watch,
            cron: "".to_string(),
            last_main_sha: None,
            last_triggered_at: None,
            created_at: Some(OffsetDateTime::now_utc()),
            enabled: true,
        };
        store
            .create_stimulus(&stimulus)
            .await
            .expect("create stimulus");
        assert_eq!(
            store
                .list_stimuli(Some(&wave.id))
                .await
                .expect("list stimuli")
                .len(),
            1
        );

        let run = make_run(&wave, WaveRunStatus::Running, WaveRunKind::Main);
        store.create_wave_run(&run).await.expect("create wave run");
        assert!(store
            .get_active_wave_run(&wave.id)
            .await
            .expect("get active")
            .is_some());

        let fork_run = ForkRun {
            id: LfdId::new(),
            wave_run_id: run.id.clone(),
            step_index: 0,
            branch_index: 0,
            status: ForkRunStatus::Pending,
            worktree: "/tmp/branch".to_string(),
        };
        store
            .upsert_fork_run(&fork_run)
            .await
            .expect("upsert fork run");
        assert_eq!(
            store
                .list_fork_runs(&run.id, 0)
                .await
                .expect("list fork runs")
                .len(),
            1
        );

        let agent = Agent {
            id: LfdId::new(),
            step: "plan".to_string(),
            repo: "/repo".to_string(),
            worktree: "/repo".to_string(),
            wave_run_id: Some(run.id.clone()),
            status: AgentStatus::Waiting,
            started_at: Some(OffsetDateTime::now_utc()),
            ended_at: None,
            pid: None,
            container_id: None,
            model: "claude-code".to_string(),
            run_mode: "auto".to_string(),
        };
        store.start_agent(&agent).await.expect("start agent");
        assert!(store
            .get_waiting_agent_for_wave(&wave.id)
            .await
            .expect("get waiting")
            .is_some());

        let summary = Summary {
            id: LfdId::new(),
            wave_id: wave.id.clone(),
            content: "summary".to_string(),
            source_hash: "abc".to_string(),
            token_budget: 100,
            model: "claude-code".to_string(),
            created_at: Some(OffsetDateTime::now_utc()),
        };
        store
            .upsert_summary(&summary)
            .await
            .expect("upsert summary");
        assert!(store
            .get_summary(&wave.id)
            .await
            .expect("get summary")
            .is_some());

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
        store
            .upsert_summary(&updated_summary)
            .await
            .expect("upsert updated summary");
        let reloaded = store
            .get_summary(&wave.id)
            .await
            .expect("get updated summary")
            .expect("summary should exist");
        assert_eq!(reloaded.content, "# Updated summary");
        assert_eq!(reloaded.source_hash, "def456");

        // Chat memory block CRUD
        let block_a = ChatMemoryBlock {
            wave_id: wave.id.clone(),
            name: "preferences".to_string(),
            content: "Keep responses concise.".to_string(),
            position: 1,
            updated_at: Some(OffsetDateTime::now_utc()),
        };
        let block_b = ChatMemoryBlock {
            wave_id: wave.id.clone(),
            name: "project-context".to_string(),
            content: "Repo uses Rust + Swift.".to_string(),
            position: 0,
            updated_at: Some(OffsetDateTime::now_utc()),
        };
        store
            .upsert_chat_memory_block(&block_a)
            .await
            .expect("upsert block a");
        store
            .upsert_chat_memory_block(&block_b)
            .await
            .expect("upsert block b");
        let blocks = store
            .list_chat_memory_blocks(&wave.id)
            .await
            .expect("list blocks");
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].name, "project-context");
        assert_eq!(blocks[1].name, "preferences");

        let block_a_updated = ChatMemoryBlock {
            content: "Prefer bullet points.".to_string(),
            position: 2,
            ..block_a
        };
        store
            .upsert_chat_memory_block(&block_a_updated)
            .await
            .expect("upsert updated block");
        let blocks = store
            .list_chat_memory_blocks(&wave.id)
            .await
            .expect("list blocks after update");
        let updated = blocks
            .iter()
            .find(|block| block.name == "preferences")
            .expect("updated memory block should exist");
        assert_eq!(updated.content, "Prefer bullet points.");
        assert_eq!(updated.position, 2);

        store
            .delete_chat_memory_block(&wave.id, "project-context")
            .await
            .expect("delete block");
        let blocks = store
            .list_chat_memory_blocks(&wave.id)
            .await
            .expect("list blocks after delete");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].name, "preferences");

        store.delete_wave(&wave.id).await.expect("delete wave");
        assert!(store
            .get_wave(&wave.id)
            .await
            .expect("get deleted wave")
            .is_none());
    }

    #[tokio::test]
    async fn active_run_excludes_failed_and_sidecar() {
        let db_path = env::temp_dir().join(format!("lfd-test-{}.db", LfdId::new()));
        let config = StorageConfig::sqlite(db_path);
        let store = super::open_store(&config).await.expect("store should open");

        let wave = make_wave("/repo-active");
        store.create_wave(&wave).await.expect("create wave");

        let mut run = make_run(&wave, WaveRunStatus::Running, WaveRunKind::Main);
        store
            .create_wave_run(&run)
            .await
            .expect("create running run");
        assert!(store
            .get_active_wave_run(&wave.id)
            .await
            .expect("active run")
            .is_some());

        run.status = WaveRunStatus::Failed;
        run.error = Some("failed".to_string());
        run.ended_at = Some(OffsetDateTime::now_utc());
        store.update_wave_run(&run).await.expect("update run");
        assert!(store
            .get_active_wave_run(&wave.id)
            .await
            .expect("active after fail")
            .is_none());
        assert_eq!(
            store
                .get_latest_wave_run(&wave.id)
                .await
                .expect("latest run")
                .expect("run exists")
                .status,
            WaveRunStatus::Failed
        );

        let sidecar = make_run(&wave, WaveRunStatus::Running, WaveRunKind::Sidecar);
        store
            .create_wave_run(&sidecar)
            .await
            .expect("create sidecar run");
        assert!(store
            .get_active_wave_run(&wave.id)
            .await
            .expect("active with sidecar")
            .is_none());

        store.delete_wave(&wave.id).await.expect("delete wave");
    }

    #[tokio::test]
    async fn find_next_unmerged_transitions() {
        let db_path = env::temp_dir().join(format!("lfd-test-{}.db", LfdId::new()));
        let config = StorageConfig::sqlite(db_path);
        let store = super::open_store(&config).await.expect("store should open");

        let wave = make_wave("/repo-live-pr-transitions");
        store.create_wave(&wave).await.expect("create wave");

        let make_run =
            |iteration: u32, parent_run_id: Option<LfdId>, parent_pr_number: Option<u32>| {
                let pr_number = 100 + iteration;
                WaveRun {
                    id: LfdId::new(),
                    wave_id: wave.id.clone(),
                    snapshot: WaveRunSnapshot {
                        repo: wave.repo.clone(),
                        flow: wave.flow.clone(),
                        direction: wave.direction.clone(),
                        area: wave.area.clone(),
                        pr: Some(PullRequest {
                            url: format!("https://example.test/pr/{pr_number}"),
                            number: Some(pr_number),
                            state: Some("open".to_string()),
                            title: Some(format!("run-{iteration}")),
                            branch: Some(format!("feature-{pr_number}")),
                        }),
                    },
                    iteration,
                    step_index: 0,
                    status: WaveRunStatus::Completed,
                    worktree: format!("/repo/live-pr/{iteration}"),
                    branch: format!("feature-{pr_number}"),
                    started_at: Some(OffsetDateTime::now_utc()),
                    ended_at: Some(OffsetDateTime::now_utc()),
                    error: None,
                    flow_parents: Vec::new(),
                    run_kind: WaveRunKind::Main,
                    sidecar_kind: None,
                    parent_run_id,
                    parent_pr_number,
                    stack_position: iteration.saturating_sub(1),
                    stack_group_id: wave.id.to_string(),
                    stack_status: WaveRunStackStatus::Active,
                    lineage_inferred: false,
                }
            };

        let make_pr_state =
            |pr_number: u32, state: LivePrState, head_sha: &str| LivePullRequestState {
                repo_id: wave.repo.clone(),
                pr_number,
                state,
                is_draft: false,
                head_ref: format!("feature-{pr_number}"),
                head_sha: head_sha.to_string(),
                base_ref: "main".to_string(),
                updated_at: OffsetDateTime::now_utc(),
                merged_at: (state == LivePrState::Merged).then(OffsetDateTime::now_utc),
                synced_at: OffsetDateTime::now_utc(),
            };

        let run1 = make_run(1, None, None);
        store
            .create_wave_run(&run1)
            .await
            .expect("create run1");

        let run2 = make_run(2, Some(run1.id.clone()), Some(101));
        store
            .create_wave_run(&run2)
            .await
            .expect("create run2");

        let first_pending = store
            .find_next_unmerged_run(&wave.id)
            .await
            .expect("find first unmerged");
        assert_eq!(first_pending.map(|run| run.id), Some(run1.id.clone()));

        store
            .upsert_live_pr_state(&make_pr_state(101, LivePrState::Merged, "sha-101"))
            .await
            .expect("upsert pr 101 merged");
        let second_pending = store
            .find_next_unmerged_run(&wave.id)
            .await
            .expect("find second unmerged");
        assert_eq!(second_pending.map(|run| run.id), Some(run2.id.clone()));

        for (state, sha, expected) in [
            (LivePrState::Closed, "sha-102", Some(run2.id.clone())),
            (LivePrState::Unknown, "sha-102b", Some(run2.id.clone())),
            (LivePrState::Merged, "sha-102c", None),
        ] {
            store
                .upsert_live_pr_state(&make_pr_state(102, state, sha))
                .await
                .expect("upsert pr state");
            let pending = store
                .find_next_unmerged_run(&wave.id)
                .await
                .expect("find unmerged");
            assert_eq!(pending.map(|run| run.id), expected);
        }

        store.delete_wave(&wave.id).await.expect("delete wave");
    }

    #[tokio::test]
    async fn queue_block_and_merge_event_crud() {
        let db_path = env::temp_dir().join(format!("lfd-test-{}.db", LfdId::new()));
        let config = StorageConfig::sqlite(db_path);
        let store = super::open_store(&config).await.expect("store should open");

        let wave = make_wave("/repo-queue");
        store.create_wave(&wave).await.expect("create wave");

        let mut run = make_run(&wave, WaveRunStatus::Completed, WaveRunKind::Main);
        run.snapshot.pr = Some(PullRequest {
            url: "https://example.test/pr/42".to_string(),
            number: Some(42),
            state: Some("open".to_string()),
            title: Some("feature".to_string()),
            branch: Some("feature-42".to_string()),
        });
        run.parent_pr_number = Some(42);
        store.create_wave_run(&run).await.expect("create run");

        let block = QueueBlock {
            wave_id: wave.id.clone(),
            run_id: run.id.clone(),
            reason: "rebase_conflict".to_string(),
            attempted_at: OffsetDateTime::now_utc(),
            conflict_files: vec!["src/lib.rs".to_string()],
            error: Some("merge failed".to_string()),
        };
        store
            .upsert_queue_block(&block)
            .await
            .expect("upsert block");
        let blocks = store
            .list_queue_blocks(&wave.id)
            .await
            .expect("list blocks");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].reason, "rebase_conflict");
        let deleted = store
            .delete_queue_block(&wave.id, &run.id)
            .await
            .expect("delete block");
        assert_eq!(deleted, 1);

        let merge_event = QueueMergeEvent {
            wave_id: wave.id.clone(),
            pr_number: 42,
            merged_at: OffsetDateTime::now_utc(),
            processed_at: OffsetDateTime::now_utc(),
        };
        let first = store
            .record_merge_event(&merge_event)
            .await
            .expect("record first");
        let second = store
            .record_merge_event(&merge_event)
            .await
            .expect("record second");
        assert!(first);
        assert!(!second);

        store.delete_wave(&wave.id).await.expect("delete wave");
    }
}
