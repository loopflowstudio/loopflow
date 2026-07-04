use std::path::PathBuf;
use std::sync::Arc;

use crate::lfd::id::LfdId;
use crate::lfd::types::{
    ActivationLog, AttentionItem, AttentionKind, AttentionStatus, ChatMemoryBlock, ChatMessage,
    ExecutionProcess, LivePrState, LivePullRequestState, PendingActivation, QueueBlock,
    QueueMergeEvent, Repo, RepoEdge, RepoId, RepoWork, Run, RunStackStatus, Session, SessionStatus,
    Summary, Trigger, Wave, WaveCron,
};

pub mod catalog;
pub mod migrations;
pub mod postgres;
pub mod rows;
pub mod sqlite;
mod token_crypto;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
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
    pub run_id: LfdId,
    pub step_index: u32,
    pub branch_index: u32,
    pub status: ForkRunStatus,
    pub worktree: String,
}

/// Token usage recorded for a single run, tagged with the wave and repo it
/// belongs to and the provider (agent) that generated it. `repo` is optional
/// because rows recorded before migration 043 carry no repo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunTokenUsage {
    pub run_id: LfdId,
    pub wave: LfdId,
    pub repo: Option<String>,
    pub provider: String,
    pub model: Option<String>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub recorded_at: i64,
}

/// Summed token usage for one (wave, provider) pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaveProviderUsage {
    pub wave: LfdId,
    pub provider: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
}

/// Summed token usage for one (repo, provider) pair. `repo` is optional to
/// account for usage rows recorded before the repo dimension existed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoProviderUsage {
    pub repo: Option<String>,
    pub provider: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
}

/// Summed token usage for one provider across all waves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderUsage {
    pub provider: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
}

/// Aggregated token usage: per (repo, provider), per (wave, provider), plus
/// per-provider rollups.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TokenUsageReport {
    pub by_repo_provider: Vec<RepoProviderUsage>,
    pub by_wave_provider: Vec<WaveProviderUsage>,
    pub by_provider: Vec<ProviderUsage>,
}

impl TokenUsageReport {
    /// Build the report from the per-(repo, provider) and per-(wave, provider)
    /// grouped rows, folding the per-provider totals from the wave rows. Rows
    /// are assumed to be one per group.
    pub fn from_grouped(
        by_repo_provider: Vec<RepoProviderUsage>,
        by_wave_provider: Vec<WaveProviderUsage>,
    ) -> Self {
        let mut by_provider: Vec<ProviderUsage> = Vec::new();
        for row in &by_wave_provider {
            match by_provider
                .iter_mut()
                .find(|entry| entry.provider == row.provider)
            {
                Some(entry) => {
                    entry.input_tokens += row.input_tokens;
                    entry.output_tokens += row.output_tokens;
                    entry.cache_read_tokens += row.cache_read_tokens;
                }
                None => by_provider.push(ProviderUsage {
                    provider: row.provider.clone(),
                    input_tokens: row.input_tokens,
                    output_tokens: row.output_tokens,
                    cache_read_tokens: row.cache_read_tokens,
                }),
            }
        }
        Self {
            by_repo_provider,
            by_wave_provider,
            by_provider,
        }
    }
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
    pub fn wave_state(&self) -> &dyn WaveStateStore {
        self
    }

    pub fn execution(&self) -> &dyn ExecutionStore {
        self
    }

    pub fn tokens(&self) -> &dyn TokenStore {
        self
    }

    pub fn repos(&self) -> &dyn RepoStore {
        self
    }

    pub fn admin(&self) -> &dyn StoreAdmin {
        self
    }

    pub async fn list_waves(&self, repo: Option<&str>) -> StoreResult<Vec<Wave>> {
        WaveStateStore::list_waves(self, repo).await
    }

    pub async fn list_loopable_waves(&self) -> StoreResult<Vec<Wave>> {
        WaveStateStore::list_loopable_waves(self).await
    }

    pub async fn list_wave_crons(&self, wave_id: &LfdId) -> StoreResult<Vec<WaveCron>> {
        WaveStateStore::list_wave_crons(self, wave_id).await
    }

    pub async fn list_all_active_crons(&self) -> StoreResult<Vec<WaveCron>> {
        WaveStateStore::list_all_active_crons(self).await
    }

    pub async fn create_wave_cron(&self, cron: &WaveCron) -> StoreResult<()> {
        WaveStateStore::create_wave_cron(self, cron).await
    }

    pub async fn replace_wave_crons(&self, wave_id: &LfdId, crons: &[WaveCron]) -> StoreResult<()> {
        self.delete_wave_crons(wave_id).await?;
        for cron in crons {
            self.create_wave_cron(cron).await?;
        }
        Ok(())
    }

    pub async fn update_wave_cron_last_triggered(
        &self,
        cron_id: &LfdId,
        last_triggered_at: Option<i64>,
    ) -> StoreResult<()> {
        WaveStateStore::update_wave_cron_last_triggered(self, cron_id, last_triggered_at).await
    }

    pub async fn delete_wave_crons(&self, wave_id: &LfdId) -> StoreResult<()> {
        WaveStateStore::delete_wave_crons(self, wave_id).await
    }

    pub async fn list_wave_repos(&self, wave_id: &LfdId) -> StoreResult<Vec<RepoWork>> {
        WaveStateStore::list_wave_repos(self, wave_id).await
    }

    pub async fn replace_wave_repos(&self, wave_id: &LfdId, repos: &[RepoWork]) -> StoreResult<()> {
        WaveStateStore::delete_wave_repos(self, wave_id).await?;
        for repo in repos {
            WaveStateStore::upsert_wave_repo(self, wave_id, repo).await?;
        }
        Ok(())
    }

    /// Stitch `repos` from the `wave_repos` table onto each wave. The store read
    /// path fills `repos: Vec::new()` in `map_wave_row`; the actual load happens
    /// here, in the layer that owns the store handle.
    async fn attach_repos_vec(&self, mut waves: Vec<Wave>) -> StoreResult<Vec<Wave>> {
        for wave in &mut waves {
            wave.repos = WaveStateStore::list_wave_repos(self, wave.id()).await?;
        }
        Ok(waves)
    }

    async fn attach_repos_opt(&self, wave: Option<Wave>) -> StoreResult<Option<Wave>> {
        match wave {
            Some(mut wave) => {
                wave.repos = WaveStateStore::list_wave_repos(self, wave.id()).await?;
                Ok(Some(wave))
            }
            None => Ok(None),
        }
    }

    /// Persist the wave's `repos` as the source of truth for per-repo execution
    /// state (worktree/branch/status/iteration). The wave-level `paused` column
    /// is written in `upsert_wave`.
    async fn persist_wave_repos(&self, wave: &Wave) -> StoreResult<()> {
        self.replace_wave_repos(wave.id(), &wave.repos).await
    }

    /// A chord's contents: the waves whose `parent_wave_id` is `parent`,
    /// ordered by creation, each stitched with its `repos`.
    pub async fn list_child_waves(&self, parent: &LfdId) -> StoreResult<Vec<Wave>> {
        WaveStateStore::children_of(self, parent).await
    }

    pub async fn get_wave(&self, wave_id: &LfdId) -> StoreResult<Option<Wave>> {
        WaveStateStore::get_wave(self, wave_id).await
    }

    pub async fn get_wave_by_name(&self, name: &str) -> StoreResult<Option<Wave>> {
        WaveStateStore::get_wave_by_name(self, name).await
    }

    pub async fn create_wave(&self, wave: &Wave) -> StoreResult<()> {
        WaveStateStore::create_wave(self, wave).await?;
        self.persist_wave_repos(wave).await
    }

    pub async fn update_wave(&self, wave: &Wave) -> StoreResult<()> {
        WaveStateStore::update_wave(self, wave).await?;
        self.persist_wave_repos(wave).await
    }

    pub async fn delete_wave(&self, wave_id: &LfdId) -> StoreResult<()> {
        WaveStateStore::delete_wave(self, wave_id).await
    }

    pub async fn list_runs(
        &self,
        wave_id: Option<&LfdId>,
        limit: Option<u32>,
    ) -> StoreResult<Vec<Run>> {
        WaveStateStore::list_runs(self, wave_id, limit).await
    }

    pub async fn get_run(&self, run_id: &LfdId) -> StoreResult<Option<Run>> {
        WaveStateStore::get_run(self, run_id).await
    }

    pub async fn get_active_run(&self, wave_id: &LfdId) -> StoreResult<Option<Run>> {
        WaveStateStore::get_active_run(self, wave_id).await
    }

    pub async fn count_active_runs(&self, wave_id: &LfdId) -> StoreResult<u32> {
        WaveStateStore::count_active_runs(self, wave_id).await
    }

    pub async fn get_latest_run(&self, wave_id: &LfdId) -> StoreResult<Option<Run>> {
        WaveStateStore::get_latest_run(self, wave_id).await
    }

    pub async fn create_run(&self, run: &Run) -> StoreResult<()> {
        WaveStateStore::create_run(self, run).await
    }

    pub async fn update_run(&self, run: &Run) -> StoreResult<()> {
        WaveStateStore::update_run(self, run).await
    }

    pub async fn list_stack_runs(&self, wave_id: &LfdId) -> StoreResult<Vec<Run>> {
        WaveStateStore::list_stack_runs(self, wave_id).await
    }

    pub async fn find_next_unmerged_run(&self, wave_id: &LfdId) -> StoreResult<Option<Run>> {
        WaveStateStore::find_next_unmerged_run(self, wave_id).await
    }

    pub async fn find_descendants(&self, run_id: &LfdId) -> StoreResult<Vec<Run>> {
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

    pub async fn list_attention_items(
        &self,
        status: Option<AttentionStatus>,
        kind: Option<AttentionKind>,
    ) -> StoreResult<Vec<AttentionItem>> {
        WaveStateStore::list_attention_items(self, status, kind).await
    }

    pub async fn get_attention_item(
        &self,
        attention_id: &LfdId,
    ) -> StoreResult<Option<AttentionItem>> {
        WaveStateStore::get_attention_item(self, attention_id).await
    }

    pub async fn find_attention_item_for_run(
        &self,
        run_id: &LfdId,
        kind: AttentionKind,
    ) -> StoreResult<Option<AttentionItem>> {
        WaveStateStore::find_attention_item_for_run(self, run_id, kind).await
    }

    pub async fn upsert_attention_item(&self, item: &AttentionItem) -> StoreResult<()> {
        WaveStateStore::upsert_attention_item(self, item).await
    }

    pub async fn delete_attention_item(&self, attention_id: &LfdId) -> StoreResult<u32> {
        WaveStateStore::delete_attention_item(self, attention_id).await
    }

    pub async fn list_queue_blocks(&self, wave_id: &LfdId) -> StoreResult<Vec<QueueBlock>> {
        WaveStateStore::list_queue_blocks(self, wave_id).await
    }

    pub async fn upsert_queue_block(&self, block: &QueueBlock) -> StoreResult<()> {
        WaveStateStore::upsert_queue_block(self, block).await
    }

    pub async fn delete_queue_block(&self, wave_id: &LfdId, run_id: &LfdId) -> StoreResult<u32> {
        WaveStateStore::delete_queue_block(self, wave_id, run_id).await
    }

    pub async fn record_merge_event(&self, event: &QueueMergeEvent) -> StoreResult<bool> {
        WaveStateStore::record_merge_event(self, event).await
    }

    pub async fn list_triggers(&self, wave_id: Option<&LfdId>) -> StoreResult<Vec<Trigger>> {
        WaveStateStore::list_triggers(self, wave_id).await
    }

    pub async fn list_triggers_by_signal(&self, signal: i32) -> StoreResult<Vec<Trigger>> {
        WaveStateStore::list_triggers_by_signal(self, signal).await
    }

    pub async fn get_trigger(&self, trigger_id: &LfdId) -> StoreResult<Option<Trigger>> {
        WaveStateStore::get_trigger(self, trigger_id).await
    }

    pub async fn create_trigger(&self, trigger: &Trigger) -> StoreResult<()> {
        WaveStateStore::create_trigger(self, trigger).await
    }

    pub async fn update_trigger(&self, trigger: &Trigger) -> StoreResult<()> {
        WaveStateStore::update_trigger(self, trigger).await
    }

    pub async fn delete_trigger(&self, trigger_id: &LfdId) -> StoreResult<()> {
        WaveStateStore::delete_trigger(self, trigger_id).await
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

    pub async fn delete_pending_activation_by_id(&self, activation_id: &LfdId) -> StoreResult<u32> {
        WaveStateStore::delete_pending_activation_by_id(self, activation_id).await
    }

    pub async fn get_pending_for_trigger(
        &self,
        wave_id: &LfdId,
        trigger_id: Option<&LfdId>,
    ) -> StoreResult<Option<PendingActivation>> {
        WaveStateStore::get_pending_for_trigger(self, wave_id, trigger_id).await
    }

    pub async fn create_activation_log(&self, log: &ActivationLog) -> StoreResult<()> {
        WaveStateStore::create_activation_log(self, log).await
    }

    pub async fn list_activation_log(
        &self,
        wave_id: &LfdId,
        limit: u32,
    ) -> StoreResult<Vec<ActivationLog>> {
        WaveStateStore::list_activation_log(self, wave_id, limit).await
    }

    pub async fn get_activation_log(
        &self,
        activation_log_id: &LfdId,
    ) -> StoreResult<Option<ActivationLog>> {
        WaveStateStore::get_activation_log(self, activation_log_id).await
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

    pub async fn record_run_usage(&self, usage: &RunTokenUsage) -> StoreResult<()> {
        WaveStateStore::record_run_usage(self, usage).await
    }

    pub async fn aggregate_token_usage(&self) -> StoreResult<TokenUsageReport> {
        WaveStateStore::aggregate_token_usage(self).await
    }

    pub async fn list_repos(&self) -> StoreResult<Vec<Repo>> {
        RepoStore::list_repos(self).await
    }

    pub async fn get_repo(&self, path: &str) -> StoreResult<Option<Repo>> {
        RepoStore::get_repo(self, path).await
    }

    pub async fn upsert_repo(&self, repo: &Repo) -> StoreResult<()> {
        RepoStore::upsert_repo(self, repo).await
    }

    pub async fn delete_repo(&self, path: &str) -> StoreResult<()> {
        RepoStore::delete_repo(self, path).await
    }

    pub async fn get_repo_by_repo_id(&self, repo_id: &RepoId) -> StoreResult<Option<Repo>> {
        RepoStore::get_repo_by_repo_id(self, repo_id).await
    }

    pub async fn list_edges(&self) -> StoreResult<Vec<RepoEdge>> {
        RepoStore::list_edges(self).await
    }

    pub async fn add_edge(&self, edge: &RepoEdge) -> StoreResult<()> {
        RepoStore::add_edge(self, edge).await
    }

    pub async fn remove_edge(&self, parent_id: &RepoId, child_id: &RepoId) -> StoreResult<()> {
        RepoStore::remove_edge(self, parent_id, child_id).await
    }

    pub async fn children(&self, repo_id: &RepoId) -> StoreResult<Vec<Repo>> {
        RepoStore::children(self, repo_id).await
    }

    pub async fn parents(&self, repo_id: &RepoId) -> StoreResult<Vec<Repo>> {
        RepoStore::parents(self, repo_id).await
    }

    pub async fn list_fork_runs(
        &self,
        run_id: &LfdId,
        step_index: u32,
    ) -> StoreResult<Vec<ForkRun>> {
        ExecutionStore::list_fork_runs(self, run_id, step_index).await
    }

    pub async fn upsert_fork_run(&self, fork_run: &ForkRun) -> StoreResult<()> {
        ExecutionStore::upsert_fork_run(self, fork_run).await
    }

    pub async fn delete_fork_runs(&self, run_id: &LfdId, step_index: u32) -> StoreResult<u32> {
        ExecutionStore::delete_fork_runs(self, run_id, step_index).await
    }

    pub async fn list_agents(&self) -> StoreResult<Vec<ExecutionProcess>> {
        ExecutionStore::list_agents(self).await
    }

    pub async fn list_agent_history(
        &self,
        worktree: Option<&str>,
        repo: Option<&str>,
        limit: Option<u32>,
    ) -> StoreResult<Vec<ExecutionProcess>> {
        ExecutionStore::list_agent_history(self, worktree, repo, limit).await
    }

    pub async fn get_agent(&self, agent_id: &LfdId) -> StoreResult<Option<ExecutionProcess>> {
        ExecutionStore::get_agent(self, agent_id).await
    }

    pub async fn get_waiting_agent_for_wave(
        &self,
        wave_id: &LfdId,
    ) -> StoreResult<Option<ExecutionProcess>> {
        ExecutionStore::get_waiting_agent_for_wave(self, wave_id).await
    }

    pub async fn start_agent(&self, execution_run: &ExecutionProcess) -> StoreResult<()> {
        ExecutionStore::start_agent(self, execution_run).await
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

    pub async fn get_active_agents_for_wave(
        &self,
        wave_id: &LfdId,
    ) -> StoreResult<Vec<ExecutionProcess>> {
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

    pub async fn get_stuck_agents(
        &self,
        older_than_secs: u64,
    ) -> StoreResult<Vec<ExecutionProcess>> {
        ExecutionStore::get_stuck_agents(self, older_than_secs).await
    }

    pub async fn fail_orphaned_runs(&self) -> StoreResult<u32> {
        ExecutionStore::fail_orphaned_runs(self).await
    }

    pub async fn create_control_session(&self, session: &Session) -> StoreResult<()> {
        ControlSessionStore::create_session(self, session).await
    }

    pub async fn get_control_session(&self, session_id: &LfdId) -> StoreResult<Option<Session>> {
        ControlSessionStore::get_session(self, session_id).await
    }

    pub async fn list_control_sessions(
        &self,
        wave_id: Option<&LfdId>,
        statuses: Option<&[SessionStatus]>,
    ) -> StoreResult<Vec<Session>> {
        ControlSessionStore::list_sessions(self, wave_id, statuses).await
    }

    pub async fn get_active_control_session_for_run(
        &self,
        run_id: &LfdId,
    ) -> StoreResult<Option<Session>> {
        ControlSessionStore::get_active_session_for_run(self, run_id).await
    }

    pub async fn update_control_session(&self, session: &Session) -> StoreResult<()> {
        ControlSessionStore::update_session(self, session).await
    }

    pub async fn get_provider_token(&self, provider: &str) -> StoreResult<Option<ProviderToken>> {
        TokenStore::get_provider_token(self, provider).await
    }

    pub async fn upsert_provider_token(&self, token: &ProviderToken) -> StoreResult<()> {
        TokenStore::upsert_provider_token(self, token).await
    }

    pub async fn delete_provider_token(&self, provider: &str) -> StoreResult<()> {
        TokenStore::delete_provider_token(self, provider).await
    }

    pub async fn list_provider_tokens(&self) -> StoreResult<Vec<ProviderToken>> {
        TokenStore::list_provider_tokens(self).await
    }

    pub async fn upsert_secrets_provider_config(
        &self,
        config: &SecretsProviderConfig,
    ) -> StoreResult<()> {
        SecretsProviderStore::upsert_secrets_provider_config(self, config).await
    }

    pub async fn delete_secrets_provider_config(&self, provider: &str) -> StoreResult<()> {
        SecretsProviderStore::delete_secrets_provider_config(self, provider).await
    }

    pub async fn list_secrets_provider_configs(&self) -> StoreResult<Vec<SecretsProviderConfig>> {
        SecretsProviderStore::list_secrets_provider_configs(self).await
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
    async fn list_loopable_waves(&self) -> StoreResult<Vec<Wave>>;
    async fn list_wave_crons(&self, wave_id: &LfdId) -> StoreResult<Vec<WaveCron>>;
    async fn list_all_active_crons(&self) -> StoreResult<Vec<WaveCron>>;
    async fn children_of(&self, parent: &LfdId) -> StoreResult<Vec<Wave>>;
    async fn get_wave(&self, wave_id: &LfdId) -> StoreResult<Option<Wave>>;
    async fn get_wave_by_name(&self, name: &str) -> StoreResult<Option<Wave>>;
    async fn create_wave(&self, wave: &Wave) -> StoreResult<()>;
    async fn update_wave(&self, wave: &Wave) -> StoreResult<()>;
    async fn delete_wave(&self, wave_id: &LfdId) -> StoreResult<()>;
    async fn create_wave_cron(&self, cron: &WaveCron) -> StoreResult<()>;
    async fn update_wave_cron_last_triggered(
        &self,
        cron_id: &LfdId,
        last_triggered_at: Option<i64>,
    ) -> StoreResult<()>;
    async fn delete_wave_crons(&self, wave_id: &LfdId) -> StoreResult<()>;
    async fn list_wave_repos(&self, wave_id: &LfdId) -> StoreResult<Vec<RepoWork>>;
    async fn upsert_wave_repo(&self, wave_id: &LfdId, repo: &RepoWork) -> StoreResult<()>;
    async fn delete_wave_repos(&self, wave_id: &LfdId) -> StoreResult<()>;

    async fn list_runs(&self, wave_id: Option<&LfdId>, limit: Option<u32>)
        -> StoreResult<Vec<Run>>;
    async fn get_run(&self, run_id: &LfdId) -> StoreResult<Option<Run>>;
    async fn get_active_run(&self, wave_id: &LfdId) -> StoreResult<Option<Run>>;
    async fn count_active_runs(&self, wave_id: &LfdId) -> StoreResult<u32>;
    async fn get_latest_run(&self, wave_id: &LfdId) -> StoreResult<Option<Run>>;
    async fn create_run(&self, run: &Run) -> StoreResult<()>;
    async fn update_run(&self, run: &Run) -> StoreResult<()>;
    async fn list_stack_runs(&self, wave_id: &LfdId) -> StoreResult<Vec<Run>>;
    async fn find_next_unmerged_run(&self, wave_id: &LfdId) -> StoreResult<Option<Run>>;
    async fn find_descendants(&self, run_id: &LfdId) -> StoreResult<Vec<Run>>;
    async fn get_live_pr_state(
        &self,
        repo_id: &str,
        pr_number: u32,
    ) -> StoreResult<Option<LivePullRequestState>>;
    async fn upsert_live_pr_state(&self, state: &LivePullRequestState) -> StoreResult<()>;
    async fn list_attention_items(
        &self,
        status: Option<AttentionStatus>,
        kind: Option<AttentionKind>,
    ) -> StoreResult<Vec<AttentionItem>>;
    async fn get_attention_item(&self, attention_id: &LfdId) -> StoreResult<Option<AttentionItem>>;
    async fn find_attention_item_for_run(
        &self,
        run_id: &LfdId,
        kind: AttentionKind,
    ) -> StoreResult<Option<AttentionItem>>;
    async fn upsert_attention_item(&self, item: &AttentionItem) -> StoreResult<()>;
    async fn delete_attention_item(&self, attention_id: &LfdId) -> StoreResult<u32>;
    async fn list_queue_blocks(&self, wave_id: &LfdId) -> StoreResult<Vec<QueueBlock>>;
    async fn upsert_queue_block(&self, block: &QueueBlock) -> StoreResult<()>;
    async fn delete_queue_block(&self, wave_id: &LfdId, run_id: &LfdId) -> StoreResult<u32>;
    async fn record_merge_event(&self, event: &QueueMergeEvent) -> StoreResult<bool>;

    async fn list_triggers(&self, wave_id: Option<&LfdId>) -> StoreResult<Vec<Trigger>>;
    async fn list_triggers_by_signal(&self, signal: i32) -> StoreResult<Vec<Trigger>>;
    async fn get_trigger(&self, trigger_id: &LfdId) -> StoreResult<Option<Trigger>>;
    async fn create_trigger(&self, trigger: &Trigger) -> StoreResult<()>;
    async fn update_trigger(&self, trigger: &Trigger) -> StoreResult<()>;
    async fn delete_trigger(&self, trigger_id: &LfdId) -> StoreResult<()>;
    async fn list_pending_activations(
        &self,
        wave_id: &LfdId,
    ) -> StoreResult<Vec<PendingActivation>>;
    async fn create_pending_activation(&self, activation: &PendingActivation) -> StoreResult<()>;
    async fn update_pending_activation(&self, activation: &PendingActivation) -> StoreResult<()>;
    async fn delete_pending_activation_by_id(&self, activation_id: &LfdId) -> StoreResult<u32>;
    async fn get_pending_for_trigger(
        &self,
        wave_id: &LfdId,
        trigger_id: Option<&LfdId>,
    ) -> StoreResult<Option<PendingActivation>>;
    async fn create_activation_log(&self, log: &ActivationLog) -> StoreResult<()>;
    async fn list_activation_log(
        &self,
        wave_id: &LfdId,
        limit: u32,
    ) -> StoreResult<Vec<ActivationLog>>;
    async fn get_activation_log(
        &self,
        activation_log_id: &LfdId,
    ) -> StoreResult<Option<ActivationLog>>;

    async fn get_summary(&self, wave_id: &LfdId) -> StoreResult<Option<Summary>>;
    async fn upsert_summary(&self, summary: &Summary) -> StoreResult<()>;

    async fn list_chat_memory_blocks(&self, wave_id: &LfdId) -> StoreResult<Vec<ChatMemoryBlock>>;
    async fn upsert_chat_memory_block(&self, block: &ChatMemoryBlock) -> StoreResult<()>;
    async fn delete_chat_memory_block(&self, wave_id: &LfdId, name: &str) -> StoreResult<()>;

    async fn list_chat_messages(&self, wave_id: &LfdId) -> StoreResult<Vec<ChatMessage>>;
    async fn create_chat_message(&self, message: &ChatMessage) -> StoreResult<()>;

    async fn record_run_usage(&self, usage: &RunTokenUsage) -> StoreResult<()>;
    async fn aggregate_token_usage(&self) -> StoreResult<TokenUsageReport>;
}

#[async_trait::async_trait]
pub trait RepoStore: Send + Sync {
    async fn list_repos(&self) -> StoreResult<Vec<Repo>>;
    async fn get_repo(&self, path: &str) -> StoreResult<Option<Repo>>;
    async fn upsert_repo(&self, repo: &Repo) -> StoreResult<()>;
    async fn delete_repo(&self, path: &str) -> StoreResult<()>;
    async fn get_repo_by_repo_id(&self, repo_id: &RepoId) -> StoreResult<Option<Repo>>;
    async fn list_edges(&self) -> StoreResult<Vec<RepoEdge>>;
    async fn add_edge(&self, edge: &RepoEdge) -> StoreResult<()>;
    async fn remove_edge(&self, parent_id: &RepoId, child_id: &RepoId) -> StoreResult<()>;
    async fn children(&self, repo_id: &RepoId) -> StoreResult<Vec<Repo>>;
    async fn parents(&self, repo_id: &RepoId) -> StoreResult<Vec<Repo>>;
}

#[async_trait::async_trait]
pub trait ExecutionStore: Send + Sync {
    async fn list_fork_runs(&self, run_id: &LfdId, step_index: u32) -> StoreResult<Vec<ForkRun>>;
    async fn list_orphaned_fork_runs(&self) -> StoreResult<Vec<ForkRun>>;
    async fn upsert_fork_run(&self, fork_run: &ForkRun) -> StoreResult<()>;
    async fn delete_fork_runs(&self, run_id: &LfdId, step_index: u32) -> StoreResult<u32>;

    async fn list_agents(&self) -> StoreResult<Vec<ExecutionProcess>>;
    async fn list_agent_history(
        &self,
        worktree: Option<&str>,
        repo: Option<&str>,
        limit: Option<u32>,
    ) -> StoreResult<Vec<ExecutionProcess>>;
    async fn get_agent(&self, agent_id: &LfdId) -> StoreResult<Option<ExecutionProcess>>;
    async fn get_waiting_agent_for_wave(
        &self,
        wave_id: &LfdId,
    ) -> StoreResult<Option<ExecutionProcess>>;
    async fn start_agent(&self, execution_run: &ExecutionProcess) -> StoreResult<()>;
    async fn update_agent_status(
        &self,
        agent_id: &LfdId,
        status: i32,
        pid: Option<u32>,
        container_id: Option<&str>,
    ) -> StoreResult<()>;
    async fn end_agent(&self, agent_id: &LfdId, status: i32, ended_at: i64) -> StoreResult<()>;
    async fn get_active_agents_for_wave(
        &self,
        wave_id: &LfdId,
    ) -> StoreResult<Vec<ExecutionProcess>>;
    async fn end_active_agent_for_wave(
        &self,
        wave_id: &LfdId,
        status: i32,
        ended_at: i64,
    ) -> StoreResult<()>;
    async fn get_stuck_agents(&self, older_than_secs: u64) -> StoreResult<Vec<ExecutionProcess>>;

    async fn fail_orphaned_runs(&self) -> StoreResult<u32>;
}

#[async_trait::async_trait]
pub trait ControlSessionStore: Send + Sync {
    async fn create_session(&self, session: &Session) -> StoreResult<()>;
    async fn get_session(&self, session_id: &LfdId) -> StoreResult<Option<Session>>;
    async fn list_sessions(
        &self,
        wave_id: Option<&LfdId>,
        statuses: Option<&[SessionStatus]>,
    ) -> StoreResult<Vec<Session>>;
    async fn get_active_session_for_run(&self, run_id: &LfdId) -> StoreResult<Option<Session>>;
    async fn update_session(&self, session: &Session) -> StoreResult<()>;
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
    pub expires_at: Option<i64>,
    pub login: Option<String>,
    pub updated_at: i64,
    pub credential_type: CredentialType,
}

#[async_trait::async_trait]
pub trait TokenStore: Send + Sync {
    async fn get_provider_token(&self, provider: &str) -> StoreResult<Option<ProviderToken>>;
    async fn upsert_provider_token(&self, token: &ProviderToken) -> StoreResult<()>;
    async fn delete_provider_token(&self, provider: &str) -> StoreResult<()>;
    async fn list_provider_tokens(&self) -> StoreResult<Vec<ProviderToken>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretsProviderConfig {
    pub provider: String,
    pub project: Option<String>,
    pub config: Option<String>,
    pub updated_at: i64,
}

#[async_trait::async_trait]
pub trait SecretsProviderStore: Send + Sync {
    async fn upsert_secrets_provider_config(
        &self,
        config: &SecretsProviderConfig,
    ) -> StoreResult<()>;
    async fn delete_secrets_provider_config(&self, provider: &str) -> StoreResult<()>;
    async fn list_secrets_provider_configs(&self) -> StoreResult<Vec<SecretsProviderConfig>>;
}

#[async_trait::async_trait]
pub trait StoreAdmin: Send + Sync {
    async fn health_check(&self) -> StoreResult<()>;
    async fn schema_version(&self) -> StoreResult<String>;
}

#[async_trait::async_trait]
impl WaveStateStore for Store {
    // Keep backend dispatch explicit and centralized in this file.
    // Verbose match arms are intentional: they keep sqlite/postgres behavior greppable.
    async fn list_waves(&self, repo: Option<&str>) -> StoreResult<Vec<Wave>> {
        let waves = match &self.backend {
            StoreBackend::Sqlite(store) => {
                let repo = repo.map(str::to_string);
                run_sqlite(store, move |store| store.list_waves(repo.as_deref())).await
            }
            StoreBackend::Postgres(store) => store.list_waves(repo).await,
        }?;
        self.attach_repos_vec(waves).await
    }

    async fn list_loopable_waves(&self) -> StoreResult<Vec<Wave>> {
        let waves = match &self.backend {
            StoreBackend::Sqlite(store) => {
                run_sqlite(store, move |store| store.list_loopable_waves()).await
            }
            StoreBackend::Postgres(store) => store.list_loopable_waves().await,
        }?;
        self.attach_repos_vec(waves).await
    }

    async fn list_wave_crons(&self, wave_id: &LfdId) -> StoreResult<Vec<WaveCron>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let wave_id = wave_id.clone();
                run_sqlite(store, move |store| store.list_wave_crons(&wave_id)).await
            }
            StoreBackend::Postgres(store) => store.list_wave_crons(wave_id).await,
        }
    }

    async fn list_all_active_crons(&self) -> StoreResult<Vec<WaveCron>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                run_sqlite(store, move |store| store.list_all_active_crons()).await
            }
            StoreBackend::Postgres(store) => store.list_all_active_crons().await,
        }
    }

    async fn children_of(&self, parent: &LfdId) -> StoreResult<Vec<Wave>> {
        let waves = match &self.backend {
            StoreBackend::Sqlite(store) => {
                let parent = parent.clone();
                run_sqlite(store, move |store| store.list_child_waves(&parent)).await
            }
            StoreBackend::Postgres(store) => store.list_child_waves(parent).await,
        }?;
        self.attach_repos_vec(waves).await
    }

    async fn get_wave(&self, wave_id: &LfdId) -> StoreResult<Option<Wave>> {
        let wave = match &self.backend {
            StoreBackend::Sqlite(store) => {
                let wave_id = wave_id.clone();
                run_sqlite(store, move |store| store.get_wave(&wave_id)).await
            }
            StoreBackend::Postgres(store) => store.get_wave(wave_id).await,
        }?;
        self.attach_repos_opt(wave).await
    }

    async fn get_wave_by_name(&self, name: &str) -> StoreResult<Option<Wave>> {
        let wave = match &self.backend {
            StoreBackend::Sqlite(store) => {
                let name = name.to_string();
                run_sqlite(store, move |store| store.get_wave_by_name(&name)).await
            }
            StoreBackend::Postgres(store) => store.get_wave_by_name(name).await,
        }?;
        self.attach_repos_opt(wave).await
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

    async fn create_wave_cron(&self, cron: &WaveCron) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let cron = cron.clone();
                run_sqlite(store, move |store| store.create_wave_cron(&cron)).await
            }
            StoreBackend::Postgres(store) => store.create_wave_cron(cron).await,
        }
    }

    async fn update_wave_cron_last_triggered(
        &self,
        cron_id: &LfdId,
        last_triggered_at: Option<i64>,
    ) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let cron_id = cron_id.clone();
                run_sqlite(store, move |store| {
                    store.update_wave_cron_last_triggered(&cron_id, last_triggered_at)
                })
                .await
            }
            StoreBackend::Postgres(store) => {
                store
                    .update_wave_cron_last_triggered(cron_id, last_triggered_at)
                    .await
            }
        }
    }

    async fn delete_wave_crons(&self, wave_id: &LfdId) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let wave_id = wave_id.clone();
                run_sqlite(store, move |store| store.delete_wave_crons(&wave_id)).await
            }
            StoreBackend::Postgres(store) => store.delete_wave_crons(wave_id).await,
        }
    }

    async fn list_wave_repos(&self, wave_id: &LfdId) -> StoreResult<Vec<RepoWork>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let wave_id = wave_id.clone();
                run_sqlite(store, move |store| store.list_wave_repos(&wave_id)).await
            }
            StoreBackend::Postgres(store) => store.list_wave_repos(wave_id).await,
        }
    }

    async fn upsert_wave_repo(&self, wave_id: &LfdId, repo: &RepoWork) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let wave_id = wave_id.clone();
                let repo = repo.clone();
                run_sqlite(store, move |store| store.upsert_wave_repo(&wave_id, &repo)).await
            }
            StoreBackend::Postgres(store) => store.upsert_wave_repo(wave_id, repo).await,
        }
    }

    async fn delete_wave_repos(&self, wave_id: &LfdId) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let wave_id = wave_id.clone();
                run_sqlite(store, move |store| store.delete_wave_repos(&wave_id)).await
            }
            StoreBackend::Postgres(store) => store.delete_wave_repos(wave_id).await,
        }
    }

    async fn list_runs(
        &self,
        wave_id: Option<&LfdId>,
        limit: Option<u32>,
    ) -> StoreResult<Vec<Run>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let wave_id = wave_id.cloned();
                run_sqlite(store, move |store| store.list_runs(wave_id.as_ref(), limit)).await
            }
            StoreBackend::Postgres(store) => store.list_runs(wave_id, limit).await,
        }
    }

    async fn get_run(&self, run_id: &LfdId) -> StoreResult<Option<Run>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let run_id = run_id.clone();
                run_sqlite(store, move |store| store.get_run(&run_id)).await
            }
            StoreBackend::Postgres(store) => store.get_run(run_id).await,
        }
    }

    async fn get_active_run(&self, wave_id: &LfdId) -> StoreResult<Option<Run>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let wave_id = wave_id.clone();
                run_sqlite(store, move |store| store.get_active_run(&wave_id)).await
            }
            StoreBackend::Postgres(store) => store.get_active_run(wave_id).await,
        }
    }

    async fn count_active_runs(&self, wave_id: &LfdId) -> StoreResult<u32> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let wave_id = wave_id.clone();
                run_sqlite(store, move |store| store.count_active_runs(&wave_id)).await
            }
            StoreBackend::Postgres(store) => store.count_active_runs(wave_id).await,
        }
    }

    async fn get_latest_run(&self, wave_id: &LfdId) -> StoreResult<Option<Run>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let wave_id = wave_id.clone();
                run_sqlite(store, move |store| store.get_latest_run(&wave_id)).await
            }
            StoreBackend::Postgres(store) => store.get_latest_run(wave_id).await,
        }
    }

    async fn create_run(&self, run: &Run) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let run = run.clone();
                run_sqlite(store, move |store| store.create_run(&run)).await
            }
            StoreBackend::Postgres(store) => store.create_run(run).await,
        }
    }

    async fn update_run(&self, run: &Run) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let run = run.clone();
                run_sqlite(store, move |store| store.update_run(&run)).await
            }
            StoreBackend::Postgres(store) => store.update_run(run).await,
        }
    }

    async fn list_stack_runs(&self, wave_id: &LfdId) -> StoreResult<Vec<Run>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let wave_id = wave_id.clone();
                run_sqlite(store, move |store| store.list_stack_runs(&wave_id)).await
            }
            StoreBackend::Postgres(store) => store.list_stack_runs(wave_id).await,
        }
    }

    async fn find_next_unmerged_run(&self, wave_id: &LfdId) -> StoreResult<Option<Run>> {
        let runs = self.list_stack_runs(wave_id).await?;
        for run in runs {
            if matches!(
                run.stack_status,
                RunStackStatus::Merged | RunStackStatus::Superseded
            ) {
                continue;
            }

            let Some(pr_number) = run.pr.as_ref().and_then(|pr| pr.number) else {
                return Ok(Some(run));
            };

            let Some(live_state) = self.get_live_pr_state(&run.repo, pr_number).await? else {
                return Ok(Some(run));
            };
            if live_state.state != LivePrState::Merged {
                return Ok(Some(run));
            }
        }
        Ok(None)
    }

    async fn find_descendants(&self, run_id: &LfdId) -> StoreResult<Vec<Run>> {
        let Some(parent) = self.get_run(run_id).await? else {
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

    async fn list_attention_items(
        &self,
        status: Option<AttentionStatus>,
        kind: Option<AttentionKind>,
    ) -> StoreResult<Vec<AttentionItem>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                run_sqlite(store, move |store| store.list_attention_items(status, kind)).await
            }
            StoreBackend::Postgres(store) => store.list_attention_items(status, kind).await,
        }
    }

    async fn get_attention_item(&self, attention_id: &LfdId) -> StoreResult<Option<AttentionItem>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let attention_id = attention_id.clone();
                run_sqlite(store, move |store| store.get_attention_item(&attention_id)).await
            }
            StoreBackend::Postgres(store) => store.get_attention_item(attention_id).await,
        }
    }

    async fn find_attention_item_for_run(
        &self,
        run_id: &LfdId,
        kind: AttentionKind,
    ) -> StoreResult<Option<AttentionItem>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let run_id = run_id.clone();
                run_sqlite(store, move |store| {
                    store.find_attention_item_for_run(&run_id, kind)
                })
                .await
            }
            StoreBackend::Postgres(store) => store.find_attention_item_for_run(run_id, kind).await,
        }
    }

    async fn upsert_attention_item(&self, item: &AttentionItem) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let item = item.clone();
                run_sqlite(store, move |store| store.upsert_attention_item(&item)).await
            }
            StoreBackend::Postgres(store) => store.upsert_attention_item(item).await,
        }
    }

    async fn delete_attention_item(&self, attention_id: &LfdId) -> StoreResult<u32> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let attention_id = attention_id.clone();
                run_sqlite(store, move |store| {
                    store.delete_attention_item(&attention_id)
                })
                .await
            }
            StoreBackend::Postgres(store) => store.delete_attention_item(attention_id).await,
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

    async fn list_triggers(&self, wave_id: Option<&LfdId>) -> StoreResult<Vec<Trigger>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let wave_id = wave_id.cloned();
                run_sqlite(store, move |store| store.list_triggers(wave_id.as_ref())).await
            }
            StoreBackend::Postgres(store) => store.list_triggers(wave_id).await,
        }
    }

    async fn list_triggers_by_signal(&self, signal: i32) -> StoreResult<Vec<Trigger>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                run_sqlite(store, move |store| store.list_triggers_by_signal(signal)).await
            }
            StoreBackend::Postgres(store) => store.list_triggers_by_signal(signal).await,
        }
    }

    async fn get_trigger(&self, trigger_id: &LfdId) -> StoreResult<Option<Trigger>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let trigger_id = trigger_id.clone();
                run_sqlite(store, move |store| store.get_trigger(&trigger_id)).await
            }
            StoreBackend::Postgres(store) => store.get_trigger(trigger_id).await,
        }
    }

    async fn create_trigger(&self, trigger: &Trigger) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let trigger = trigger.clone();
                run_sqlite(store, move |store| store.create_trigger(&trigger)).await
            }
            StoreBackend::Postgres(store) => store.create_trigger(trigger).await,
        }
    }

    async fn update_trigger(&self, trigger: &Trigger) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let trigger = trigger.clone();
                run_sqlite(store, move |store| store.update_trigger(&trigger)).await
            }
            StoreBackend::Postgres(store) => store.update_trigger(trigger).await,
        }
    }

    async fn delete_trigger(&self, trigger_id: &LfdId) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let trigger_id = trigger_id.clone();
                run_sqlite(store, move |store| store.delete_trigger(&trigger_id)).await
            }
            StoreBackend::Postgres(store) => store.delete_trigger(trigger_id).await,
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

    async fn delete_pending_activation_by_id(&self, activation_id: &LfdId) -> StoreResult<u32> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let activation_id = activation_id.clone();
                run_sqlite(store, move |store| {
                    store.delete_pending_activation_by_id(&activation_id)
                })
                .await
            }
            StoreBackend::Postgres(store) => {
                store.delete_pending_activation_by_id(activation_id).await
            }
        }
    }

    async fn get_pending_for_trigger(
        &self,
        wave_id: &LfdId,
        trigger_id: Option<&LfdId>,
    ) -> StoreResult<Option<PendingActivation>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let wave_id = wave_id.clone();
                let trigger_id = trigger_id.cloned();
                run_sqlite(store, move |store| {
                    store.get_pending_for_trigger(&wave_id, trigger_id.as_ref())
                })
                .await
            }
            StoreBackend::Postgres(store) => {
                store.get_pending_for_trigger(wave_id, trigger_id).await
            }
        }
    }

    async fn create_activation_log(&self, log: &ActivationLog) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let log = log.clone();
                run_sqlite(store, move |store| store.create_activation_log(&log)).await
            }
            StoreBackend::Postgres(store) => store.create_activation_log(log).await,
        }
    }

    async fn list_activation_log(
        &self,
        wave_id: &LfdId,
        limit: u32,
    ) -> StoreResult<Vec<ActivationLog>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let wave_id = wave_id.clone();
                run_sqlite(store, move |store| {
                    store.list_activation_log(&wave_id, limit)
                })
                .await
            }
            StoreBackend::Postgres(store) => store.list_activation_log(wave_id, limit).await,
        }
    }

    async fn get_activation_log(
        &self,
        activation_log_id: &LfdId,
    ) -> StoreResult<Option<ActivationLog>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let activation_log_id = activation_log_id.clone();
                run_sqlite(store, move |store| {
                    store.get_activation_log(&activation_log_id)
                })
                .await
            }
            StoreBackend::Postgres(store) => store.get_activation_log(activation_log_id).await,
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

    async fn record_run_usage(&self, usage: &RunTokenUsage) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let usage = usage.clone();
                run_sqlite(store, move |store| store.record_run_usage(&usage)).await
            }
            StoreBackend::Postgres(store) => store.record_run_usage(usage).await,
        }
    }

    async fn aggregate_token_usage(&self) -> StoreResult<TokenUsageReport> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                run_sqlite(store, move |store| store.aggregate_token_usage()).await
            }
            StoreBackend::Postgres(store) => store.aggregate_token_usage().await,
        }
    }
}

#[async_trait::async_trait]
impl RepoStore for Store {
    async fn list_repos(&self) -> StoreResult<Vec<Repo>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => run_sqlite(store, |store| store.list_repos()).await,
            StoreBackend::Postgres(store) => store.list_repos().await,
        }
    }

    async fn get_repo(&self, path: &str) -> StoreResult<Option<Repo>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let path = path.to_string();
                run_sqlite(store, move |store| store.get_repo(&path)).await
            }
            StoreBackend::Postgres(store) => store.get_repo(path).await,
        }
    }

    async fn upsert_repo(&self, repo: &Repo) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let repo = repo.clone();
                run_sqlite(store, move |store| store.upsert_repo(&repo)).await
            }
            StoreBackend::Postgres(store) => store.upsert_repo(repo).await,
        }
    }

    async fn delete_repo(&self, path: &str) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let path = path.to_string();
                run_sqlite(store, move |store| store.delete_repo(&path)).await
            }
            StoreBackend::Postgres(store) => store.delete_repo(path).await,
        }
    }

    async fn get_repo_by_repo_id(&self, repo_id: &RepoId) -> StoreResult<Option<Repo>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let repo_id = repo_id.clone();
                run_sqlite(store, move |store| store.get_repo_by_repo_id(&repo_id)).await
            }
            StoreBackend::Postgres(store) => store.get_repo_by_repo_id(repo_id).await,
        }
    }

    async fn list_edges(&self) -> StoreResult<Vec<RepoEdge>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => run_sqlite(store, |store| store.list_edges()).await,
            StoreBackend::Postgres(store) => store.list_edges().await,
        }
    }

    async fn add_edge(&self, edge: &RepoEdge) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let edge = edge.clone();
                run_sqlite(store, move |store| store.add_edge(&edge)).await
            }
            StoreBackend::Postgres(store) => store.add_edge(edge).await,
        }
    }

    async fn remove_edge(&self, parent_id: &RepoId, child_id: &RepoId) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let parent_id = parent_id.clone();
                let child_id = child_id.clone();
                run_sqlite(store, move |store| store.remove_edge(&parent_id, &child_id)).await
            }
            StoreBackend::Postgres(store) => store.remove_edge(parent_id, child_id).await,
        }
    }

    async fn children(&self, repo_id: &RepoId) -> StoreResult<Vec<Repo>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let repo_id = repo_id.clone();
                run_sqlite(store, move |store| store.children(&repo_id)).await
            }
            StoreBackend::Postgres(store) => store.children(repo_id).await,
        }
    }

    async fn parents(&self, repo_id: &RepoId) -> StoreResult<Vec<Repo>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let repo_id = repo_id.clone();
                run_sqlite(store, move |store| store.parents(&repo_id)).await
            }
            StoreBackend::Postgres(store) => store.parents(repo_id).await,
        }
    }
}

#[async_trait::async_trait]
impl ExecutionStore for Store {
    async fn list_fork_runs(&self, run_id: &LfdId, step_index: u32) -> StoreResult<Vec<ForkRun>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let run_id = run_id.clone();
                run_sqlite(store, move |store| {
                    store.list_fork_runs(&run_id, step_index)
                })
                .await
            }
            StoreBackend::Postgres(store) => store.list_fork_runs(run_id, step_index).await,
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

    async fn list_orphaned_fork_runs(&self) -> StoreResult<Vec<ForkRun>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                run_sqlite(store, |store| store.list_orphaned_fork_runs()).await
            }
            StoreBackend::Postgres(store) => store.list_orphaned_fork_runs().await,
        }
    }

    async fn delete_fork_runs(&self, run_id: &LfdId, step_index: u32) -> StoreResult<u32> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let run_id = run_id.clone();
                run_sqlite(store, move |store| {
                    store.delete_fork_runs(&run_id, step_index)
                })
                .await
            }
            StoreBackend::Postgres(store) => store.delete_fork_runs(run_id, step_index).await,
        }
    }

    async fn list_agents(&self) -> StoreResult<Vec<ExecutionProcess>> {
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
    ) -> StoreResult<Vec<ExecutionProcess>> {
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

    async fn get_agent(&self, agent_id: &LfdId) -> StoreResult<Option<ExecutionProcess>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let agent_id = agent_id.clone();
                run_sqlite(store, move |store| store.get_agent(&agent_id)).await
            }
            StoreBackend::Postgres(store) => store.get_agent(agent_id).await,
        }
    }

    async fn get_waiting_agent_for_wave(
        &self,
        wave_id: &LfdId,
    ) -> StoreResult<Option<ExecutionProcess>> {
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

    async fn start_agent(&self, execution_run: &ExecutionProcess) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let execution_run = execution_run.clone();
                run_sqlite(store, move |store| store.start_agent(&execution_run)).await
            }
            StoreBackend::Postgres(store) => store.start_agent(execution_run).await,
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

    async fn get_active_agents_for_wave(
        &self,
        wave_id: &LfdId,
    ) -> StoreResult<Vec<ExecutionProcess>> {
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

    async fn get_stuck_agents(&self, older_than_secs: u64) -> StoreResult<Vec<ExecutionProcess>> {
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
impl ControlSessionStore for Store {
    async fn create_session(&self, session: &Session) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let session = session.clone();
                run_sqlite(store, move |store| store.create_control_session(&session)).await
            }
            StoreBackend::Postgres(store) => store.create_control_session(session).await,
        }
    }

    async fn get_session(&self, session_id: &LfdId) -> StoreResult<Option<Session>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let session_id = session_id.clone();
                run_sqlite(store, move |store| store.get_control_session(&session_id)).await
            }
            StoreBackend::Postgres(store) => store.get_control_session(session_id).await,
        }
    }

    async fn list_sessions(
        &self,
        wave_id: Option<&LfdId>,
        statuses: Option<&[SessionStatus]>,
    ) -> StoreResult<Vec<Session>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let wave_id = wave_id.cloned();
                let statuses = statuses.map(|values| values.to_vec());
                run_sqlite(store, move |store| {
                    store.list_control_sessions(wave_id.as_ref(), statuses.as_deref())
                })
                .await
            }
            StoreBackend::Postgres(store) => store.list_control_sessions(wave_id, statuses).await,
        }
    }

    async fn get_active_session_for_run(&self, run_id: &LfdId) -> StoreResult<Option<Session>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let run_id = run_id.clone();
                run_sqlite(store, move |store| {
                    store.get_active_control_session_for_run(&run_id)
                })
                .await
            }
            StoreBackend::Postgres(store) => store.get_active_control_session_for_run(run_id).await,
        }
    }

    async fn update_session(&self, session: &Session) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let session = session.clone();
                run_sqlite(store, move |store| store.update_control_session(&session)).await
            }
            StoreBackend::Postgres(store) => store.update_control_session(session).await,
        }
    }
}

#[async_trait::async_trait]
impl TokenStore for Store {
    async fn get_provider_token(&self, provider: &str) -> StoreResult<Option<ProviderToken>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let provider = provider.to_string();
                run_sqlite(store, move |store| store.get_provider_token(&provider)).await
            }
            StoreBackend::Postgres(store) => store.get_provider_token(provider).await,
        }
    }

    async fn upsert_provider_token(&self, token: &ProviderToken) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let token = token.clone();
                run_sqlite(store, move |store| store.upsert_provider_token(&token)).await
            }
            StoreBackend::Postgres(store) => store.upsert_provider_token(token).await,
        }
    }

    async fn delete_provider_token(&self, provider: &str) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let provider = provider.to_string();
                run_sqlite(store, move |store| store.delete_provider_token(&provider)).await
            }
            StoreBackend::Postgres(store) => store.delete_provider_token(provider).await,
        }
    }

    async fn list_provider_tokens(&self) -> StoreResult<Vec<ProviderToken>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                run_sqlite(store, |store| store.list_provider_tokens()).await
            }
            StoreBackend::Postgres(store) => store.list_provider_tokens().await,
        }
    }
}

#[async_trait::async_trait]
impl SecretsProviderStore for Store {
    async fn upsert_secrets_provider_config(
        &self,
        config: &SecretsProviderConfig,
    ) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let config = config.clone();
                run_sqlite(store, move |store| {
                    store.upsert_secrets_provider_config(&config)
                })
                .await
            }
            StoreBackend::Postgres(_) => Ok(()),
        }
    }

    async fn delete_secrets_provider_config(&self, provider: &str) -> StoreResult<()> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                let provider = provider.to_string();
                run_sqlite(store, move |store| {
                    store.delete_secrets_provider_config(&provider)
                })
                .await
            }
            StoreBackend::Postgres(_) => Ok(()),
        }
    }

    async fn list_secrets_provider_configs(&self) -> StoreResult<Vec<SecretsProviderConfig>> {
        match &self.backend {
            StoreBackend::Sqlite(store) => {
                run_sqlite(store, |store| store.list_secrets_provider_configs()).await
            }
            StoreBackend::Postgres(_) => Ok(Vec::new()),
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
    use super::{ExecutionStore, ForkRun, ForkRunStatus, RunTokenUsage, StorageConfig};
    use crate::lfd::id::LfdId;
    use crate::lfd::types::{
        ChatMemoryBlock, ExecutionProcess, ExecutionProcessStatus, LivePrState,
        LivePullRequestState, PullRequest, QueueBlock, QueueBlockReason, QueueMergeEvent, Repo,
        RepoEdge, RepoId, RepoWork, Run, RunStackStatus, RunStatus, Signal, Summary, Trigger, Wave,
        WaveCron, WaveMode, WaveStatus,
    };
    use std::env;
    use time::OffsetDateTime;

    fn make_wave(repo: &str) -> Wave {
        let id = LfdId::new();
        Wave {
            id: id.clone(),
            name: format!("wave-{id}"),
            mode: WaveMode::Loop,
            primary_flow: "ship-roadmap".to_string(),
            goal: "ship-roadmap".to_string(),
            metrics: Vec::new(),
            crons: Vec::new(),
            repos: vec![RepoWork {
                repo: repo.to_string(),
                worktree: String::new(),
                branch: String::new(),
                status: WaveStatus::Idle,
                iteration: 0,
                cycle_start_iteration: 0,
                position: 0,
            }],
            direction: vec!["focus".to_string()],
            area: vec!["src".to_string()],
            paused: false,
            created_at: Some(OffsetDateTime::now_utc()),
            workers: 1,
            parent_wave_id: None,
        }
    }

    fn make_run(wave: &Wave, status: RunStatus) -> Run {
        Run {
            id: LfdId::new(),
            wave_id: wave.id().clone(),
            repo: wave.repo().to_string(),
            flow: wave.primary_flow().clone(),
            task: None,
            direction: wave.direction().clone(),
            area: wave.area().clone(),
            iteration: 0,
            step_index: 0,
            status,
            worktree: "/repo".to_string(),
            branch: "main".to_string(),
            started_at: Some(OffsetDateTime::now_utc()),
            ended_at: None,
            error: None,
            flow_parents: Vec::new(),
            execution_cursor: None,
            activation_log_id: None,
            parent_run_id: None,
            parent_pr_number: None,
            stack_position: 0,
            stack_group_id: wave.id().to_string(),
            stack_status: RunStackStatus::Active,
            lineage_inferred: false,
            target_branch: "main".to_string(),
            repair_of: None,
            pr: None,
        }
    }

    async fn run_store_basic_suite(store: &super::Store) {
        let mut wave = make_wave("/repo");
        store.create_wave(&wave).await.expect("create wave");
        assert!(store.get_wave(wave.id()).await.expect("get wave").is_some());

        wave.set_status(WaveStatus::Paused);
        store.update_wave(&wave).await.expect("update wave");
        let loaded = store
            .get_wave(wave.id())
            .await
            .expect("get wave")
            .expect("wave exists");
        assert_eq!(loaded.status(), WaveStatus::Paused);
        assert_eq!(loaded.repos.len(), 1);
        assert_eq!(loaded.repos[0].repo, "/repo");
        assert_eq!(loaded.repos[0].status, WaveStatus::Paused);
        let loopable = store
            .list_loopable_waves()
            .await
            .expect("list loopable waves");
        assert!(loopable.iter().all(|candidate| candidate.id() != wave.id()));

        let cron = WaveCron {
            id: LfdId::new(),
            wave_id: wave.id().clone(),
            flow: "wave-polish".to_string(),
            schedule: "0 0 * * 1".to_string(),
            last_triggered_at: None,
            created_at: Some(OffsetDateTime::now_utc()),
        };
        store
            .create_wave_cron(&cron)
            .await
            .expect("create wave cron");
        let listed_crons = store
            .list_wave_crons(wave.id())
            .await
            .expect("list wave crons");
        assert_eq!(listed_crons.len(), 1);
        assert_eq!(listed_crons[0].flow, "wave-polish");
        store
            .update_wave_cron_last_triggered(&cron.id, Some(1234))
            .await
            .expect("update cron last_triggered_at");
        let updated_crons = store
            .list_wave_crons(wave.id())
            .await
            .expect("list updated wave crons");
        assert_eq!(updated_crons[0].last_triggered_at, Some(1234));

        let repo = Repo {
            path: "/tmp/repo".to_string(),
            repo_id: RepoId::parse("loopflowstudio/repo").expect("repo id"),
            name: "repo".to_string(),
            added_at: OffsetDateTime::now_utc(),
        };
        store.upsert_repo(&repo).await.expect("upsert repo");
        assert_eq!(
            store
                .list_repos()
                .await
                .expect("list repos")
                .first()
                .map(|entry| entry.path.clone()),
            Some(repo.path.clone())
        );
        assert!(store
            .get_repo(&repo.path)
            .await
            .expect("get repo")
            .is_some());
        store
            .delete_repo(&repo.path)
            .await
            .expect("delete repo registration");
        assert!(store
            .get_repo(&repo.path)
            .await
            .expect("repo removed")
            .is_none());

        let parent = Repo {
            path: "/tmp/parent".to_string(),
            repo_id: RepoId::parse("loopflowstudio/parent").expect("parent repo id"),
            name: "parent".to_string(),
            added_at: OffsetDateTime::now_utc(),
        };
        let child = Repo {
            path: "/tmp/child".to_string(),
            repo_id: RepoId::parse("loopflowstudio/child").expect("child repo id"),
            name: "child".to_string(),
            added_at: OffsetDateTime::now_utc(),
        };
        store
            .upsert_repo(&parent)
            .await
            .expect("upsert parent repo");
        store.upsert_repo(&child).await.expect("upsert child repo");
        assert!(store
            .get_repo_by_repo_id(&parent.repo_id)
            .await
            .expect("get by repo id")
            .is_some());
        store
            .add_edge(&RepoEdge {
                parent_repo_id: parent.repo_id.clone(),
                child_repo_id: child.repo_id.clone(),
            })
            .await
            .expect("add edge");
        assert_eq!(store.list_edges().await.expect("list edges").len(), 1);
        assert_eq!(
            store
                .children(&parent.repo_id)
                .await
                .expect("list children")
                .len(),
            1
        );
        assert_eq!(
            store
                .parents(&child.repo_id)
                .await
                .expect("list parents")
                .len(),
            1
        );
        store
            .delete_repo(&parent.path)
            .await
            .expect("delete parent repo");
        assert!(store
            .list_edges()
            .await
            .expect("list edges after delete")
            .is_empty());

        let source_wave = make_wave("/repo");
        store
            .create_wave(&source_wave)
            .await
            .expect("create source wave");

        let trigger = Trigger {
            id: LfdId::new(),
            wave_id: wave.id().clone(),
            source_wave_id: Some(source_wave.id().clone()),
            signal: Signal::Wave,
            flow: None,
            last_main_sha: None,
            last_triggered_at: None,
            created_at: Some(OffsetDateTime::now_utc()),
            enabled: true,
            max_iterations: None,
        };
        store
            .create_trigger(&trigger)
            .await
            .expect("create trigger");
        let triggers = store
            .list_triggers(Some(wave.id()))
            .await
            .expect("list triggers");
        assert_eq!(triggers.len(), 1);
        assert_eq!(triggers[0].source_wave_id.as_ref(), Some(source_wave.id()));

        let run = make_run(&wave, RunStatus::Running);
        store.create_run(&run).await.expect("create wave run");
        assert!(store
            .get_active_run(wave.id())
            .await
            .expect("get active")
            .is_some());

        let fork_run = ForkRun {
            id: LfdId::new(),
            run_id: run.id.clone(),
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
        let mut failed_run = run.clone();
        failed_run.status = RunStatus::Failed;
        store
            .update_run(&failed_run)
            .await
            .expect("update run to failed");
        let orphaned = store
            .list_orphaned_fork_runs()
            .await
            .expect("list orphaned fork runs");
        assert_eq!(orphaned.len(), 1);
        store.update_run(&run).await.expect("restore run status");
        let deleted_forks = store
            .delete_fork_runs(&run.id, 0)
            .await
            .expect("delete fork runs");
        assert_eq!(deleted_forks, 1);

        let execution_run = ExecutionProcess {
            id: LfdId::new(),
            step: "plan".to_string(),
            repo: "/repo".to_string(),
            worktree: "/repo".to_string(),
            run_id: Some(run.id.clone()),
            status: ExecutionProcessStatus::Waiting,
            started_at: Some(OffsetDateTime::now_utc()),
            ended_at: None,
            pid: None,
            container_id: None,
            agent: "claude-code".to_string(),
            run_mode: "auto".to_string(),
        };
        store
            .start_agent(&execution_run)
            .await
            .expect("start agent");
        assert!(store
            .get_waiting_agent_for_wave(wave.id())
            .await
            .expect("get waiting")
            .is_some());

        let summary = Summary {
            id: LfdId::new(),
            wave_id: wave.id().clone(),
            content: "summary".to_string(),
            source_hash: "abc".to_string(),
            token_budget: 100,
            agent: "claude-code".to_string(),
            created_at: Some(OffsetDateTime::now_utc()),
        };
        store
            .upsert_summary(&summary)
            .await
            .expect("upsert summary");
        assert!(store
            .get_summary(wave.id())
            .await
            .expect("get summary")
            .is_some());

        // Upsert replaces on same wave_id
        let updated_summary = Summary {
            id: LfdId::new(),
            wave_id: wave.id().clone(),
            content: "# Updated summary".to_string(),
            source_hash: "def456".to_string(),
            token_budget: 10000,
            agent: "claude-code".to_string(),
            created_at: Some(OffsetDateTime::now_utc()),
        };
        store
            .upsert_summary(&updated_summary)
            .await
            .expect("upsert updated summary");
        let reloaded = store
            .get_summary(wave.id())
            .await
            .expect("get updated summary")
            .expect("summary should exist");
        assert_eq!(reloaded.content, "# Updated summary");
        assert_eq!(reloaded.source_hash, "def456");

        // Chat memory block CRUD
        let block_a = ChatMemoryBlock {
            wave_id: wave.id().clone(),
            name: "preferences".to_string(),
            content: "Keep responses concise.".to_string(),
            position: 1,
            updated_at: Some(OffsetDateTime::now_utc()),
        };
        let block_b = ChatMemoryBlock {
            wave_id: wave.id().clone(),
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
            .list_chat_memory_blocks(wave.id())
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
            .list_chat_memory_blocks(wave.id())
            .await
            .expect("list blocks after update");
        let updated = blocks
            .iter()
            .find(|block| block.name == "preferences")
            .expect("updated memory block should exist");
        assert_eq!(updated.content, "Prefer bullet points.");
        assert_eq!(updated.position, 2);

        store
            .delete_chat_memory_block(wave.id(), "project-context")
            .await
            .expect("delete block");
        let blocks = store
            .list_chat_memory_blocks(wave.id())
            .await
            .expect("list blocks after delete");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].name, "preferences");

        store.delete_wave(wave.id()).await.expect("delete wave");
        assert!(store
            .get_wave(wave.id())
            .await
            .expect("get deleted wave")
            .is_none());
    }

    #[tokio::test]
    async fn sqlite_store_basic_suite() {
        let db_path = env::temp_dir().join(format!("lfd-test-{}.db", LfdId::new()));
        let config = StorageConfig::sqlite(db_path);
        let store = super::open_store(&config).await.expect("store should open");
        run_store_basic_suite(&store).await;
    }

    // A chord is a wave whose children point back at it via `parent_wave_id`.
    // `list_child_waves` returns those children (ordered, repos stitched); a
    // leaf wave returns none. This is the ancestry the WaveAgentTree needs.
    #[tokio::test]
    async fn sqlite_wave_ancestry_and_children() {
        let db_path = env::temp_dir().join(format!("lfd-test-{}.db", LfdId::new()));
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

    #[tokio::test]
    async fn token_usage_aggregates_by_wave_and_provider() {
        let db_path = env::temp_dir().join(format!("lfd-test-{}.db", LfdId::new()));
        let config = StorageConfig::sqlite(db_path);
        let store = super::open_store(&config).await.expect("store should open");

        let wave = make_wave("/repo-usage");
        store.create_wave(&wave).await.expect("create wave");
        let claude_run = make_run(&wave, RunStatus::Completed);
        let codex_run = make_run(&wave, RunStatus::Completed);
        store.create_run(&claude_run).await.expect("create run");
        store.create_run(&codex_run).await.expect("create run");

        store
            .record_run_usage(&RunTokenUsage {
                run_id: claude_run.id.clone(),
                wave: wave.id().clone(),
                repo: Some(wave.repo().to_string()),
                provider: "claude".to_string(),
                model: Some("claude-opus-4-8".to_string()),
                input_tokens: 100,
                output_tokens: 50,
                cache_read_tokens: 1500,
                recorded_at: OffsetDateTime::now_utc().unix_timestamp(),
            })
            .await
            .expect("record claude usage");
        store
            .record_run_usage(&RunTokenUsage {
                run_id: codex_run.id.clone(),
                wave: wave.id().clone(),
                repo: Some(wave.repo().to_string()),
                provider: "codex".to_string(),
                model: None,
                input_tokens: 200,
                output_tokens: 80,
                cache_read_tokens: 0,
                recorded_at: OffsetDateTime::now_utc().unix_timestamp(),
            })
            .await
            .expect("record codex usage");

        let report = store.aggregate_token_usage().await.expect("aggregate");

        assert_eq!(report.by_wave_provider.len(), 2);
        let claude = report
            .by_wave_provider
            .iter()
            .find(|row| row.provider == "claude")
            .expect("claude row");
        assert_eq!(claude.wave, *wave.id());
        assert_eq!(claude.input_tokens, 100);
        assert_eq!(claude.output_tokens, 50);
        assert_eq!(claude.cache_read_tokens, 1500);

        let codex = report
            .by_wave_provider
            .iter()
            .find(|row| row.provider == "codex")
            .expect("codex row");
        assert_eq!(codex.input_tokens, 200);
        assert_eq!(codex.output_tokens, 80);

        // Per-provider rollup matches the single-wave totals here.
        let claude_total = report
            .by_provider
            .iter()
            .find(|row| row.provider == "claude")
            .expect("claude total");
        assert_eq!(claude_total.input_tokens, 100);
        assert_eq!(claude_total.cache_read_tokens, 1500);
        let codex_total = report
            .by_provider
            .iter()
            .find(|row| row.provider == "codex")
            .expect("codex total");
        assert_eq!(codex_total.output_tokens, 80);

        // Re-recording the same run replaces its row rather than double-counting.
        store
            .record_run_usage(&RunTokenUsage {
                run_id: claude_run.id.clone(),
                wave: wave.id().clone(),
                repo: Some(wave.repo().to_string()),
                provider: "claude".to_string(),
                model: None,
                input_tokens: 10,
                output_tokens: 5,
                cache_read_tokens: 0,
                recorded_at: OffsetDateTime::now_utc().unix_timestamp(),
            })
            .await
            .expect("re-record claude usage");
        let report = store
            .aggregate_token_usage()
            .await
            .expect("aggregate again");
        let claude = report
            .by_wave_provider
            .iter()
            .find(|row| row.provider == "claude")
            .expect("claude row after replace");
        assert_eq!(claude.input_tokens, 10);
        assert_eq!(claude.cache_read_tokens, 0);
    }

    #[tokio::test]
    async fn token_usage_aggregates_by_repo_and_provider() {
        let db_path = env::temp_dir().join(format!("lfd-test-{}.db", LfdId::new()));
        let config = StorageConfig::sqlite(db_path);
        let store = super::open_store(&config).await.expect("store should open");

        // Two runs in one repo (different providers) plus a run in a second repo.
        let wave_a = make_wave("/repo-alpha");
        let wave_b = make_wave("/repo-beta");
        store.create_wave(&wave_a).await.expect("create wave a");
        store.create_wave(&wave_b).await.expect("create wave b");
        let alpha_claude = make_run(&wave_a, RunStatus::Completed);
        let alpha_codex = make_run(&wave_a, RunStatus::Completed);
        let beta_claude = make_run(&wave_b, RunStatus::Completed);
        store.create_run(&alpha_claude).await.expect("create run");
        store.create_run(&alpha_codex).await.expect("create run");
        store.create_run(&beta_claude).await.expect("create run");

        for (run, wave, provider, input, output, cache) in [
            (&alpha_claude, &wave_a, "claude", 100u64, 50u64, 10u64),
            (&alpha_codex, &wave_a, "codex", 200, 80, 0),
            (&beta_claude, &wave_b, "claude", 300, 100, 5),
        ] {
            store
                .record_run_usage(&RunTokenUsage {
                    run_id: run.id.clone(),
                    wave: wave.id().clone(),
                    repo: Some(wave.repo().to_string()),
                    provider: provider.to_string(),
                    model: None,
                    input_tokens: input,
                    output_tokens: output,
                    cache_read_tokens: cache,
                    recorded_at: OffsetDateTime::now_utc().unix_timestamp(),
                })
                .await
                .expect("record usage");
        }

        let report = store.aggregate_token_usage().await.expect("aggregate");

        // Three (repo, provider) groups: (alpha, claude), (alpha, codex), (beta, claude).
        assert_eq!(report.by_repo_provider.len(), 3);
        let find = |repo: &str, provider: &str| {
            report
                .by_repo_provider
                .iter()
                .find(|row| row.repo.as_deref() == Some(repo) && row.provider == provider)
                .expect("repo/provider row")
        };
        let alpha_claude_row = find("/repo-alpha", "claude");
        assert_eq!(alpha_claude_row.input_tokens, 100);
        assert_eq!(alpha_claude_row.output_tokens, 50);
        assert_eq!(alpha_claude_row.cache_read_tokens, 10);
        let alpha_codex_row = find("/repo-alpha", "codex");
        assert_eq!(alpha_codex_row.input_tokens, 200);
        let beta_claude_row = find("/repo-beta", "claude");
        assert_eq!(beta_claude_row.input_tokens, 300);
        assert_eq!(beta_claude_row.cache_read_tokens, 5);

        // Per-provider rollup sums across repos: claude spans both repos.
        let claude_total = report
            .by_provider
            .iter()
            .find(|row| row.provider == "claude")
            .expect("claude total");
        assert_eq!(claude_total.input_tokens, 400);
        assert_eq!(claude_total.output_tokens, 150);
        assert_eq!(claude_total.cache_read_tokens, 15);
    }

    #[tokio::test]
    async fn sqlite_health_check_succeeds() {
        let db_path = env::temp_dir().join(format!("lfd-test-{}.db", LfdId::new()));
        let config = StorageConfig::sqlite(db_path);
        let store = super::open_store(&config).await.expect("store should open");

        store.health_check().await.expect("sqlite health check");
    }

    // When lfd restarts, `fail_orphaned_runs` marks in-flight runs as Failed.
    // Without also resetting the wave's own status, the wave stays visually
    // "running" — the Concerto sidebar and buttons stay disabled forever even
    // though no executor is attached. Cover the reset here.
    #[tokio::test]
    async fn fail_orphaned_runs_resets_stuck_wave_status() {
        let db_path = env::temp_dir().join(format!("lfd-test-{}.db", LfdId::new()));
        let config = StorageConfig::sqlite(db_path);
        let store = super::open_store(&config).await.expect("store should open");

        // Wave that was actively running when lfd died.
        let mut running_wave = make_wave("/repo-stuck-running");
        running_wave.set_status(WaveStatus::Running);
        store.create_wave(&running_wave).await.expect("create wave");
        let mut running_run = make_run(&running_wave, RunStatus::Running);
        store.create_run(&running_run).await.expect("create run");

        // Wave that was at an interactive waitpoint.
        let mut waiting_wave = make_wave("/repo-stuck-waiting");
        waiting_wave.set_status(WaveStatus::Waiting);
        store
            .create_wave(&waiting_wave)
            .await
            .expect("create waiting wave");
        let mut waiting_run = make_run(&waiting_wave, RunStatus::Waiting);
        store
            .create_run(&waiting_run)
            .await
            .expect("create waiting run");

        // Paused wave with no active run — must be left alone.
        let mut paused_wave = make_wave("/repo-paused");
        paused_wave.set_status(WaveStatus::Paused);
        store
            .create_wave(&paused_wave)
            .await
            .expect("create paused wave");

        let _ = store.fail_orphaned_runs().await.expect("fail orphans");

        running_run = store
            .get_run(&running_run.id)
            .await
            .expect("get run")
            .expect("run exists");
        assert_eq!(running_run.status, RunStatus::Failed);
        waiting_run = store
            .get_run(&waiting_run.id)
            .await
            .expect("get waiting run")
            .expect("run exists");
        assert_eq!(waiting_run.status, RunStatus::Failed);

        let running_after = store
            .get_wave(running_wave.id())
            .await
            .expect("get wave")
            .expect("wave exists");
        assert_eq!(
            running_after.status(),
            WaveStatus::Idle,
            "wave whose run was orphaned should be reset to Idle"
        );
        let waiting_after = store
            .get_wave(waiting_wave.id())
            .await
            .expect("get waiting wave")
            .expect("wave exists");
        assert_eq!(
            waiting_after.status(),
            WaveStatus::Idle,
            "waiting-state wave should also be reset to Idle"
        );
        let paused_after = store
            .get_wave(paused_wave.id())
            .await
            .expect("get paused wave")
            .expect("wave exists");
        assert_eq!(
            paused_after.status(),
            WaveStatus::Paused,
            "paused wave must keep its status across orphan cleanup"
        );
    }

    #[tokio::test]
    #[ignore] // requires DATABASE_URL
    async fn postgres_store_parity() {
        let url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let store = super::open_store(&StorageConfig::postgres(url))
            .await
            .expect("connect");
        run_store_basic_suite(&store).await;
    }

    #[tokio::test]
    async fn provider_token_round_trip() {
        let db_path = env::temp_dir().join(format!("lfd-test-{}.db", LfdId::new()));
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
        let db_path = env::temp_dir().join(format!("lfd-test-{}.db", LfdId::new()));
        let config = StorageConfig::sqlite(db_path.clone());
        let store = super::open_store(&config).await.expect("store should open");
        let token = super::ProviderToken {
            provider: "github".to_string(),
            access_token: "gho_secret_access".to_string(),
            refresh_token: Some("ghr_secret_refresh".to_string()),
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
        let db_path = env::temp_dir().join(format!("lfd-test-{}.db", LfdId::new()));
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

    #[tokio::test]
    async fn active_run_excludes_failed_runs() {
        let db_path = env::temp_dir().join(format!("lfd-test-{}.db", LfdId::new()));
        let config = StorageConfig::sqlite(db_path);
        let store = super::open_store(&config).await.expect("store should open");

        let wave = make_wave("/repo-active");
        store.create_wave(&wave).await.expect("create wave");

        let mut run = make_run(&wave, RunStatus::Running);
        store.create_run(&run).await.expect("create running run");
        assert!(store
            .get_active_run(wave.id())
            .await
            .expect("active run")
            .is_some());

        run.status = RunStatus::Failed;
        run.error = Some("failed".to_string());
        run.ended_at = Some(OffsetDateTime::now_utc());
        store.update_run(&run).await.expect("update run");
        assert!(store
            .get_active_run(wave.id())
            .await
            .expect("active after fail")
            .is_none());
        assert_eq!(
            store
                .get_latest_run(wave.id())
                .await
                .expect("latest run")
                .expect("run exists")
                .status,
            RunStatus::Failed
        );

        let mut ci_fix = make_run(&wave, RunStatus::Running);
        ci_fix.flow = "ci-fix".to_string();
        store.create_run(&ci_fix).await.expect("create ci-fix run");
        assert!(store
            .get_active_run(wave.id())
            .await
            .expect("active with ci-fix")
            .is_some());

        store.delete_wave(wave.id()).await.expect("delete wave");
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
                Run {
                    id: LfdId::new(),
                    wave_id: wave.id().clone(),
                    repo: wave.repo().to_string(),
                    flow: wave.primary_flow().clone(),
                    task: None,
                    direction: wave.direction().clone(),
                    area: wave.area().clone(),
                    iteration,
                    step_index: 0,
                    status: RunStatus::Completed,
                    worktree: format!("/repo/live-pr/{iteration}"),
                    branch: format!("feature-{pr_number}"),
                    started_at: Some(OffsetDateTime::now_utc()),
                    ended_at: Some(OffsetDateTime::now_utc()),
                    error: None,
                    flow_parents: Vec::new(),
                    execution_cursor: None,
                    activation_log_id: None,
                    parent_run_id,
                    parent_pr_number,
                    stack_position: iteration.saturating_sub(1),
                    stack_group_id: wave.id().to_string(),
                    stack_status: RunStackStatus::Active,
                    lineage_inferred: false,
                    target_branch: "main".to_string(),
                    repair_of: None,
                    pr: Some(PullRequest {
                        url: format!("https://example.test/pr/{pr_number}"),
                        number: Some(pr_number),
                        state: Some("open".to_string()),
                        title: Some(format!("run-{iteration}")),
                        branch: Some(format!("feature-{pr_number}")),
                    }),
                }
            };

        let make_pr_state =
            |pr_number: u32, state: LivePrState, head_sha: &str| LivePullRequestState {
                repo_id: wave.repo().to_string(),
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
        store.create_run(&run1).await.expect("create run1");

        let run2 = make_run(2, Some(run1.id.clone()), Some(101));
        store.create_run(&run2).await.expect("create run2");

        let first_pending = store
            .find_next_unmerged_run(wave.id())
            .await
            .expect("find first unmerged");
        assert_eq!(first_pending.map(|run| run.id), Some(run1.id.clone()));

        store
            .upsert_live_pr_state(&make_pr_state(101, LivePrState::Merged, "sha-101"))
            .await
            .expect("upsert pr 101 merged");
        let second_pending = store
            .find_next_unmerged_run(wave.id())
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
                .find_next_unmerged_run(wave.id())
                .await
                .expect("find unmerged");
            assert_eq!(pending.map(|run| run.id), expected);
        }

        store.delete_wave(wave.id()).await.expect("delete wave");
    }

    #[tokio::test]
    async fn queue_block_and_merge_event_crud() {
        let db_path = env::temp_dir().join(format!("lfd-test-{}.db", LfdId::new()));
        let config = StorageConfig::sqlite(db_path);
        let store = super::open_store(&config).await.expect("store should open");

        let wave = make_wave("/repo-queue");
        store.create_wave(&wave).await.expect("create wave");

        let mut run = make_run(&wave, RunStatus::Completed);
        run.pr = Some(PullRequest {
            url: "https://example.test/pr/42".to_string(),
            number: Some(42),
            state: Some("open".to_string()),
            title: Some("feature".to_string()),
            branch: Some("feature-42".to_string()),
        });
        run.parent_pr_number = Some(42);
        store.create_run(&run).await.expect("create run");

        let block = QueueBlock {
            wave_id: wave.id().clone(),
            run_id: run.id.clone(),
            reason: QueueBlockReason::RebaseConflict,
            attempted_at: OffsetDateTime::now_utc(),
            conflict_files: vec!["src/lib.rs".to_string()],
            error: Some("merge failed".to_string()),
        };
        store
            .upsert_queue_block(&block)
            .await
            .expect("upsert block");
        let blocks = store
            .list_queue_blocks(wave.id())
            .await
            .expect("list blocks");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].reason, QueueBlockReason::RebaseConflict);
        let deleted = store
            .delete_queue_block(wave.id(), &run.id)
            .await
            .expect("delete block");
        assert_eq!(deleted, 1);

        let merge_event = QueueMergeEvent {
            wave_id: wave.id().clone(),
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

        store.delete_wave(wave.id()).await.expect("delete wave");
    }
}
