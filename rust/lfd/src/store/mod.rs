use std::sync::Arc;

use crate::id::LfdId;
use crate::proto::control::Agent;
use crate::proto::control::PendingActivation;
use crate::proto::control::Stimulus;
use crate::proto::control::Wave;

pub mod postgres;
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
    pub wave_id: LfdId,
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
    fn schema_version(&self) -> StoreResult<u32>;

    // Wave management
    fn list_waves(&self, repo: Option<&str>) -> StoreResult<Vec<Wave>>;
    fn get_wave(&self, wave_id: &LfdId) -> StoreResult<Option<Wave>>;
    fn create_wave(&self, wave: &Wave) -> StoreResult<()>;
    fn update_wave(&self, wave: &Wave) -> StoreResult<()>;
    fn delete_wave(&self, wave_id: &LfdId) -> StoreResult<()>;

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
    fn list_fork_runs(&self, wave_id: &LfdId, step_index: u32) -> StoreResult<Vec<ForkRun>>;
    fn upsert_fork_run(&self, fork_run: &ForkRun) -> StoreResult<()>;
    fn delete_fork_runs(&self, wave_id: &LfdId, step_index: u32) -> StoreResult<u32>;

    // Step runs
    fn list_agents(&self) -> StoreResult<Vec<Agent>>;
    fn list_agent_history(
        &self,
        worktree: Option<&str>,
        repo: Option<&str>,
        limit: Option<u32>,
    ) -> StoreResult<Vec<Agent>>;
    fn get_agent(&self, agent_id: &LfdId) -> StoreResult<Option<Agent>>;
    fn get_waiting_agent(&self, wave_id: &LfdId) -> StoreResult<Option<Agent>>;
    fn start_agent(&self, agent: &Agent) -> StoreResult<()>;
    fn update_agent_status(
        &self,
        agent_id: &LfdId,
        status: i32,
        pid: Option<u32>,
    ) -> StoreResult<()>;
    fn end_agent(&self, agent_id: &LfdId, status: i32, ended_at: i64) -> StoreResult<()>;
    fn end_active_agent_for_wave(
        &self,
        wave_id: &LfdId,
        status: i32,
        ended_at: i64,
    ) -> StoreResult<()>;
    fn get_stuck_agents(&self, older_than_secs: u64) -> StoreResult<Vec<Agent>>;
}

pub type SharedStore = Arc<dyn RunStore>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::LfdId;
    use crate::proto::control::{
        Agent, AgentStatus, MergeMode, PendingActivation, Stimulus, StimulusKind, Wave, WaveStatus,
    };
    use prost_types::Timestamp;
    use std::env;
    use std::path::PathBuf;
    use testcontainers::{clients::Cli, core::WaitFor, GenericImage};
    use time::OffsetDateTime;
    use uuid::Uuid;

    fn now_timestamp() -> Timestamp {
        let now = OffsetDateTime::now_utc();
        Timestamp {
            seconds: now.unix_timestamp(),
            nanos: 0,
        }
    }

    fn make_wave(repo: &str) -> Wave {
        let id = LfdId::new().to_string();
        Wave {
            id: id.clone(),
            name: format!("wave-{id}"),
            repo: repo.to_string(),
            flow: "default".to_string(),
            direction: vec!["focus".to_string()],
            area: vec!["src".to_string()],
            paused: false,
            status: WaveStatus::WaveIdle as i32,
            iteration: 0,
            worktree: "/tmp".to_string(),
            branch: "main".to_string(),
            pr_limit: 5,
            merge_mode: MergeMode::MergePr as i32,
            pid: None,
            created_at: Some(now_timestamp()),
            consecutive_failures: 0,
            pending_activations: 0,
            step_index: 0,
        }
    }

    fn make_stimulus(wave_id: &str) -> Stimulus {
        Stimulus {
            id: LfdId::new().to_string(),
            wave_id: wave_id.to_string(),
            kind: StimulusKind::StimulusWatch as i32,
            cron: "".to_string(),
            last_main_sha: Some("abc123".to_string()),
            last_triggered_at: Some(100),
            enabled: true,
            created_at: Some(now_timestamp()),
        }
    }

    fn make_pending_activation(wave_id: &str, stimulus_id: &str) -> PendingActivation {
        PendingActivation {
            id: LfdId::new().to_string(),
            wave_id: wave_id.to_string(),
            stimulus_id: stimulus_id.to_string(),
            from_sha: "aaa".to_string(),
            to_sha: "bbb".to_string(),
            queued_at: OffsetDateTime::now_utc().unix_timestamp(),
        }
    }

    fn make_agent(wave_id: Option<&str>, status: AgentStatus, started_at: i64) -> Agent {
        Agent {
            id: LfdId::new().to_string(),
            step: "plan".to_string(),
            repo: "/repo".to_string(),
            worktree: "/repo".to_string(),
            flow_run_id: None,
            wave_id: wave_id.map(|value| value.to_string()),
            status: status as i32,
            started_at: Some(Timestamp {
                seconds: started_at,
                nanos: 0,
            }),
            ended_at: None,
            pid: None,
            model: "claude-code".to_string(),
            run_mode: "auto".to_string(),
        }
    }

    fn run_store_suite(store: &dyn RunStore) {
        let mut wave = make_wave("/repo");
        store.create_wave(&wave).unwrap();

        let loaded = store
            .get_wave(&LfdId::parse(&wave.id).unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(loaded.name, wave.name);

        wave.paused = true;
        wave.status = WaveStatus::WaveRunning as i32;
        store.update_wave(&wave).unwrap();
        let updated = store
            .get_wave(&LfdId::parse(&wave.id).unwrap())
            .unwrap()
            .unwrap();
        assert!(updated.paused);
        assert_eq!(updated.status, WaveStatus::WaveRunning as i32);

        let repo_waves = store.list_waves(Some(&wave.repo)).unwrap();
        assert!(!repo_waves.is_empty());

        let stimulus = make_stimulus(&wave.id);
        store.create_stimulus(&stimulus).unwrap();
        let listed = store
            .list_stimuli(Some(&LfdId::parse(&wave.id).unwrap()))
            .unwrap();
        assert_eq!(listed.len(), 1);
        let by_kind = store
            .list_stimuli_by_kind(StimulusKind::StimulusWatch as i32)
            .unwrap();
        assert_eq!(by_kind.len(), 1);

        let mut stimulus_updated = stimulus.clone();
        stimulus_updated.enabled = false;
        store.update_stimulus(&stimulus_updated).unwrap();
        let loaded_stimulus = store
            .get_stimulus(&LfdId::parse(&stimulus.id).unwrap())
            .unwrap()
            .unwrap();
        assert!(!loaded_stimulus.enabled);

        let activation = make_pending_activation(&wave.id, &stimulus.id);
        store.create_pending_activation(&activation).unwrap();
        let activations = store
            .list_pending_activations(&LfdId::parse(&wave.id).unwrap())
            .unwrap();
        assert_eq!(activations.len(), 1);
        let pending = store
            .get_pending_for_stimulus(
                &LfdId::parse(&wave.id).unwrap(),
                &LfdId::parse(&stimulus.id).unwrap(),
            )
            .unwrap()
            .unwrap();
        assert_eq!(pending.id, activation.id);

        let wave_after_pending = store
            .get_wave(&LfdId::parse(&wave.id).unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(wave_after_pending.pending_activations, 1);

        let mut activation_updated = activation.clone();
        activation_updated.to_sha = "ccc".to_string();
        store
            .update_pending_activation(&activation_updated)
            .unwrap();

        let deleted = store
            .delete_pending_activations(&LfdId::parse(&wave.id).unwrap())
            .unwrap();
        assert_eq!(deleted, 1);
        let wave_after_delete = store
            .get_wave(&LfdId::parse(&wave.id).unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(wave_after_delete.pending_activations, 0);

        let fork_run = ForkRun {
            id: LfdId::new(),
            wave_id: LfdId::parse(&wave.id).unwrap(),
            step_index: 0,
            branch_index: 0,
            status: ForkRunStatus::Pending,
            worktree: "/tmp/branch".to_string(),
        };
        store.upsert_fork_run(&fork_run).unwrap();
        let forks = store
            .list_fork_runs(&LfdId::parse(&wave.id).unwrap(), 0)
            .unwrap();
        assert_eq!(forks.len(), 1);
        let deleted_forks = store
            .delete_fork_runs(&LfdId::parse(&wave.id).unwrap(), 0)
            .unwrap();
        assert_eq!(deleted_forks, 1);

        let now = OffsetDateTime::now_utc().unix_timestamp();
        let agent = make_agent(Some(&wave.id), AgentStatus::AgentWaiting, now);
        store.start_agent(&agent).unwrap();
        let waiting = store
            .get_waiting_agent(&LfdId::parse(&wave.id).unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(waiting.id, agent.id);
        store
            .update_agent_status(
                &LfdId::parse(&agent.id).unwrap(),
                AgentStatus::AgentRunning as i32,
                Some(123),
            )
            .unwrap();
        store
            .end_agent(
                &LfdId::parse(&agent.id).unwrap(),
                AgentStatus::AgentCompleted as i32,
                now + 10,
            )
            .unwrap();

        let old_run = make_agent(Some(&wave.id), AgentStatus::AgentRunning, now - 3600);
        store.start_agent(&old_run).unwrap();
        let stuck = store.get_stuck_agents(60).unwrap();
        assert!(stuck.iter().any(|run| run.id == old_run.id));

        store.delete_wave(&LfdId::parse(&wave.id).unwrap()).unwrap();
        assert!(store
            .get_wave(&LfdId::parse(&wave.id).unwrap())
            .unwrap()
            .is_none());
    }

    fn reset_postgres(url: &str) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let (client, connection) = tokio_postgres::connect(url, tokio_postgres::NoTls)
                .await
                .unwrap();
            let connection_task = tokio::spawn(async move {
                let _ = connection.await;
            });
            client
                .batch_execute(
                    "TRUNCATE pending_activations, stimuli, fork_runs, agents, waves CASCADE",
                )
                .await
                .unwrap();
            connection_task.abort();
        });
    }

    #[test]
    fn sqlite_store_suite() {
        let path = env::temp_dir()
            .join(format!("lfd-test-{}.db", Uuid::new_v4()))
            .to_string_lossy()
            .to_string();
        let store = super::sqlite::SqliteStore::new(&PathBuf::from(path)).unwrap();
        run_store_suite(&store);
    }

    #[test]
    fn postgres_store_suite() {
        if env::var("LFD_SKIP_POSTGRES_TESTS").is_ok() {
            eprintln!("LFD_SKIP_POSTGRES_TESTS set; skipping postgres suite");
            return;
        }

        if let Ok(url) = env::var("LFD_DATABASE_URL") {
            super::postgres::PostgresStore::migrate(&url).unwrap();
            reset_postgres(&url);
            let store = super::postgres::PostgresStore::connect(&url).unwrap();
            run_store_suite(&store);
            return;
        }

        let docker = Cli::default();
        let image = GenericImage::new("postgres", "16")
            .with_env_var("POSTGRES_PASSWORD", "test")
            .with_env_var("POSTGRES_USER", "postgres")
            .with_env_var("POSTGRES_DB", "postgres")
            .with_exposed_port(5432)
            .with_wait_for(WaitFor::message_on_stdout(
                "database system is ready to accept connections",
            ));
        let node = docker.run(image);
        let port = node.get_host_port_ipv4(5432);
        let url = format!("postgres://postgres:test@127.0.0.1:{port}/postgres");

        super::postgres::PostgresStore::migrate(&url).unwrap();
        reset_postgres(&url);
        let store = super::postgres::PostgresStore::connect(&url).unwrap();
        run_store_suite(&store);
    }
}
