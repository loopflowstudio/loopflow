# Stage 1 Design: Store trait scope reset (foundation)

## User intent (verbatim)
- "for the local / self-deployed scenario, my assumption was that sqlite would be better"
- "for the prod use case, with potentially one giant postgres for like a whole company, it seems like postgres is required"
- "and trying to make the trait only the stuff that varies"
- "is this an explosion of store triats though"

## What to build
Refactor `lfd` store interfaces from one monolithic trait into grouped capability traits plus a concrete store wrapper, while preserving SQLite+Postgres behavior and existing runtime semantics.

## Data structures
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageBackend {
    Sqlite,
    Postgres,
}

#[derive(Debug)]
pub struct Store {
    backend: StoreBackend,
}

#[derive(Debug)]
enum StoreBackend {
    Sqlite(SqliteStore),
    Postgres(PostgresStore),
}
```

```rust
#[async_trait::async_trait]
pub trait WaveStateStore {
    // Waves
    async fn list_waves(&self, repo: Option<&str>) -> StoreResult<Vec<Wave>>;
    async fn get_wave(&self, wave_id: &LfdId) -> StoreResult<Option<Wave>>;
    async fn get_wave_by_name(&self, name: &str) -> StoreResult<Option<Wave>>;
    async fn create_wave(&self, wave: &Wave) -> StoreResult<()>;
    async fn update_wave(&self, wave: &Wave) -> StoreResult<()>;
    async fn delete_wave(&self, wave_id: &LfdId) -> StoreResult<()>;

    // Wave runs
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

    // Stimuli + pending activations + summaries
    async fn list_stimuli(&self, wave_id: Option<&LfdId>) -> StoreResult<Vec<Stimulus>>;
    async fn list_stimuli_by_kind(&self, kind: i32) -> StoreResult<Vec<Stimulus>>;
    async fn get_stimulus(&self, stimulus_id: &LfdId) -> StoreResult<Option<Stimulus>>;
    async fn create_stimulus(&self, stimulus: &Stimulus) -> StoreResult<()>;
    async fn update_stimulus(&self, stimulus: &Stimulus) -> StoreResult<()>;
    async fn delete_stimulus(&self, stimulus_id: &LfdId) -> StoreResult<()>;
    async fn delete_stimuli_for_wave(&self, wave_id: &LfdId) -> StoreResult<u32>;

    async fn list_pending_activations(&self, wave_id: &LfdId) -> StoreResult<Vec<PendingActivation>>;
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
pub trait ExecutionStore {
    async fn list_fork_runs(&self, wave_run_id: &LfdId, step_index: u32) -> StoreResult<Vec<ForkRun>>;
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
pub trait StoreAdmin {
    async fn health_check(&self) -> StoreResult<()>;
    async fn schema_version(&self) -> StoreResult<String>;
}
```

## Key functions
```rust
pub async fn open_store(cfg: &StorageConfig) -> StoreResult<Store>;
pub async fn migrate_store(cfg: &StorageConfig, status_only: bool) -> StoreResult<String>;
```

```rust
impl WaveStateStore for Store { /* delegate to backend */ }
impl ExecutionStore for Store { /* delegate to backend */ }
impl StoreAdmin for Store { /* delegate to backend */ }
```

## Constraints
1. Keep both backends working; no regression to Postgres support.
2. No behavior change in existing store tests.
3. Do not add trait-per-table sprawl beyond the 3 grouped traits above.
4. Backend-specific setup (`connect`, migrations, query rendering) stays out of capability traits.
5. Keep route/executor call-site churn minimal in Stage 1 (focus on API shape and delegation only).

## Done when
```bash
cargo test -p loopflow lfd::store::tests::sqlite_store_suite
cargo test -p loopflow lfd::store::tests::run_active_excludes_failed_latest_includes
cargo test -p loopflow
```

Expected result: tests pass with SQLite and Postgres codepaths still compiling, and the codebase no longer depends on one monolithic store trait for new work.
