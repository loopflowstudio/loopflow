use std::future::Future;
use std::time::Duration;

use deadpool_postgres::{Manager, Pool};
use tokio_postgres::types::ToSql;
use tokio_postgres::NoTls;

use crate::lfd::id::LfdId;
use crate::lfd::store::rows::{
    map_agent_row, map_chat_memory_block_row, map_fork_run_row, map_live_pr_state_row,
    map_pending_activation_row, map_stimulus_row, map_summary_row, map_wave_row, map_wave_run_row,
    now_unix, serialize_pr,
};
use crate::lfd::store::{ForkRun, RunStore, StoreError, StoreResult};
use crate::lfd::types::{
    Agent, AgentStatus, ChatMemoryBlock, LivePullRequestState, PendingActivation, Stimulus,
    Summary, Wave, WaveRun, WaveRunStatus, WaveStatus,
};

const RETRY_DELAYS: [Duration; 3] = [
    Duration::from_millis(100),
    Duration::from_millis(500),
    Duration::from_secs(2),
];

// NOTE: Sync trait with block_on bridging
//
// This design prioritizes trait compatibility with SqliteStore over async efficiency.
// Fine for: tens of waves, low-latency Postgres, moderate traffic.
// Revisit when: 100s+ concurrent waves, high-latency Postgres, or pool exhaustion.

#[derive(Debug)]
pub struct PostgresStore {
    pool: Pool,
    runtime: tokio::runtime::Runtime,
}

impl PostgresStore {
    #[allow(dead_code)]
    pub fn connect(database_url: &str) -> StoreResult<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to build postgres runtime");
        let pool = build_pool(database_url)?;
        let store = Self { pool, runtime };
        let version = store.schema_version()?;
        if version.is_empty() {
            return Err(StoreError::InvalidData(
                "postgres schema missing; run `lfd migrate`".to_string(),
            ));
        }
        Ok(store)
    }

    pub async fn connect_async(database_url: &str) -> StoreResult<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to build postgres runtime");
        let pool = build_pool(database_url)?;
        let version = super::migrations::latest_version_postgres_pool(&pool).await?;
        if version.is_empty() {
            return Err(StoreError::InvalidData(
                "postgres schema missing; run `lfd migrate`".to_string(),
            ));
        }
        Ok(Self { pool, runtime })
    }

    #[allow(dead_code)]
    pub fn migrate(database_url: &str) -> StoreResult<String> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to build postgres runtime");
        runtime.block_on(Self::migrate_async(database_url))
    }

    pub async fn migrate_async(database_url: &str) -> StoreResult<String> {
        let (mut client, connection) = tokio_postgres::connect(database_url, NoTls).await?;
        let connection_task = tokio::spawn(async move {
            let _ = connection.await;
        });
        let transaction = client.transaction().await?;
        super::migrations::apply_postgres(&transaction).await?;
        transaction.commit().await?;
        let version = super::migrations::latest_version_postgres_client(&client).await?;
        connection_task.abort();
        Ok(version)
    }

    pub async fn migrate_status_async(database_url: &str) -> StoreResult<String> {
        let (client, connection) = tokio_postgres::connect(database_url, NoTls).await?;
        let connection_task = tokio::spawn(async move {
            let _ = connection.await;
        });
        let result = super::migrations::latest_version_postgres_client(&client).await;
        connection_task.abort();
        result
    }

    fn block_on<T, Fut>(&self, future: Fut) -> StoreResult<T>
    where
        Fut: Future<Output = StoreResult<T>>,
    {
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            tokio::task::block_in_place(|| handle.block_on(future))
        } else {
            self.runtime.block_on(future)
        }
    }

    fn with_client<T, F, Fut>(&self, func: F) -> StoreResult<T>
    where
        F: FnOnce(deadpool_postgres::Client) -> Fut,
        Fut: Future<Output = StoreResult<T>>,
    {
        self.block_on(async {
            let client = get_client_with_retry(&self.pool).await?;
            func(client).await
        })
    }

    fn read_waves(&self, repo: Option<&str>) -> StoreResult<Vec<Wave>> {
        self.with_client(|client| async move {
            let rows = if let Some(repo) = repo {
                client
                    .query(
                        "SELECT id, name, repo, flow, direction, area, paused, status, iteration,
                                created_at, schema_ref, schema_name
                         FROM waves WHERE repo = $1 ORDER BY created_at DESC",
                        &[&repo],
                    )
                    .await?
            } else {
                client
                    .query(
                        "SELECT id, name, repo, flow, direction, area, paused, status, iteration,
                                created_at, schema_ref, schema_name
                         FROM waves ORDER BY created_at DESC",
                        &[],
                    )
                    .await?
            };
            rows.iter().map(map_wave_row).collect()
        })
    }

    fn upsert_wave(&self, wave: &Wave) -> StoreResult<()> {
        self.with_client(|client| async move {
            let direction_json = serde_json::to_string(&wave.direction)?;
            let area_json = serde_json::to_string(&wave.area)?;
            let created_at = wave
                .created_at
                .map(|dt| dt.unix_timestamp())
                .unwrap_or_else(now_unix);
            let paused: i32 = if wave.status == WaveStatus::Paused {
                1
            } else {
                0
            };

            client
                .execute(
                    "INSERT INTO waves (
                        id, name, repo, flow, direction, area, paused, status, iteration, created_at,
                        schema_ref, schema_name
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                    ON CONFLICT(id) DO UPDATE SET
                        name = excluded.name,
                        repo = excluded.repo,
                        flow = excluded.flow,
                        direction = excluded.direction,
                        area = excluded.area,
                        paused = excluded.paused,
                        status = excluded.status,
                        iteration = excluded.iteration,
                        created_at = excluded.created_at,
                        schema_ref = excluded.schema_ref,
                        schema_name = excluded.schema_name",
                    &[
                        &wave.id,
                        &wave.name,
                        &wave.repo,
                        &wave.flow,
                        &direction_json,
                        &area_json,
                        &paused,
                        &wave.status.as_i32(),
                        &(wave.iteration as i32),
                        &created_at,
                        &wave.schema_ref,
                        &wave.schema_name,
                    ],
                )
                .await?;
            Ok(())
        })
    }
}

impl RunStore for PostgresStore {
    fn health_check(&self) -> StoreResult<()> {
        self.with_client(|client| async move {
            client.execute("SELECT 1", &[]).await?;
            Ok(())
        })
    }

    fn schema_version(&self) -> StoreResult<String> {
        self.block_on(super::migrations::latest_version_postgres_pool(&self.pool))
    }

    fn list_waves(&self, repo: Option<&str>) -> StoreResult<Vec<Wave>> {
        self.read_waves(repo)
    }

    fn get_wave(&self, wave_id: &LfdId) -> StoreResult<Option<Wave>> {
        self.with_client(|client| async move {
            let row = client
                .query_opt(
                    "SELECT id, name, repo, flow, direction, area, paused, status, iteration,
                            created_at, schema_ref, schema_name
                     FROM waves WHERE id = $1",
                    &[&wave_id],
                )
                .await?;
            row.as_ref().map(map_wave_row).transpose()
        })
    }

    fn get_wave_by_name(&self, name: &str) -> StoreResult<Option<Wave>> {
        let name = name.to_string();
        self.with_client(|client| async move {
            let row = client
                .query_opt(
                    "SELECT id, name, repo, flow, direction, area, paused, status, iteration,
                            created_at, schema_ref, schema_name
                     FROM waves WHERE name = $1",
                    &[&name],
                )
                .await?;
            row.as_ref().map(map_wave_row).transpose()
        })
    }

    fn create_wave(&self, wave: &Wave) -> StoreResult<()> {
        self.upsert_wave(wave)
    }

    fn update_wave(&self, wave: &Wave) -> StoreResult<()> {
        self.upsert_wave(wave)
    }

    fn delete_wave(&self, wave_id: &LfdId) -> StoreResult<()> {
        self.with_client(|client| async move {
            client
                .execute("DELETE FROM waves WHERE id = $1", &[&wave_id])
                .await?;
            Ok(())
        })
    }

    fn list_wave_runs(
        &self,
        wave_id: Option<&LfdId>,
        limit: Option<u32>,
    ) -> StoreResult<Vec<WaveRun>> {
        self.with_client(|client| async move {
            let mut query = String::from(
                "SELECT id, wave_id, iteration, step_index, status, worktree, branch,
                        started_at, ended_at, error, snapshot_repo, snapshot_flow, snapshot_direction,
                        snapshot_area, snapshot_pr, flow_parents, run_kind, sidecar_kind,
                        parent_run_id, parent_pr_number, stack_position, stack_group_id,
                        stack_status, lineage_inferred
                 FROM wave_runs",
            );
            let mut params: Vec<Box<dyn ToSql + Sync>> = Vec::new();
            if let Some(wave_id) = wave_id {
                query.push_str(" WHERE wave_id = $1");
                params.push(Box::new(wave_id.clone()));
            }
            query.push_str(" ORDER BY started_at DESC");
            if let Some(limit) = limit {
                query.push_str(&format!(" LIMIT ${}", params.len() + 1));
                params.push(Box::new(limit as i64));
            }

            let params_ref: Vec<&(dyn ToSql + Sync)> =
                params.iter().map(|v| v.as_ref()).collect();
            let rows = client.query(&query, &params_ref).await?;
            rows.iter().map(map_wave_run_row).collect()
        })
    }

    fn get_wave_run(&self, wave_run_id: &LfdId) -> StoreResult<Option<WaveRun>> {
        self.with_client(|client| async move {
            let row = client
                .query_opt(
                    "SELECT id, wave_id, iteration, step_index, status, worktree, branch,
                            started_at, ended_at, error, snapshot_repo, snapshot_flow, snapshot_direction,
                            snapshot_area, snapshot_pr, flow_parents, run_kind, sidecar_kind,
                            parent_run_id, parent_pr_number, stack_position, stack_group_id,
                            stack_status, lineage_inferred
                     FROM wave_runs WHERE id = $1",
                    &[&wave_run_id],
                )
                .await?;
            row.as_ref().map(map_wave_run_row).transpose()
        })
    }

    fn get_active_wave_run(&self, wave_id: &LfdId) -> StoreResult<Option<WaveRun>> {
        self.with_client(|client| async move {
            let statuses = [
                WaveRunStatus::Pending.as_i32(),
                WaveRunStatus::Running.as_i32(),
                WaveRunStatus::Waiting.as_i32(),
            ];
            let row = client
                .query_opt(
                    "SELECT id, wave_id, iteration, step_index, status, worktree, branch,
                            started_at, ended_at, error, snapshot_repo, snapshot_flow, snapshot_direction,
                            snapshot_area, snapshot_pr, flow_parents, run_kind, sidecar_kind,
                            parent_run_id, parent_pr_number, stack_position, stack_group_id,
                            stack_status, lineage_inferred
                     FROM wave_runs
                     WHERE wave_id = $1 AND status = ANY($2) AND run_kind = $3
                     ORDER BY started_at DESC LIMIT 1",
                    &[&wave_id, &&statuses[..], &crate::lfd::types::WaveRunKind::Main.as_i32()],
                )
                .await?;
            row.as_ref().map(map_wave_run_row).transpose()
        })
    }

    fn get_latest_wave_run(&self, wave_id: &LfdId) -> StoreResult<Option<WaveRun>> {
        self.with_client(|client| async move {
            let row = client
                .query_opt(
                    "SELECT id, wave_id, iteration, step_index, status, worktree, branch,
                            started_at, ended_at, error, snapshot_repo, snapshot_flow, snapshot_direction,
                            snapshot_area, snapshot_pr, flow_parents, run_kind, sidecar_kind,
                            parent_run_id, parent_pr_number, stack_position, stack_group_id,
                            stack_status, lineage_inferred
                     FROM wave_runs WHERE wave_id = $1 AND run_kind = $2
                     ORDER BY started_at DESC LIMIT 1",
                    &[&wave_id, &crate::lfd::types::WaveRunKind::Main.as_i32()],
                )
                .await?;
            row.as_ref().map(map_wave_run_row).transpose()
        })
    }

    fn create_wave_run(&self, run: &WaveRun) -> StoreResult<()> {
        self.with_client(|client| async move {
            let started_at = run
                .started_at
                .map(|dt| dt.unix_timestamp())
                .unwrap_or_else(now_unix);
            let ended_at = run.ended_at.map(|dt| dt.unix_timestamp());
            let flow_parents_json = serde_json::to_string(&run.flow_parents)?;
            client
                .execute(
                    "INSERT INTO wave_runs (
                        id, wave_id, iteration, step_index, status, worktree, branch,
                        started_at, ended_at, error, snapshot_repo, snapshot_flow, snapshot_direction,
                        snapshot_area, snapshot_pr, flow_parents, run_kind, sidecar_kind,
                        parent_run_id, parent_pr_number, stack_position, stack_group_id,
                        stack_status, lineage_inferred
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24)",
                    &[
                        &run.id,
                        &run.wave_id,
                        &(run.iteration as i32),
                        &(run.step_index as i32),
                        &run.status.as_i32(),
                        &run.worktree,
                        &run.branch,
                        &started_at,
                        &ended_at,
                        &run.error,
                        &run.snapshot.repo,
                        &run.snapshot.flow,
                        &serde_json::to_string(&run.snapshot.direction)?,
                        &serde_json::to_string(&run.snapshot.area)?,
                        &serialize_pr(&run.snapshot.pr)?,
                        &flow_parents_json,
                        &run.run_kind.as_i32(),
                        &run.sidecar_kind.map(|kind| kind.as_i32()),
                        &run.parent_run_id,
                        &run.parent_pr_number.map(|value| value as i64),
                        &(run.stack_position as i32),
                        &run.stack_group_id,
                        &run.stack_status.as_i32(),
                        &(if run.lineage_inferred { 1i32 } else { 0i32 }),
                    ],
                )
                .await?;
            Ok(())
        })
    }

    fn update_wave_run(&self, run: &WaveRun) -> StoreResult<()> {
        self.with_client(|client| async move {
            let flow_parents_json = serde_json::to_string(&run.flow_parents)?;
            let updated = client
                .execute(
                    "UPDATE wave_runs
                     SET iteration = $1, step_index = $2, status = $3, worktree = $4,
                         branch = $5, started_at = $6, ended_at = $7, error = $8,
                         snapshot_repo = $9, snapshot_flow = $10, snapshot_direction = $11,
                         snapshot_area = $12, snapshot_pr = $13, flow_parents = $14,
                         run_kind = $15, sidecar_kind = $16, parent_run_id = $17,
                         parent_pr_number = $18, stack_position = $19, stack_group_id = $20,
                         stack_status = $21, lineage_inferred = $22
                     WHERE id = $23",
                    &[
                        &(run.iteration as i32),
                        &(run.step_index as i32),
                        &run.status.as_i32(),
                        &run.worktree,
                        &run.branch,
                        &run.started_at.map(|dt| dt.unix_timestamp()),
                        &run.ended_at.map(|dt| dt.unix_timestamp()),
                        &run.error,
                        &run.snapshot.repo,
                        &run.snapshot.flow,
                        &serde_json::to_string(&run.snapshot.direction)?,
                        &serde_json::to_string(&run.snapshot.area)?,
                        &serialize_pr(&run.snapshot.pr)?,
                        &flow_parents_json,
                        &run.run_kind.as_i32(),
                        &run.sidecar_kind.map(|kind| kind.as_i32()),
                        &run.parent_run_id,
                        &run.parent_pr_number.map(|value| value as i64),
                        &(run.stack_position as i32),
                        &run.stack_group_id,
                        &run.stack_status.as_i32(),
                        &(if run.lineage_inferred { 1i32 } else { 0i32 }),
                        &run.id,
                    ],
                )
                .await?;
            if updated == 0 {
                return Err(StoreError::NotFound);
            }
            Ok(())
        })
    }

    fn list_stack_runs(&self, wave_id: &LfdId) -> StoreResult<Vec<WaveRun>> {
        self.with_client(|client| async move {
            let rows = client
                .query(
                    "SELECT id, wave_id, iteration, step_index, status, worktree, branch,
                            started_at, ended_at, error, snapshot_repo, snapshot_flow, snapshot_direction,
                            snapshot_area, snapshot_pr, flow_parents, run_kind, sidecar_kind,
                            parent_run_id, parent_pr_number, stack_position, stack_group_id,
                            stack_status, lineage_inferred
                     FROM wave_runs
                     WHERE wave_id = $1 AND run_kind = $2
                     ORDER BY stack_position ASC, started_at ASC, id ASC",
                    &[&wave_id, &crate::lfd::types::WaveRunKind::Main.as_i32()],
                )
                .await?;
            rows.iter().map(map_wave_run_row).collect()
        })
    }

    fn get_live_pr_state(
        &self,
        repo_id: &str,
        pr_number: u32,
    ) -> StoreResult<Option<LivePullRequestState>> {
        let repo_id = repo_id.to_string();
        self.with_client(|client| async move {
            let row = client
                .query_opt(
                    "SELECT repo_id, pr_number, state, is_draft, head_ref, head_sha, base_ref,
                            updated_at, merged_at, synced_at
                     FROM live_pr_states
                     WHERE repo_id = $1 AND pr_number = $2",
                    &[&repo_id, &(pr_number as i64)],
                )
                .await?;
            row.as_ref().map(map_live_pr_state_row).transpose()
        })
    }

    fn upsert_live_pr_state(&self, state: &LivePullRequestState) -> StoreResult<()> {
        let state = state.clone();
        self.with_client(|client| async move {
            client
                .execute(
                    "INSERT INTO live_pr_states (
                        repo_id, pr_number, state, is_draft, head_ref, head_sha, base_ref,
                        updated_at, merged_at, synced_at
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                    ON CONFLICT(repo_id, pr_number) DO UPDATE SET
                        state = excluded.state,
                        is_draft = excluded.is_draft,
                        head_ref = excluded.head_ref,
                        head_sha = excluded.head_sha,
                        base_ref = excluded.base_ref,
                        updated_at = excluded.updated_at,
                        merged_at = excluded.merged_at,
                        synced_at = excluded.synced_at",
                    &[
                        &state.repo_id,
                        &(state.pr_number as i64),
                        &state.state.as_i32(),
                        &(if state.is_draft { 1i32 } else { 0i32 }),
                        &state.head_ref,
                        &state.head_sha,
                        &state.base_ref,
                        &state.updated_at.unix_timestamp(),
                        &state.merged_at.map(|value| value.unix_timestamp()),
                        &state.synced_at.unix_timestamp(),
                    ],
                )
                .await?;
            Ok(())
        })
    }

    fn fail_orphaned_runs(&self) -> StoreResult<u32> {
        self.with_client(|client| async move {
            let statuses = [
                WaveRunStatus::Pending.as_i32(),
                WaveRunStatus::Running.as_i32(),
                WaveRunStatus::Waiting.as_i32(),
            ];
            let updated = client
                .execute(
                    "UPDATE wave_runs SET status = $1, error = $2, ended_at = $3
                     WHERE status = ANY($4)",
                    &[
                        &WaveRunStatus::Failed.as_i32(),
                        &"orphaned: lfd restarted".to_string(),
                        &now_unix(),
                        &&statuses[..],
                    ],
                )
                .await?;
            Ok(updated as u32)
        })
    }

    fn list_stimuli(&self, wave_id: Option<&LfdId>) -> StoreResult<Vec<Stimulus>> {
        self.with_client(|client| async move {
            let rows = if let Some(wave_id) = wave_id {
                client
                    .query(
                        "SELECT id, wave_id, kind, cron, last_main_sha, last_triggered_at, created_at, enabled
                         FROM stimuli WHERE wave_id = $1 ORDER BY created_at",
                        &[&wave_id],
                    )
                    .await?
            } else {
                client
                    .query(
                        "SELECT id, wave_id, kind, cron, last_main_sha, last_triggered_at, created_at, enabled
                         FROM stimuli ORDER BY created_at",
                        &[],
                    )
                    .await?
            };
            rows.iter().map(map_stimulus_row).collect()
        })
    }

    fn list_stimuli_by_kind(&self, kind: i32) -> StoreResult<Vec<Stimulus>> {
        self.with_client(|client| async move {
            let rows = client
                .query(
                    "SELECT id, wave_id, kind, cron, last_main_sha, last_triggered_at, created_at, enabled
                     FROM stimuli WHERE kind = $1 ORDER BY created_at",
                    &[&kind],
                )
                .await?;
            rows.iter().map(map_stimulus_row).collect()
        })
    }

    fn get_stimulus(&self, stimulus_id: &LfdId) -> StoreResult<Option<Stimulus>> {
        self.with_client(|client| async move {
            let row = client
                .query_opt(
                    "SELECT id, wave_id, kind, cron, last_main_sha, last_triggered_at, created_at, enabled
                     FROM stimuli WHERE id = $1",
                    &[&stimulus_id],
                )
                .await?;
            row.as_ref().map(map_stimulus_row).transpose()
        })
    }

    fn create_stimulus(&self, stimulus: &Stimulus) -> StoreResult<()> {
        self.with_client(|client| async move {
            let created_at = stimulus
                .created_at
                .map(|dt| dt.unix_timestamp())
                .unwrap_or_else(now_unix);
            let enabled: i32 = if stimulus.enabled { 1 } else { 0 };

            client
                .execute(
                    "INSERT INTO stimuli (id, wave_id, kind, cron, last_main_sha, last_triggered_at, created_at, enabled)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                    &[
                        &stimulus.id,
                        &stimulus.wave_id,
                        &stimulus.kind.as_i32(),
                        &stimulus.cron,
                        &stimulus.last_main_sha,
                        &stimulus.last_triggered_at,
                        &created_at,
                        &enabled,
                    ],
                )
                .await?;
            Ok(())
        })
    }

    fn update_stimulus(&self, stimulus: &Stimulus) -> StoreResult<()> {
        self.with_client(|client| async move {
            let enabled: i32 = if stimulus.enabled { 1 } else { 0 };
            let updated = client
                .execute(
                    "UPDATE stimuli SET
                        kind = $1, cron = $2, last_main_sha = $3,
                        last_triggered_at = $4, enabled = $5
                     WHERE id = $6",
                    &[
                        &stimulus.kind.as_i32(),
                        &stimulus.cron,
                        &stimulus.last_main_sha,
                        &stimulus.last_triggered_at,
                        &enabled,
                        &stimulus.id,
                    ],
                )
                .await?;
            if updated == 0 {
                return Err(StoreError::NotFound);
            }
            Ok(())
        })
    }

    fn delete_stimulus(&self, stimulus_id: &LfdId) -> StoreResult<()> {
        self.with_client(|client| async move {
            client
                .execute("DELETE FROM stimuli WHERE id = $1", &[&stimulus_id])
                .await?;
            Ok(())
        })
    }

    fn delete_stimuli_for_wave(&self, wave_id: &LfdId) -> StoreResult<u32> {
        self.with_client(|client| async move {
            let deleted = client
                .execute("DELETE FROM stimuli WHERE wave_id = $1", &[&wave_id])
                .await?;
            Ok(deleted as u32)
        })
    }

    fn list_pending_activations(&self, wave_id: &LfdId) -> StoreResult<Vec<PendingActivation>> {
        self.with_client(|client| async move {
            let rows = client
                .query(
                    "SELECT id, wave_id, stimulus_id, from_sha, to_sha, queued_at
                     FROM pending_activations WHERE wave_id = $1 ORDER BY queued_at",
                    &[&wave_id],
                )
                .await?;
            rows.iter().map(map_pending_activation_row).collect()
        })
    }

    fn create_pending_activation(&self, activation: &PendingActivation) -> StoreResult<()> {
        self.with_client(|client| async move {
            client
                .execute(
                    "INSERT INTO pending_activations (id, wave_id, stimulus_id, from_sha, to_sha, queued_at)
                     VALUES ($1, $2, $3, $4, $5, $6)",
                    &[
                        &activation.id,
                        &activation.wave_id,
                        &activation.stimulus_id,
                        &activation.from_sha,
                        &activation.to_sha,
                        &activation.queued_at,
                    ],
                )
                .await?;
            Ok(())
        })
    }

    fn update_pending_activation(&self, activation: &PendingActivation) -> StoreResult<()> {
        self.with_client(|client| async move {
            let updated = client
                .execute(
                    "UPDATE pending_activations SET from_sha = $1, to_sha = $2 WHERE id = $3",
                    &[&activation.from_sha, &activation.to_sha, &activation.id],
                )
                .await?;
            if updated == 0 {
                return Err(StoreError::NotFound);
            }
            Ok(())
        })
    }

    fn delete_pending_activations(&self, wave_id: &LfdId) -> StoreResult<u32> {
        self.with_client(|client| async move {
            let deleted = client
                .execute(
                    "DELETE FROM pending_activations WHERE wave_id = $1",
                    &[&wave_id],
                )
                .await?;
            Ok(deleted as u32)
        })
    }

    fn get_pending_for_stimulus(
        &self,
        wave_id: &LfdId,
        stimulus_id: &LfdId,
    ) -> StoreResult<Option<PendingActivation>> {
        self.with_client(|client| async move {
            let row = client
                .query_opt(
                    "SELECT id, wave_id, stimulus_id, from_sha, to_sha, queued_at
                     FROM pending_activations WHERE wave_id = $1 AND stimulus_id = $2",
                    &[&wave_id, &stimulus_id],
                )
                .await?;
            row.as_ref().map(map_pending_activation_row).transpose()
        })
    }

    fn list_fork_runs(&self, wave_run_id: &LfdId, step_index: u32) -> StoreResult<Vec<ForkRun>> {
        self.with_client(|client| async move {
            let rows = client
                .query(
                    "SELECT id, wave_run_id, step_index, branch_index, status, worktree
                     FROM fork_runs WHERE wave_run_id = $1 AND step_index = $2
                     ORDER BY branch_index ASC",
                    &[&wave_run_id, &(step_index as i32)],
                )
                .await?;
            rows.iter().map(map_fork_run_row).collect()
        })
    }

    fn upsert_fork_run(&self, fork_run: &ForkRun) -> StoreResult<()> {
        self.with_client(|client| async move {
            client
                .execute(
                    "INSERT INTO fork_runs (id, wave_run_id, step_index, branch_index, status, worktree)
                     VALUES ($1, $2, $3, $4, $5, $6)
                     ON CONFLICT(id) DO UPDATE SET
                         wave_run_id = excluded.wave_run_id,
                         step_index = excluded.step_index,
                         branch_index = excluded.branch_index,
                         status = excluded.status,
                         worktree = excluded.worktree",
                    &[
                        &fork_run.id,
                        &fork_run.wave_run_id,
                        &(fork_run.step_index as i32),
                        &(fork_run.branch_index as i32),
                        &(fork_run.status as i32),
                        &fork_run.worktree,
                    ],
                )
                .await?;
            Ok(())
        })
    }

    fn delete_fork_runs(&self, wave_run_id: &LfdId, step_index: u32) -> StoreResult<u32> {
        self.with_client(|client| async move {
            let deleted = client
                .execute(
                    "DELETE FROM fork_runs WHERE wave_run_id = $1 AND step_index = $2",
                    &[&wave_run_id, &(step_index as i32)],
                )
                .await?;
            Ok(deleted as u32)
        })
    }

    fn list_agents(&self) -> StoreResult<Vec<Agent>> {
        self.list_agent_history(None, None, None)
    }

    fn list_agent_history(
        &self,
        worktree: Option<&str>,
        repo: Option<&str>,
        limit: Option<u32>,
    ) -> StoreResult<Vec<Agent>> {
        self.with_client(|client| async move {
            let mut query = String::from(
                "SELECT id, step, repo, worktree, wave_run_id, status,
                        started_at, ended_at, pid, container_id, model, run_mode
                 FROM agents",
            );
            let mut params: Vec<Box<dyn ToSql + Sync>> = Vec::new();
            let mut where_clauses = Vec::new();

            if let Some(worktree) = worktree {
                where_clauses.push(format!("worktree = ${}", params.len() + 1));
                params.push(Box::new(worktree.to_string()));
            }
            if let Some(repo) = repo {
                where_clauses.push(format!("repo = ${}", params.len() + 1));
                params.push(Box::new(repo.to_string()));
            }
            if !where_clauses.is_empty() {
                query.push_str(" WHERE ");
                query.push_str(&where_clauses.join(" AND "));
            }
            query.push_str(" ORDER BY started_at DESC");
            if let Some(limit) = limit {
                query.push_str(&format!(" LIMIT ${}", params.len() + 1));
                params.push(Box::new(limit as i64));
            }

            let params_ref: Vec<&(dyn ToSql + Sync)> = params.iter().map(|v| v.as_ref()).collect();
            let rows = client.query(&query, &params_ref).await?;
            rows.iter().map(map_agent_row).collect()
        })
    }

    fn get_agent(&self, agent_id: &LfdId) -> StoreResult<Option<Agent>> {
        self.with_client(|client| async move {
            let row = client
                .query_opt(
                    "SELECT id, step, repo, worktree, wave_run_id, status,
                            started_at, ended_at, pid, container_id, model, run_mode
                     FROM agents WHERE id = $1",
                    &[&agent_id],
                )
                .await?;
            row.as_ref().map(map_agent_row).transpose()
        })
    }

    fn get_waiting_agent_for_wave(&self, wave_id: &LfdId) -> StoreResult<Option<Agent>> {
        self.with_client(|client| async move {
            let row = client
                .query_opt(
                    "SELECT a.id, a.step, a.repo, a.worktree, a.wave_run_id, a.status,
                            a.started_at, a.ended_at, a.pid, a.container_id, a.model, a.run_mode
                     FROM agents a JOIN wave_runs r ON a.wave_run_id = r.id
                     WHERE r.wave_id = $1 AND a.status = $2
                     ORDER BY a.started_at DESC LIMIT 1",
                    &[&wave_id, &AgentStatus::Waiting.as_i32()],
                )
                .await?;
            row.as_ref().map(map_agent_row).transpose()
        })
    }

    fn start_agent(&self, agent: &Agent) -> StoreResult<()> {
        self.with_client(|client| async move {
            let started_at = agent
                .started_at
                .map(|dt| dt.unix_timestamp())
                .unwrap_or_else(now_unix);
            let ended_at = agent.ended_at.map(|dt| dt.unix_timestamp());
            let pid = agent.pid.map(|v| v as i32);
            let container_id = agent.container_id.as_deref();
            client
                .execute(
                    "INSERT INTO agents (
                        id, step, repo, worktree, wave_run_id, status, started_at,
                        ended_at, pid, container_id, model, run_mode
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
                    &[
                        &agent.id,
                        &agent.step,
                        &agent.repo,
                        &agent.worktree,
                        &agent.wave_run_id,
                        &agent.status.as_i32(),
                        &started_at,
                        &ended_at,
                        &pid,
                        &container_id,
                        &agent.model,
                        &agent.run_mode,
                    ],
                )
                .await?;
            Ok(())
        })
    }

    fn update_agent_status(
        &self,
        agent_id: &LfdId,
        status: i32,
        pid: Option<u32>,
        container_id: Option<&str>,
    ) -> StoreResult<()> {
        self.with_client(|client| async move {
            let pid = pid.map(|v| v as i32);
            let container_id = container_id.map(str::to_string);
            let updated = client
                .execute(
                    "UPDATE agents
                     SET status = $1,
                         pid = COALESCE($2, pid),
                         container_id = COALESCE($3, container_id)
                     WHERE id = $4",
                    &[&status, &pid, &container_id, &agent_id],
                )
                .await?;
            if updated == 0 {
                return Err(StoreError::NotFound);
            }
            Ok(())
        })
    }

    fn end_agent(&self, agent_id: &LfdId, status: i32, ended_at: i64) -> StoreResult<()> {
        self.with_client(|client| async move {
            let updated = client
                .execute(
                    "UPDATE agents SET status = $1, ended_at = $2 WHERE id = $3",
                    &[&status, &ended_at, &agent_id],
                )
                .await?;
            if updated == 0 {
                return Err(StoreError::NotFound);
            }
            Ok(())
        })
    }

    fn get_active_agents_for_wave(&self, wave_id: &LfdId) -> StoreResult<Vec<Agent>> {
        self.with_client(|client| async move {
            let rows = client
                .query(
                    "SELECT a.id, a.step, a.repo, a.worktree, a.wave_run_id, a.status,
                            a.started_at, a.ended_at, a.pid, a.container_id, a.model, a.run_mode
                     FROM agents a JOIN wave_runs r ON a.wave_run_id = r.id
                     WHERE r.wave_id = $1 AND a.ended_at IS NULL
                     ORDER BY a.started_at DESC",
                    &[&wave_id],
                )
                .await?;
            rows.iter().map(map_agent_row).collect()
        })
    }

    fn end_active_agent_for_wave(
        &self,
        wave_id: &LfdId,
        status: i32,
        ended_at: i64,
    ) -> StoreResult<()> {
        self.with_client(|client| async move {
            let updated = client
                .execute(
                    "UPDATE agents SET status = $1, ended_at = $2
                     WHERE wave_run_id IN (SELECT id FROM wave_runs WHERE wave_id = $3)
                     AND ended_at IS NULL",
                    &[&status, &ended_at, &wave_id.as_str()],
                )
                .await?;
            if updated == 0 {
                return Err(StoreError::NotFound);
            }
            Ok(())
        })
    }

    fn get_stuck_agents(&self, older_than_secs: u64) -> StoreResult<Vec<Agent>> {
        self.with_client(|client| async move {
            let cutoff = now_unix() - older_than_secs as i64;
            let rows = client
                .query(
                    "SELECT id, step, repo, worktree, wave_run_id, status,
                            started_at, ended_at, pid, container_id, model, run_mode
                     FROM agents WHERE ended_at IS NULL AND started_at <= $1
                     ORDER BY started_at ASC",
                    &[&cutoff],
                )
                .await?;
            rows.iter().map(map_agent_row).collect()
        })
    }

    fn get_summary(&self, wave_id: &LfdId) -> StoreResult<Option<Summary>> {
        self.with_client(|client| async move {
            let row = client
                .query_opt(
                    "SELECT id, wave_id, content, source_hash, token_budget, model, created_at
                     FROM summaries WHERE wave_id = $1",
                    &[&wave_id],
                )
                .await?;
            row.as_ref().map(map_summary_row).transpose()
        })
    }

    fn upsert_summary(&self, summary: &Summary) -> StoreResult<()> {
        self.with_client(|client| async move {
            let created_at = summary
                .created_at
                .map(|dt| dt.unix_timestamp())
                .unwrap_or_else(now_unix);

            client
                .execute(
                    "INSERT INTO summaries (id, wave_id, content, source_hash, token_budget, model, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7)
                     ON CONFLICT(wave_id) DO UPDATE SET
                         content = excluded.content,
                         source_hash = excluded.source_hash,
                         token_budget = excluded.token_budget,
                         model = excluded.model,
                         created_at = excluded.created_at",
                    &[
                        &summary.id,
                        &summary.wave_id,
                        &summary.content,
                        &summary.source_hash,
                        &(summary.token_budget as i32),
                        &summary.model,
                        &created_at,
                    ],
                )
                .await?;
            Ok(())
        })
    }

    fn list_chat_memory_blocks(&self, wave_id: &LfdId) -> StoreResult<Vec<ChatMemoryBlock>> {
        self.with_client(|client| async move {
            let rows = client
                .query(
                    "SELECT wave_id, name, content, position, updated_at
                     FROM chat_memory_blocks
                     WHERE wave_id = $1
                     ORDER BY position ASC, name ASC",
                    &[&wave_id],
                )
                .await?;
            rows.iter().map(map_chat_memory_block_row).collect()
        })
    }

    fn upsert_chat_memory_block(&self, block: &ChatMemoryBlock) -> StoreResult<()> {
        self.with_client(|client| async move {
            let updated_at = block
                .updated_at
                .map(|dt| dt.unix_timestamp())
                .unwrap_or_else(now_unix);
            client
                .execute(
                    "INSERT INTO chat_memory_blocks (wave_id, name, content, position, updated_at)
                     VALUES ($1, $2, $3, $4, $5)
                     ON CONFLICT(wave_id, name) DO UPDATE SET
                         content = excluded.content,
                         position = excluded.position,
                         updated_at = excluded.updated_at",
                    &[
                        &block.wave_id,
                        &block.name,
                        &block.content,
                        &(block.position as i32),
                        &updated_at,
                    ],
                )
                .await?;
            Ok(())
        })
    }

    fn delete_chat_memory_block(&self, wave_id: &LfdId, name: &str) -> StoreResult<()> {
        let name = name.to_string();
        self.with_client(|client| async move {
            client
                .execute(
                    "DELETE FROM chat_memory_blocks WHERE wave_id = $1 AND name = $2",
                    &[&wave_id, &name],
                )
                .await?;
            Ok(())
        })
    }
}

fn build_pool(database_url: &str) -> StoreResult<Pool> {
    let config: tokio_postgres::Config = database_url
        .parse()
        .map_err(|err| StoreError::InvalidData(format!("invalid database url: {err}")))?;
    let manager = Manager::new(config, NoTls);
    let pool = Pool::builder(manager)
        .max_size(16)
        .build()
        .map_err(|err| StoreError::InvalidData(format!("failed to build pool: {err}")))?;
    Ok(pool)
}

async fn get_client_with_retry(
    pool: &Pool,
) -> Result<deadpool_postgres::Client, deadpool_postgres::PoolError> {
    let mut last_error = None;
    for (attempt, delay) in RETRY_DELAYS.iter().enumerate() {
        match pool.get().await {
            Ok(client) => return Ok(client),
            Err(err) => {
                last_error = Some(err);
                if attempt < RETRY_DELAYS.len() - 1 {
                    tokio::time::sleep(*delay).await;
                }
            }
        }
    }
    Err(last_error.expect("retry loop always sets last_error"))
}
