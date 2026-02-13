use std::sync::Arc;

use crate::lfd::id::LfdId;
use crate::lfd::types::{Agent, PendingActivation, Stimulus, Summary, Wave, WaveRun};

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
        Agent, AgentStatus, PendingActivation, Stimulus, StimulusKind, Summary, Wave, WaveRun,
        WaveRunSnapshot, WaveRunStatus, WaveStatus,
    };
    use std::env;
    use std::path::PathBuf;
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
        };
        store.create_wave_run(&run).unwrap();
        let loaded_run = store.get_wave_run(&run.id).unwrap().unwrap();
        assert_eq!(loaded_run.wave_id, wave.id);

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
    fn run_active_excludes_failed_latest_includes(store: &dyn RunStore) {
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

        store.delete_wave(&wave.id).unwrap();
    }

    #[test]
    fn sqlite_store_suite() {
        let path = env::temp_dir()
            .join(format!("lfd-test-{}.db", LfdId::new()))
            .to_string_lossy()
            .to_string();
        let store = super::sqlite::SqliteStore::new(&PathBuf::from(path)).unwrap();
        run_store_suite(&store);
        run_active_excludes_failed_latest_includes(&store);
    }
}
