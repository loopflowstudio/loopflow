//! Persistence as shared infrastructure: the machine's registry, written and
//! read by `lf` directly — `lfd` is one more client, not the owner. This
//! module owns the sqlite registry, schema migrations, and registry API
//! ([`Store`] and its per-domain traits).
//!
//! The persisted domain types still live in `crate::lfd::types` for now; the
//! type split is a later, non-mechanical skill.
//!

use std::path::PathBuf;
use std::sync::Arc;

use crate::lfd::id::LfdId;
use crate::lfd::types::{
    AttentionItem, AttentionKind, AttentionStatus, ChatMemoryBlock, ChatMessage, LivePrState,
    LivePullRequestState, QueueBlock, Repo, RepoEdge, RepoId, Run, RunStackStatus, Session,
    SessionStatus, Summary, Wave,
};

pub mod catalog;
pub mod migrations;
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

/// One row of the machine-grain run ledger (`run_events`): a lifecycle event
/// for a run, flow, or skill, written directly by `lf` (and by `lfd`) into the
/// local store. Token/cost fields are populated on terminal run events when
/// the stream reported them.
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

/// Summed token usage and cost for one (repo, provider) pair — the finest
/// grain the ledger aggregates, and the only one queried: every coarser
/// rollup is a fold of these rows. `repo` and `provider` are optional because
/// rows recorded before those dimensions existed carry neither.
#[derive(Debug, Clone, PartialEq)]
pub struct RepoProviderUsage {
    pub repo: Option<String>,
    pub provider: Option<String>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cost_usd: f64,
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

    /// A chord's contents: the waves whose `parent_wave_id` is `parent`,
    /// ordered by creation.
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
        WaveStateStore::create_wave(self, wave).await
    }

    pub async fn update_wave(&self, wave: &Wave) -> StoreResult<()> {
        WaveStateStore::update_wave(self, wave).await
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

    pub async fn list_runs_active_or_ended_since(
        &self,
        ended_since: time::OffsetDateTime,
    ) -> StoreResult<Vec<Run>> {
        WaveStateStore::list_runs_active_or_ended_since(self, ended_since).await
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

    /// This wave's live sessions plus sessions completed at or after
    /// `completed_since` (unix seconds) — a poller's working set, bounded
    /// regardless of how much terminal history the wave accumulates.
    pub async fn list_recent_control_sessions(
        &self,
        wave_id: &LfdId,
        completed_since: i64,
    ) -> StoreResult<Vec<Session>> {
        ControlSessionStore::list_recent_sessions(self, wave_id, completed_since).await
    }

    /// The wave's live brain, if any: a non-terminal `WaveAgent` session —
    /// either lfd-launched (`POST /waves/{id}/run`) or a self-registered
    /// `lf wave` server. One-brain enforcement (run_wave idempotency, the
    /// loop-ticker skip, wave-server registration conflicts) keys on this
    /// single fact.
    pub async fn live_wave_agent_session(&self, wave_id: &LfdId) -> StoreResult<Option<Session>> {
        let sessions = self
            .list_control_sessions(
                Some(wave_id),
                Some(crate::lfd::types::LIVE_SESSION_STATUSES),
            )
            .await?;
        Ok(sessions
            .into_iter()
            .find(|session| session.session_use == crate::lfd::types::SessionUse::WaveAgent))
    }

    /// Record a session in the run registry. The db IS the registry: `lf`
    /// writes its own row here directly — self-registered flow runs, the
    /// `lf wave` server's WaveAgent row, placed `lf` runs — no
    /// daemon in the path. The writer later marks the row terminal via
    /// [`Store::update_control_session`].
    pub async fn register_session(&self, session: &Session) -> StoreResult<()> {
        ControlSessionStore::create_session(self, session).await
    }

    /// Live sessions grouped under one worktree, keyed by the worktree
    /// directory's basename (`<repo>.<wave>`, `<repo>.<wave>.<id>`, ...):
    /// everything currently running in that tree, whoever launched it.
    pub async fn active_sessions_by_worktree(
        &self,
        worktree_name: &str,
    ) -> StoreResult<Vec<Session>> {
        let sessions = self
            .list_control_sessions(None, Some(crate::lfd::types::LIVE_SESSION_STATUSES))
            .await?;
        Ok(sessions
            .into_iter()
            .filter(|session| {
                std::path::Path::new(&session.cwd)
                    .file_name()
                    .and_then(|name| name.to_str())
                    == Some(worktree_name)
            })
            .collect())
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
    async fn children_of(&self, parent: &LfdId) -> StoreResult<Vec<Wave>>;
    async fn get_wave(&self, wave_id: &LfdId) -> StoreResult<Option<Wave>>;
    async fn get_wave_by_name(&self, name: &str) -> StoreResult<Option<Wave>>;
    async fn create_wave(&self, wave: &Wave) -> StoreResult<()>;
    async fn update_wave(&self, wave: &Wave) -> StoreResult<()>;
    async fn delete_wave(&self, wave_id: &LfdId) -> StoreResult<()>;

    async fn list_runs(&self, wave_id: Option<&LfdId>, limit: Option<u32>)
        -> StoreResult<Vec<Run>>;
    /// Non-terminal runs plus runs that ended at or after `ended_since` —
    /// the push bridge's working set, so it never scans the whole terminal
    /// history.
    async fn list_runs_active_or_ended_since(
        &self,
        ended_since: time::OffsetDateTime,
    ) -> StoreResult<Vec<Run>>;
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

    async fn get_summary(&self, wave_id: &LfdId) -> StoreResult<Option<Summary>>;
    async fn upsert_summary(&self, summary: &Summary) -> StoreResult<()>;

    async fn list_chat_memory_blocks(&self, wave_id: &LfdId) -> StoreResult<Vec<ChatMemoryBlock>>;
    async fn upsert_chat_memory_block(&self, block: &ChatMemoryBlock) -> StoreResult<()>;
    async fn delete_chat_memory_block(&self, wave_id: &LfdId, name: &str) -> StoreResult<()>;

    async fn list_chat_messages(&self, wave_id: &LfdId) -> StoreResult<Vec<ChatMessage>>;
    async fn create_chat_message(&self, message: &ChatMessage) -> StoreResult<()>;
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
    async fn list_recent_sessions(
        &self,
        wave_id: &LfdId,
        completed_since: i64,
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

#[async_trait::async_trait]
pub trait StoreAdmin: Send + Sync {
    async fn health_check(&self) -> StoreResult<()>;
    async fn schema_version(&self) -> StoreResult<String>;
}

#[async_trait::async_trait]
impl WaveStateStore for Store {
    async fn list_waves(&self, repo: Option<&str>) -> StoreResult<Vec<Wave>> {
        let waves = {
            let repo = repo.map(str::to_string);
            run_sqlite(&self.sqlite, move |store| store.list_waves(repo.as_deref())).await
        }?;
        Ok(waves)
    }

    async fn children_of(&self, parent: &LfdId) -> StoreResult<Vec<Wave>> {
        let parent = parent.clone();
        run_sqlite(&self.sqlite, move |store| store.list_child_waves(&parent)).await
    }

    async fn get_wave(&self, wave_id: &LfdId) -> StoreResult<Option<Wave>> {
        let wave_id = wave_id.clone();
        run_sqlite(&self.sqlite, move |store| store.get_wave(&wave_id)).await
    }

    async fn get_wave_by_name(&self, name: &str) -> StoreResult<Option<Wave>> {
        let name = name.to_string();
        run_sqlite(&self.sqlite, move |store| store.get_wave_by_name(&name)).await
    }

    async fn create_wave(&self, wave: &Wave) -> StoreResult<()> {
        {
            let wave = wave.clone();
            run_sqlite(&self.sqlite, move |store| store.create_wave(&wave)).await
        }
    }

    async fn update_wave(&self, wave: &Wave) -> StoreResult<()> {
        {
            let wave = wave.clone();
            run_sqlite(&self.sqlite, move |store| store.update_wave(&wave)).await
        }
    }

    async fn delete_wave(&self, wave_id: &LfdId) -> StoreResult<()> {
        {
            let wave_id = wave_id.clone();
            run_sqlite(&self.sqlite, move |store| store.delete_wave(&wave_id)).await
        }
    }

    async fn list_runs(
        &self,
        wave_id: Option<&LfdId>,
        limit: Option<u32>,
    ) -> StoreResult<Vec<Run>> {
        {
            let wave_id = wave_id.cloned();
            run_sqlite(&self.sqlite, move |store| {
                store.list_runs(wave_id.as_ref(), limit)
            })
            .await
        }
    }

    async fn list_runs_active_or_ended_since(
        &self,
        ended_since: time::OffsetDateTime,
    ) -> StoreResult<Vec<Run>> {
        {
            run_sqlite(&self.sqlite, move |store| {
                store.list_runs_active_or_ended_since(ended_since)
            })
            .await
        }
    }

    async fn get_run(&self, run_id: &LfdId) -> StoreResult<Option<Run>> {
        {
            let run_id = run_id.clone();
            run_sqlite(&self.sqlite, move |store| store.get_run(&run_id)).await
        }
    }

    async fn get_active_run(&self, wave_id: &LfdId) -> StoreResult<Option<Run>> {
        {
            let wave_id = wave_id.clone();
            run_sqlite(&self.sqlite, move |store| store.get_active_run(&wave_id)).await
        }
    }

    async fn count_active_runs(&self, wave_id: &LfdId) -> StoreResult<u32> {
        {
            let wave_id = wave_id.clone();
            run_sqlite(&self.sqlite, move |store| store.count_active_runs(&wave_id)).await
        }
    }

    async fn get_latest_run(&self, wave_id: &LfdId) -> StoreResult<Option<Run>> {
        {
            let wave_id = wave_id.clone();
            run_sqlite(&self.sqlite, move |store| store.get_latest_run(&wave_id)).await
        }
    }

    async fn create_run(&self, run: &Run) -> StoreResult<()> {
        {
            let run = run.clone();
            run_sqlite(&self.sqlite, move |store| store.create_run(&run)).await
        }
    }

    async fn update_run(&self, run: &Run) -> StoreResult<()> {
        {
            let run = run.clone();
            run_sqlite(&self.sqlite, move |store| store.update_run(&run)).await
        }
    }

    async fn list_stack_runs(&self, wave_id: &LfdId) -> StoreResult<Vec<Run>> {
        {
            let wave_id = wave_id.clone();
            run_sqlite(&self.sqlite, move |store| store.list_stack_runs(&wave_id)).await
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
        {
            let repo_id = repo_id.to_string();
            run_sqlite(&self.sqlite, move |store| {
                store.get_live_pr_state(&repo_id, pr_number)
            })
            .await
        }
    }

    async fn upsert_live_pr_state(&self, state: &LivePullRequestState) -> StoreResult<()> {
        {
            let state = state.clone();
            run_sqlite(&self.sqlite, move |store| {
                store.upsert_live_pr_state(&state)
            })
            .await
        }
    }

    async fn list_attention_items(
        &self,
        status: Option<AttentionStatus>,
        kind: Option<AttentionKind>,
    ) -> StoreResult<Vec<AttentionItem>> {
        {
            run_sqlite(&self.sqlite, move |store| {
                store.list_attention_items(status, kind)
            })
            .await
        }
    }

    async fn get_attention_item(&self, attention_id: &LfdId) -> StoreResult<Option<AttentionItem>> {
        {
            let attention_id = attention_id.clone();
            run_sqlite(&self.sqlite, move |store| {
                store.get_attention_item(&attention_id)
            })
            .await
        }
    }

    async fn find_attention_item_for_run(
        &self,
        run_id: &LfdId,
        kind: AttentionKind,
    ) -> StoreResult<Option<AttentionItem>> {
        {
            let run_id = run_id.clone();
            run_sqlite(&self.sqlite, move |store| {
                store.find_attention_item_for_run(&run_id, kind)
            })
            .await
        }
    }

    async fn upsert_attention_item(&self, item: &AttentionItem) -> StoreResult<()> {
        {
            let item = item.clone();
            run_sqlite(&self.sqlite, move |store| {
                store.upsert_attention_item(&item)
            })
            .await
        }
    }

    async fn delete_attention_item(&self, attention_id: &LfdId) -> StoreResult<u32> {
        {
            let attention_id = attention_id.clone();
            run_sqlite(&self.sqlite, move |store| {
                store.delete_attention_item(&attention_id)
            })
            .await
        }
    }

    async fn list_queue_blocks(&self, wave_id: &LfdId) -> StoreResult<Vec<QueueBlock>> {
        {
            let wave_id = wave_id.clone();
            run_sqlite(&self.sqlite, move |store| store.list_queue_blocks(&wave_id)).await
        }
    }

    async fn upsert_queue_block(&self, block: &QueueBlock) -> StoreResult<()> {
        {
            let block = block.clone();
            run_sqlite(&self.sqlite, move |store| store.upsert_queue_block(&block)).await
        }
    }

    async fn delete_queue_block(&self, wave_id: &LfdId, run_id: &LfdId) -> StoreResult<u32> {
        {
            let wave_id = wave_id.clone();
            let run_id = run_id.clone();
            run_sqlite(&self.sqlite, move |store| {
                store.delete_queue_block(&wave_id, &run_id)
            })
            .await
        }
    }

    async fn get_summary(&self, wave_id: &LfdId) -> StoreResult<Option<Summary>> {
        {
            let wave_id = wave_id.clone();
            run_sqlite(&self.sqlite, move |store| store.get_summary(&wave_id)).await
        }
    }

    async fn upsert_summary(&self, summary: &Summary) -> StoreResult<()> {
        {
            let summary = summary.clone();
            run_sqlite(&self.sqlite, move |store| store.upsert_summary(&summary)).await
        }
    }

    async fn list_chat_memory_blocks(&self, wave_id: &LfdId) -> StoreResult<Vec<ChatMemoryBlock>> {
        {
            let wave_id = wave_id.clone();
            run_sqlite(&self.sqlite, move |store| {
                store.list_chat_memory_blocks(&wave_id)
            })
            .await
        }
    }

    async fn upsert_chat_memory_block(&self, block: &ChatMemoryBlock) -> StoreResult<()> {
        {
            let block = block.clone();
            run_sqlite(&self.sqlite, move |store| {
                store.upsert_chat_memory_block(&block)
            })
            .await
        }
    }

    async fn delete_chat_memory_block(&self, wave_id: &LfdId, name: &str) -> StoreResult<()> {
        {
            let wave_id = wave_id.clone();
            let name = name.to_string();
            run_sqlite(&self.sqlite, move |store| {
                store.delete_chat_memory_block(&wave_id, &name)
            })
            .await
        }
    }

    async fn list_chat_messages(&self, wave_id: &LfdId) -> StoreResult<Vec<ChatMessage>> {
        {
            let wave_id = wave_id.clone();
            run_sqlite(&self.sqlite, move |store| {
                store.list_chat_messages(&wave_id)
            })
            .await
        }
    }

    async fn create_chat_message(&self, message: &ChatMessage) -> StoreResult<()> {
        {
            let message = message.clone();
            run_sqlite(&self.sqlite, move |store| {
                store.create_chat_message(&message)
            })
            .await
        }
    }
}

#[async_trait::async_trait]
impl RepoStore for Store {
    async fn list_repos(&self) -> StoreResult<Vec<Repo>> {
        {
            run_sqlite(&self.sqlite, |store| store.list_repos()).await
        }
    }

    async fn get_repo(&self, path: &str) -> StoreResult<Option<Repo>> {
        {
            let path = path.to_string();
            run_sqlite(&self.sqlite, move |store| store.get_repo(&path)).await
        }
    }

    async fn upsert_repo(&self, repo: &Repo) -> StoreResult<()> {
        {
            let repo = repo.clone();
            run_sqlite(&self.sqlite, move |store| store.upsert_repo(&repo)).await
        }
    }

    async fn delete_repo(&self, path: &str) -> StoreResult<()> {
        {
            let path = path.to_string();
            run_sqlite(&self.sqlite, move |store| store.delete_repo(&path)).await
        }
    }

    async fn get_repo_by_repo_id(&self, repo_id: &RepoId) -> StoreResult<Option<Repo>> {
        {
            let repo_id = repo_id.clone();
            run_sqlite(&self.sqlite, move |store| {
                store.get_repo_by_repo_id(&repo_id)
            })
            .await
        }
    }

    async fn list_edges(&self) -> StoreResult<Vec<RepoEdge>> {
        {
            run_sqlite(&self.sqlite, |store| store.list_edges()).await
        }
    }

    async fn add_edge(&self, edge: &RepoEdge) -> StoreResult<()> {
        {
            let edge = edge.clone();
            run_sqlite(&self.sqlite, move |store| store.add_edge(&edge)).await
        }
    }

    async fn remove_edge(&self, parent_id: &RepoId, child_id: &RepoId) -> StoreResult<()> {
        {
            let parent_id = parent_id.clone();
            let child_id = child_id.clone();
            run_sqlite(&self.sqlite, move |store| {
                store.remove_edge(&parent_id, &child_id)
            })
            .await
        }
    }

    async fn children(&self, repo_id: &RepoId) -> StoreResult<Vec<Repo>> {
        {
            let repo_id = repo_id.clone();
            run_sqlite(&self.sqlite, move |store| store.children(&repo_id)).await
        }
    }

    async fn parents(&self, repo_id: &RepoId) -> StoreResult<Vec<Repo>> {
        {
            let repo_id = repo_id.clone();
            run_sqlite(&self.sqlite, move |store| store.parents(&repo_id)).await
        }
    }
}

#[async_trait::async_trait]
impl ExecutionStore for Store {
    async fn list_fork_runs(&self, run_id: &LfdId, step_index: u32) -> StoreResult<Vec<ForkRun>> {
        {
            let run_id = run_id.clone();
            run_sqlite(&self.sqlite, move |store| {
                store.list_fork_runs(&run_id, step_index)
            })
            .await
        }
    }

    async fn upsert_fork_run(&self, fork_run: &ForkRun) -> StoreResult<()> {
        {
            let fork_run = fork_run.clone();
            run_sqlite(&self.sqlite, move |store| store.upsert_fork_run(&fork_run)).await
        }
    }

    async fn list_orphaned_fork_runs(&self) -> StoreResult<Vec<ForkRun>> {
        {
            run_sqlite(&self.sqlite, |store| store.list_orphaned_fork_runs()).await
        }
    }

    async fn delete_fork_runs(&self, run_id: &LfdId, step_index: u32) -> StoreResult<u32> {
        {
            let run_id = run_id.clone();
            run_sqlite(&self.sqlite, move |store| {
                store.delete_fork_runs(&run_id, step_index)
            })
            .await
        }
    }

    async fn fail_orphaned_runs(&self) -> StoreResult<u32> {
        {
            run_sqlite(&self.sqlite, |store| store.fail_orphaned_runs()).await
        }
    }
}

#[async_trait::async_trait]
impl ControlSessionStore for Store {
    async fn create_session(&self, session: &Session) -> StoreResult<()> {
        {
            let session = session.clone();
            run_sqlite(&self.sqlite, move |store| {
                store.create_control_session(&session)
            })
            .await
        }
    }

    async fn get_session(&self, session_id: &LfdId) -> StoreResult<Option<Session>> {
        {
            let session_id = session_id.clone();
            run_sqlite(&self.sqlite, move |store| {
                store.get_control_session(&session_id)
            })
            .await
        }
    }

    async fn list_sessions(
        &self,
        wave_id: Option<&LfdId>,
        statuses: Option<&[SessionStatus]>,
    ) -> StoreResult<Vec<Session>> {
        {
            let wave_id = wave_id.cloned();
            let statuses = statuses.map(|values| values.to_vec());
            run_sqlite(&self.sqlite, move |store| {
                store.list_control_sessions(wave_id.as_ref(), statuses.as_deref())
            })
            .await
        }
    }

    async fn list_recent_sessions(
        &self,
        wave_id: &LfdId,
        completed_since: i64,
    ) -> StoreResult<Vec<Session>> {
        {
            let wave_id = wave_id.clone();
            run_sqlite(&self.sqlite, move |store| {
                store.list_recent_control_sessions(&wave_id, completed_since)
            })
            .await
        }
    }

    async fn get_active_session_for_run(&self, run_id: &LfdId) -> StoreResult<Option<Session>> {
        {
            let run_id = run_id.clone();
            run_sqlite(&self.sqlite, move |store| {
                store.get_active_control_session_for_run(&run_id)
            })
            .await
        }
    }

    async fn update_session(&self, session: &Session) -> StoreResult<()> {
        {
            let session = session.clone();
            run_sqlite(&self.sqlite, move |store| {
                store.update_control_session(&session)
            })
            .await
        }
    }
}

#[async_trait::async_trait]
impl TokenStore for Store {
    async fn get_provider_token(&self, provider: &str) -> StoreResult<Option<ProviderToken>> {
        {
            let provider = provider.to_string();
            run_sqlite(&self.sqlite, move |store| {
                store.get_provider_token(&provider)
            })
            .await
        }
    }

    async fn upsert_provider_token(&self, token: &ProviderToken) -> StoreResult<()> {
        {
            let token = token.clone();
            run_sqlite(&self.sqlite, move |store| {
                store.upsert_provider_token(&token)
            })
            .await
        }
    }

    async fn delete_provider_token(&self, provider: &str) -> StoreResult<()> {
        {
            let provider = provider.to_string();
            run_sqlite(&self.sqlite, move |store| {
                store.delete_provider_token(&provider)
            })
            .await
        }
    }

    async fn list_provider_tokens(&self) -> StoreResult<Vec<ProviderToken>> {
        {
            run_sqlite(&self.sqlite, |store| store.list_provider_tokens()).await
        }
    }
}

#[async_trait::async_trait]
impl StoreAdmin for Store {
    async fn health_check(&self) -> StoreResult<()> {
        {
            run_sqlite(&self.sqlite, |store| store.health_check()).await
        }
    }

    async fn schema_version(&self) -> StoreResult<String> {
        {
            run_sqlite(&self.sqlite, |store| store.schema_version()).await
        }
    }
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
    let cfg = crate::lfd::storage_config_from_env().ok()?;
    let StorageConfig::Sqlite { path } = &cfg;
    if !path.exists() {
        return None;
    }
    // lf-direct openers can meet a db created by an older lfd (the daemon
    // migrates only at its own boot). Apply pending migrations here so a
    // direct writer never hits schema drift; versioned migrations make a
    // concurrent second applier a no-op, and sqlite locking serializes
    // them. A failed migration means the db is unusable for us: warn and
    // report "not instrumented" rather than limping on a wrong schema.
    let conn = rusqlite::Connection::open(path).ok()?;
    if let Err(err) = migrations::apply_sqlite(&conn) {
        tracing::warn!(?path, %err, "registry store migration failed; running uninstrumented");
        return None;
    }
    open_store(&cfg).await.ok()
}

pub async fn migrate_store(cfg: &StorageConfig, status_only: bool) -> StoreResult<String> {
    let StorageConfig::Sqlite { path } = cfg;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| StoreError::InvalidData(format!("failed to create db dir: {err}")))?;
    }
    let conn = rusqlite::Connection::open(path)?;
    if !status_only {
        migrations::apply_sqlite(&conn)?;
    }
    migrations::latest_version_sqlite(&conn)
}
pub type SharedStore = Arc<Store>;

#[cfg(test)]
mod tests {
    use super::sqlite::SqliteStore;
    use super::{ExecutionStore, ForkRun, ForkRunStatus, RunEventRow, StorageConfig};
    use crate::lfd::id::LfdId;
    use crate::lfd::types::{
        ChatMemoryBlock, LivePrState, LivePullRequestState, PullRequest, QueueBlock,
        QueueBlockReason, Repo, RepoEdge, RepoId, Run, RunStackStatus, RunStatus, Summary, Wave,
        WaveStatus, DEFAULT_WAVE_FLOW,
    };
    use std::env;
    use time::OffsetDateTime;

    fn make_wave(repo: &str) -> Wave {
        let id = LfdId::new();
        Wave {
            id: id.clone(),
            name: format!("wave-{id}"),
            goal: "ship-roadmap".to_string(),
            metrics: Vec::new(),
            repo: repo.to_string(),
            worktree: String::new(),
            branch: String::new(),
            status: WaveStatus::Idle,
            iteration: 0,
            cycle_start_iteration: 0,
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
            flow: DEFAULT_WAVE_FLOW.to_string(),
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
        assert_eq!(loaded.repo(), "/repo");
        assert_eq!(loaded.status, WaveStatus::Paused);

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

    /// Attach usage to a row: (input, output, cache_read) tokens and cost.
    fn with_usage(
        mut row: RunEventRow,
        wave: Option<&str>,
        provider: &str,
        tokens: (i64, i64, i64),
        cost: f64,
    ) -> RunEventRow {
        row.wave = wave.map(str::to_string);
        row.provider = Some(provider.to_string());
        row.input_tokens = Some(tokens.0);
        row.output_tokens = Some(tokens.1);
        row.cache_read_tokens = Some(tokens.2);
        row.cost_usd = Some(cost);
        row
    }

    #[test]
    fn token_usage_sums_terminal_rows_and_skips_skill_snapshots() {
        let db_path = env::temp_dir().join(format!("lfd-test-{}.db", LfdId::new()));
        let store = SqliteStore::new(&db_path).expect("store should open");

        let rows = [
            event_row("a", 1, "run", "started"),
            // A skill boundary carries a *cumulative* snapshot of the run so
            // far, so summing every row would count its tokens twice.
            with_usage(
                event_row("a", 2, "skill", "completed"),
                Some("w1"),
                "claude",
                (60, 30, 900),
                0.10,
            ),
            with_usage(
                event_row("a", 3, "run", "completed"),
                Some("w1"),
                "claude",
                (100, 50, 1500),
                0.25,
            ),
            // Run b belongs to no wave — it still belongs in the rollup.
            with_usage(
                event_row("b", 1, "run", "completed"),
                None,
                "codex",
                (200, 80, 0),
                0.10,
            ),
        ];
        for row in &rows {
            store.insert_run_event(row).expect("insert run event");
        }

        let report = store.aggregate_token_usage().expect("aggregate");

        let claude = report
            .iter()
            .find(|row| row.provider.as_deref() == Some("claude"))
            .expect("claude row");
        assert_eq!(
            claude.input_tokens, 100,
            "skill snapshot must not double-count"
        );
        assert_eq!(claude.cache_read_tokens, 1500);
        assert_eq!(claude.cost_usd, 0.25);

        // A wave-less run still aggregates: the grain is (repo, provider).
        let codex = report
            .iter()
            .find(|row| row.provider.as_deref() == Some("codex"))
            .expect("codex belongs in the rollup despite having no wave");
        assert_eq!(codex.input_tokens, 200);
        assert_eq!(codex.cost_usd, 0.10);
    }

    #[test]
    fn token_usage_is_additive_across_processes_sharing_a_run_id() {
        let db_path = env::temp_dir().join(format!("lfd-test-{}.db", LfdId::new()));
        let store = SqliteStore::new(&db_path).expect("store should open");

        // A child `lf` inherits LF_RUN_ID, so one run_id can carry a terminal
        // row from each process. Their tokens add; they do not overwrite.
        let mut parent = with_usage(
            event_row("shared", 1, "run", "completed"),
            Some("w1"),
            "claude",
            (100, 10, 0),
            0.10,
        );
        parent.process_id = "parent".to_string();
        let mut child = with_usage(
            event_row("shared", 2, "run", "completed"),
            Some("w1"),
            "claude",
            (5, 1, 0),
            0.01,
        );
        child.process_id = "child".to_string();
        child.parent_process_id = Some("parent".to_string());
        let rows = [parent, child];
        for row in &rows {
            store.insert_run_event(row).expect("insert run event");
        }

        let report = store.aggregate_token_usage().expect("aggregate");
        assert_eq!(report.len(), 1);
        assert_eq!(report[0].input_tokens, 105);
        assert!((report[0].cost_usd - 0.11).abs() < f64::EPSILON);
    }

    #[test]
    fn a_closed_vocabulary_rejects_an_unknown_node() {
        let db_path = env::temp_dir().join(format!("lfd-test-{}.db", LfdId::new()));
        let store = SqliteStore::new(&db_path).expect("store should open");
        let row = event_row("bad-node", 0, "task", "started");
        let error = store
            .insert_run_event(&row)
            .expect_err("unknown node must violate the ledger contract");
        assert!(error.to_string().contains("CHECK constraint failed"));
    }

    #[test]
    fn store_open_rejects_a_recorded_migration_with_a_drifted_ledger() {
        let db_path = env::temp_dir().join(format!("lfd-test-{}.db", LfdId::new()));
        {
            let conn = rusqlite::Connection::open(&db_path).expect("open sqlite db");
            conn.execute_batch(
                "CREATE TABLE schema_migrations (
                     version TEXT PRIMARY KEY,
                     applied_at INTEGER NOT NULL
                 );
                 CREATE TABLE run_events (run_id TEXT NOT NULL);",
            )
            .unwrap();
            for migration in super::migrations::migrations() {
                conn.execute(
                    "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, 0)",
                    rusqlite::params![migration.version],
                )
                .unwrap();
            }
        }

        let error = SqliteStore::new(&db_path)
            .expect_err("a ledger that lies about migration 057 must fail open");
        assert!(error.to_string().contains("process_id"));
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
    // "running" — the Loopflow sidebar and buttons stay disabled forever even
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
                    flow: DEFAULT_WAVE_FLOW.to_string(),
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

        store.delete_wave(wave.id()).await.expect("delete wave");
    }
}
