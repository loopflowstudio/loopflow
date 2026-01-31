use std::future::Future;

use deadpool_postgres::{Manager, Pool};
use prost_types::Timestamp;
use tokio_postgres::error::SqlState;
use tokio_postgres::types::ToSql;
use tokio_postgres::{NoTls, Row};

use crate::proto::control::{PendingActivation, StepRun, StepRunStatus, Stimulus, Wave};
use crate::store::{ForkRun, ForkRunStatus, RunStore, StoreError, StoreResult};

const SCHEMA_VERSION: u32 = 1;
const MIGRATION_001: &str = include_str!("migrations/postgres/001_initial.sql");

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
        if version == 0 {
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
        let version = schema_version_async(&pool).await?;
        if version == 0 {
            return Err(StoreError::InvalidData(
                "postgres schema missing; run `lfd migrate`".to_string(),
            ));
        }
        Ok(Self { pool, runtime })
    }

    #[allow(dead_code)]
    pub fn migrate(database_url: &str) -> StoreResult<u32> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to build postgres runtime");
        let pool = build_pool(database_url)?;
        let migrator = PostgresMigrator { pool, runtime };
        migrator.migrate()
    }

    pub async fn migrate_async(database_url: &str) -> StoreResult<u32> {
        migrate_async(database_url).await
    }

    pub async fn migrate_status_async(database_url: &str) -> StoreResult<u32> {
        schema_version_direct(database_url).await
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
            let client = self.pool.get().await?;
            func(client).await
        })
    }

    fn read_waves(&self, repo: Option<&str>) -> StoreResult<Vec<Wave>> {
        self.with_client(|client| async move {
            let rows = if let Some(repo) = repo {
                client
                    .query(
                        "
                        SELECT id, name, repo, flow, direction, area, paused, status, iteration,
                               worktree, branch, pr_limit, merge_mode, pid, created_at,
                               consecutive_failures, pending_activations, step_index
                        FROM waves
                        WHERE repo = $1
                        ORDER BY created_at DESC
                        ",
                        &[&repo],
                    )
                    .await?
            } else {
                client
                    .query(
                        "
                        SELECT id, name, repo, flow, direction, area, paused, status, iteration,
                               worktree, branch, pr_limit, merge_mode, pid, created_at,
                               consecutive_failures, pending_activations, step_index
                        FROM waves
                        ORDER BY created_at DESC
                        ",
                        &[],
                    )
                    .await?
            };

            let mut waves = Vec::new();
            for row in rows {
                waves.push(map_wave_row(&row)?);
            }
            Ok(waves)
        })
    }

    fn upsert_wave(&self, wave: &Wave) -> StoreResult<()> {
        self.with_client(|client| async move {
            let direction_json = serde_json::to_value(&wave.direction)?;
            let area_json = serde_json::to_value(&wave.area)?;
            let created_at = wave
                .created_at
                .as_ref()
                .map(timestamp_to_unix)
                .unwrap_or_else(now_unix);

            let pid = wave.pid.map(|value| value as i32);
            client
                .execute(
                    "
                    INSERT INTO waves (
                        id, name, repo, flow, direction, area, stimulus_kind, stimulus_cron,
                        paused, status, iteration, worktree, branch, pr_limit, merge_mode, pid,
                        created_at, last_main_sha, consecutive_failures, pending_activations,
                        step_index
                    ) VALUES ($1, $2, $3, $4, $5, $6, 1, '', $7, $8, $9, $10, $11, $12, $13, $14, $15, NULL, $16, $17, $18)
                    ON CONFLICT(id) DO UPDATE SET
                        name = excluded.name,
                        repo = excluded.repo,
                        flow = excluded.flow,
                        direction = excluded.direction,
                        area = excluded.area,
                        paused = excluded.paused,
                        status = excluded.status,
                        iteration = excluded.iteration,
                        worktree = excluded.worktree,
                        branch = excluded.branch,
                        pr_limit = excluded.pr_limit,
                        merge_mode = excluded.merge_mode,
                        pid = excluded.pid,
                        created_at = excluded.created_at,
                        consecutive_failures = excluded.consecutive_failures,
                        pending_activations = excluded.pending_activations,
                        step_index = excluded.step_index
                    ",
                    &[
                        &wave.id,
                        &wave.name,
                        &wave.repo,
                        &wave.flow,
                        &direction_json,
                        &area_json,
                        &wave.paused,
                        &wave.status,
                        &(wave.iteration as i32),
                        &wave.worktree,
                        &wave.branch,
                        &(wave.pr_limit as i32),
                        &wave.merge_mode,
                        &pid,
                        &created_at,
                        &(wave.consecutive_failures as i32),
                        &(wave.pending_activations as i32),
                        &(wave.step_index as i32),
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

    fn schema_version(&self) -> StoreResult<u32> {
        self.with_client(|client| async move {
            match client
                .query_opt("SELECT value FROM meta WHERE key = 'schema_version'", &[])
                .await
            {
                Ok(Some(row)) => {
                    let value: String = row.get(0);
                    let parsed = value.parse::<u32>().unwrap_or(SCHEMA_VERSION);
                    Ok(parsed)
                }
                Ok(None) => Ok(0),
                Err(err) => {
                    if is_undefined_table(&err) {
                        return Ok(0);
                    }
                    Err(err.into())
                }
            }
        })
    }

    fn list_waves(&self, repo: Option<&str>) -> StoreResult<Vec<Wave>> {
        self.read_waves(repo)
    }

    fn get_wave(&self, wave_id: &str) -> StoreResult<Option<Wave>> {
        self.with_client(|client| async move {
            let row = client
                .query_opt(
                    "
                    SELECT id, name, repo, flow, direction, area, paused, status, iteration,
                           worktree, branch, pr_limit, merge_mode, pid, created_at,
                           consecutive_failures, pending_activations, step_index
                    FROM waves
                    WHERE id = $1
                    ",
                    &[&wave_id],
                )
                .await?;

            row.map(|row| map_wave_row(&row)).transpose()
        })
    }

    fn create_wave(&self, wave: &Wave) -> StoreResult<()> {
        self.upsert_wave(wave)
    }

    fn update_wave(&self, wave: &Wave) -> StoreResult<()> {
        self.upsert_wave(wave)
    }

    fn delete_wave(&self, wave_id: &str) -> StoreResult<()> {
        self.with_client(|client| async move {
            client
                .execute("DELETE FROM waves WHERE id = $1", &[&wave_id])
                .await?;
            Ok(())
        })
    }

    fn list_stimuli(&self, wave_id: Option<&str>) -> StoreResult<Vec<Stimulus>> {
        self.with_client(|client| async move {
            let rows = if let Some(wave_id) = wave_id {
                client
                    .query(
                        "SELECT id, wave_id, kind, cron, last_main_sha, last_triggered_at, enabled, created_at
                         FROM stimuli WHERE wave_id = $1 ORDER BY created_at",
                        &[&wave_id],
                    )
                    .await?
            } else {
                client
                    .query(
                        "SELECT id, wave_id, kind, cron, last_main_sha, last_triggered_at, enabled, created_at
                         FROM stimuli ORDER BY created_at",
                        &[],
                    )
                    .await?
            };

            let mut stimuli = Vec::new();
            for row in rows {
                stimuli.push(map_stimulus_row(&row)?);
            }
            Ok(stimuli)
        })
    }

    fn list_stimuli_by_kind(&self, kind: i32) -> StoreResult<Vec<Stimulus>> {
        self.with_client(|client| async move {
            let rows = client
                .query(
                    "SELECT id, wave_id, kind, cron, last_main_sha, last_triggered_at, enabled, created_at
                     FROM stimuli WHERE kind = $1 ORDER BY created_at",
                    &[&kind],
                )
                .await?;
            let mut stimuli = Vec::new();
            for row in rows {
                stimuli.push(map_stimulus_row(&row)?);
            }
            Ok(stimuli)
        })
    }

    fn get_stimulus(&self, stimulus_id: &str) -> StoreResult<Option<Stimulus>> {
        self.with_client(|client| async move {
            let row = client
                .query_opt(
                    "SELECT id, wave_id, kind, cron, last_main_sha, last_triggered_at, enabled, created_at
                     FROM stimuli WHERE id = $1",
                    &[&stimulus_id],
                )
                .await?;
            row.map(|row| map_stimulus_row(&row)).transpose()
        })
    }

    fn create_stimulus(&self, stimulus: &Stimulus) -> StoreResult<()> {
        self.with_client(|client| async move {
            let created_at = stimulus
                .created_at
                .as_ref()
                .map(timestamp_to_unix)
                .unwrap_or_else(now_unix);

            client
                .execute(
                    "INSERT INTO stimuli (id, wave_id, kind, cron, last_main_sha, last_triggered_at, enabled, created_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                    &[
                        &stimulus.id,
                        &stimulus.wave_id,
                        &stimulus.kind,
                        &stimulus.cron,
                        &stimulus.last_main_sha,
                        &stimulus.last_triggered_at,
                        &stimulus.enabled,
                        &created_at,
                    ],
                )
                .await?;
            Ok(())
        })
    }

    fn update_stimulus(&self, stimulus: &Stimulus) -> StoreResult<()> {
        self.with_client(|client| async move {
            let updated = client
                .execute(
                    "UPDATE stimuli SET
                        kind = $1, cron = $2, last_main_sha = $3,
                        last_triggered_at = $4, enabled = $5
                     WHERE id = $6",
                    &[
                        &stimulus.kind,
                        &stimulus.cron,
                        &stimulus.last_main_sha,
                        &stimulus.last_triggered_at,
                        &stimulus.enabled,
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

    fn delete_stimulus(&self, stimulus_id: &str) -> StoreResult<()> {
        self.with_client(|client| async move {
            client
                .execute("DELETE FROM stimuli WHERE id = $1", &[&stimulus_id])
                .await?;
            Ok(())
        })
    }

    fn delete_stimuli_for_wave(&self, wave_id: &str) -> StoreResult<u32> {
        self.with_client(|client| async move {
            let deleted = client
                .execute("DELETE FROM stimuli WHERE wave_id = $1", &[&wave_id])
                .await?;
            Ok(deleted as u32)
        })
    }

    fn list_pending_activations(&self, wave_id: &str) -> StoreResult<Vec<PendingActivation>> {
        self.with_client(|client| async move {
            let rows = client
                .query(
                    "SELECT id, wave_id, stimulus_id, from_sha, to_sha, queued_at
                     FROM pending_activations WHERE wave_id = $1 ORDER BY queued_at",
                    &[&wave_id],
                )
                .await?;
            let mut activations = Vec::new();
            for row in rows {
                activations.push(map_pending_activation_row(&row)?);
            }
            Ok(activations)
        })
    }

    fn create_pending_activation(&self, activation: &PendingActivation) -> StoreResult<()> {
        self.with_client(|client| async move {
            let mut client = client;
            let transaction = client.transaction().await?;
            transaction
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

            transaction
                .execute(
                    "UPDATE waves SET pending_activations = (
                        SELECT COUNT(*)::INTEGER FROM pending_activations WHERE wave_id = $1
                     ) WHERE id = $1",
                    &[&activation.wave_id],
                )
                .await?;
            transaction.commit().await?;
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

    fn delete_pending_activations(&self, wave_id: &str) -> StoreResult<u32> {
        self.with_client(|client| async move {
            let mut client = client;
            let transaction = client.transaction().await?;
            let deleted = transaction
                .execute(
                    "DELETE FROM pending_activations WHERE wave_id = $1",
                    &[&wave_id],
                )
                .await?;
            transaction
                .execute(
                    "UPDATE waves SET pending_activations = 0 WHERE id = $1",
                    &[&wave_id],
                )
                .await?;
            transaction.commit().await?;
            Ok(deleted as u32)
        })
    }

    fn get_pending_for_stimulus(
        &self,
        wave_id: &str,
        stimulus_id: &str,
    ) -> StoreResult<Option<PendingActivation>> {
        self.with_client(|client| async move {
            let row = client
                .query_opt(
                    "SELECT id, wave_id, stimulus_id, from_sha, to_sha, queued_at
                     FROM pending_activations WHERE wave_id = $1 AND stimulus_id = $2",
                    &[&wave_id, &stimulus_id],
                )
                .await?;
            row.map(|row| map_pending_activation_row(&row)).transpose()
        })
    }

    fn list_fork_runs(&self, wave_id: &str, step_index: u32) -> StoreResult<Vec<ForkRun>> {
        self.with_client(|client| async move {
            let rows = client
                .query(
                    "SELECT id, wave_id, step_index, branch_index, status, worktree
                     FROM fork_runs WHERE wave_id = $1 AND step_index = $2
                     ORDER BY branch_index ASC",
                    &[&wave_id, &(step_index as i32)],
                )
                .await?;
            let mut runs = Vec::new();
            for row in rows {
                runs.push(map_fork_run_row(&row)?);
            }
            Ok(runs)
        })
    }

    fn upsert_fork_run(&self, fork_run: &ForkRun) -> StoreResult<()> {
        self.with_client(|client| async move {
            client
                .execute(
                    "
                    INSERT INTO fork_runs (id, wave_id, step_index, branch_index, status, worktree)
                    VALUES ($1, $2, $3, $4, $5, $6)
                    ON CONFLICT(id) DO UPDATE SET
                        wave_id = excluded.wave_id,
                        step_index = excluded.step_index,
                        branch_index = excluded.branch_index,
                        status = excluded.status,
                        worktree = excluded.worktree
                    ",
                    &[
                        &fork_run.id,
                        &fork_run.wave_id,
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

    fn delete_fork_runs(&self, wave_id: &str, step_index: u32) -> StoreResult<u32> {
        self.with_client(|client| async move {
            let deleted = client
                .execute(
                    "DELETE FROM fork_runs WHERE wave_id = $1 AND step_index = $2",
                    &[&wave_id, &(step_index as i32)],
                )
                .await?;
            Ok(deleted as u32)
        })
    }

    fn list_step_runs(&self) -> StoreResult<Vec<StepRun>> {
        self.list_step_run_history(None, None, None)
    }

    fn list_step_run_history(
        &self,
        worktree: Option<&str>,
        repo: Option<&str>,
        limit: Option<u32>,
    ) -> StoreResult<Vec<StepRun>> {
        self.with_client(|client| async move {
            let mut query = String::from(
                "
                SELECT id, step, repo, worktree, flow_run_id, wave_id, status,
                       started_at, ended_at, pid, model, run_mode
                FROM step_runs
                ",
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
                params.push(Box::new(limit as i32));
            }

            let params_ref: Vec<&(dyn ToSql + Sync)> =
                params.iter().map(|value| value.as_ref()).collect();
            let rows = client.query(&query, &params_ref).await?;
            let mut runs = Vec::new();
            for row in rows {
                runs.push(map_step_run_row(&row)?);
            }
            Ok(runs)
        })
    }

    fn get_step_run(&self, step_run_id: &str) -> StoreResult<Option<StepRun>> {
        self.with_client(|client| async move {
            let row = client
                .query_opt(
                    "
                    SELECT id, step, repo, worktree, flow_run_id, wave_id, status,
                           started_at, ended_at, pid, model, run_mode
                    FROM step_runs
                    WHERE id = $1
                    ",
                    &[&step_run_id],
                )
                .await?;
            row.map(|row| map_step_run_row(&row)).transpose()
        })
    }

    fn get_waiting_step_run(&self, wave_id: &str) -> StoreResult<Option<StepRun>> {
        self.with_client(|client| async move {
            let row = client
                .query_opt(
                    "
                    SELECT id, step, repo, worktree, flow_run_id, wave_id, status,
                           started_at, ended_at, pid, model, run_mode
                    FROM step_runs
                    WHERE wave_id = $1 AND status = $2
                    ORDER BY started_at DESC
                    LIMIT 1
                    ",
                    &[&wave_id, &(StepRunStatus::StepWaiting as i32)],
                )
                .await?;
            row.map(|row| map_step_run_row(&row)).transpose()
        })
    }

    fn start_step_run(&self, step_run: &StepRun) -> StoreResult<()> {
        self.with_client(|client| async move {
            let started_at = step_run
                .started_at
                .as_ref()
                .map(timestamp_to_unix)
                .unwrap_or_else(now_unix);
            let ended_at = step_run.ended_at.as_ref().map(timestamp_to_unix);
            let pid = step_run.pid.map(|value| value as i32);
            client
                .execute(
                    "
                    INSERT INTO step_runs (
                        id, step, repo, worktree, flow_run_id, wave_id, status, started_at,
                        ended_at, pid, model, run_mode
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                    ",
                    &[
                        &step_run.id,
                        &step_run.step,
                        &step_run.repo,
                        &step_run.worktree,
                        &step_run.flow_run_id,
                        &step_run.wave_id,
                        &step_run.status,
                        &started_at,
                        &ended_at,
                        &pid,
                        &step_run.model,
                        &step_run.run_mode,
                    ],
                )
                .await?;
            Ok(())
        })
    }

    fn update_step_run_status(
        &self,
        step_run_id: &str,
        status: i32,
        pid: Option<u32>,
    ) -> StoreResult<()> {
        self.with_client(|client| async move {
            let pid = pid.map(|value| value as i32);
            let updated = client
                .execute(
                    "UPDATE step_runs SET status = $1, pid = COALESCE($2, pid) WHERE id = $3",
                    &[&status, &pid, &step_run_id],
                )
                .await?;
            if updated == 0 {
                return Err(StoreError::NotFound);
            }
            Ok(())
        })
    }

    fn end_step_run(&self, step_run_id: &str, status: i32, ended_at: i64) -> StoreResult<()> {
        self.with_client(|client| async move {
            let updated = client
                .execute(
                    "UPDATE step_runs SET status = $1, ended_at = $2 WHERE id = $3",
                    &[&status, &ended_at, &step_run_id],
                )
                .await?;
            if updated == 0 {
                return Err(StoreError::NotFound);
            }
            Ok(())
        })
    }

    fn get_stuck_step_runs(&self, older_than_secs: u64) -> StoreResult<Vec<StepRun>> {
        self.with_client(|client| async move {
            let cutoff = now_unix() - older_than_secs as i64;
            let rows = client
                .query(
                    "
                    SELECT id, step, repo, worktree, flow_run_id, wave_id, status,
                           started_at, ended_at, pid, model, run_mode
                    FROM step_runs
                    WHERE ended_at IS NULL AND started_at <= $1
                    ORDER BY started_at ASC
                    ",
                    &[&cutoff],
                )
                .await?;
            let mut runs = Vec::new();
            for row in rows {
                runs.push(map_step_run_row(&row)?);
            }
            Ok(runs)
        })
    }
}

#[allow(dead_code)]
struct PostgresMigrator {
    pool: Pool,
    runtime: tokio::runtime::Runtime,
}

#[allow(dead_code)]
impl PostgresMigrator {
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
            let client = self.pool.get().await?;
            func(client).await
        })
    }

    fn schema_version(&self) -> StoreResult<u32> {
        self.with_client(|client| async move { schema_version_client(&client).await })
    }

    fn migrate(&self) -> StoreResult<u32> {
        let current = self.schema_version()?;
        if current >= SCHEMA_VERSION {
            return Ok(current);
        }

        self.with_client(|client| async move {
            let mut client = client;
            let transaction = client.transaction().await?;
            transaction.batch_execute(MIGRATION_001).await?;
            transaction.commit().await?;
            Ok(SCHEMA_VERSION)
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

fn unix_to_timestamp(seconds: i64) -> Timestamp {
    Timestamp { seconds, nanos: 0 }
}

fn timestamp_to_unix(ts: &Timestamp) -> i64 {
    ts.seconds
}

fn now_unix() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

fn map_wave_row(row: &Row) -> StoreResult<Wave> {
    let direction_json: serde_json::Value = row.get(4);
    let area_json: serde_json::Value = row.get(5);
    let direction = parse_json_vec(direction_json)?;
    let area = parse_json_vec(area_json)?;

    let created_at = unix_to_timestamp(row.get::<_, i64>(14));
    let pid: Option<i32> = row.get(13);

    Ok(Wave {
        id: row.get(0),
        name: row.get(1),
        repo: row.get(2),
        flow: row.get(3),
        direction,
        area,
        paused: row.get(6),
        status: row.get::<_, i32>(7),
        iteration: row.get::<_, i32>(8) as u32,
        worktree: row.get(9),
        branch: row.get(10),
        pr_limit: row.get::<_, i32>(11) as u32,
        merge_mode: row.get::<_, i32>(12),
        pid: pid.map(|value| value as u32),
        created_at: Some(created_at),
        consecutive_failures: row.get::<_, i32>(15) as u32,
        pending_activations: row.get::<_, i32>(16) as u32,
        step_index: row.get::<_, i32>(17) as u32,
    })
}

fn map_stimulus_row(row: &Row) -> StoreResult<Stimulus> {
    let created_at = unix_to_timestamp(row.get::<_, i64>(7));

    Ok(Stimulus {
        id: row.get(0),
        wave_id: row.get(1),
        kind: row.get::<_, i32>(2),
        cron: row.get(3),
        last_main_sha: row.get(4),
        last_triggered_at: row.get(5),
        enabled: row.get(6),
        created_at: Some(created_at),
    })
}

fn map_pending_activation_row(row: &Row) -> StoreResult<PendingActivation> {
    Ok(PendingActivation {
        id: row.get(0),
        wave_id: row.get(1),
        stimulus_id: row.get(2),
        from_sha: row.get(3),
        to_sha: row.get(4),
        queued_at: row.get(5),
    })
}

fn map_fork_run_row(row: &Row) -> StoreResult<ForkRun> {
    let status_value: i32 = row.get(4);
    let status = ForkRunStatus::from_i64(status_value as i64)
        .ok_or_else(|| StoreError::InvalidData("invalid fork run status".to_string()))?;

    Ok(ForkRun {
        id: row.get(0),
        wave_id: row.get(1),
        step_index: row.get::<_, i32>(2) as u32,
        branch_index: row.get::<_, i32>(3) as u32,
        status,
        worktree: row.get(5),
    })
}

fn map_step_run_row(row: &Row) -> StoreResult<StepRun> {
    let started_at = unix_to_timestamp(row.get::<_, i64>(7));
    let ended_at: Option<i64> = row.get(8);
    let pid: Option<i32> = row.get(9);

    Ok(StepRun {
        id: row.get(0),
        step: row.get(1),
        repo: row.get(2),
        worktree: row.get(3),
        flow_run_id: row.get(4),
        wave_id: row.get(5),
        status: row.get::<_, i32>(6),
        started_at: Some(started_at),
        ended_at: ended_at.map(unix_to_timestamp),
        pid: pid.map(|value| value as u32),
        model: row.get(10),
        run_mode: row.get(11),
    })
}

fn parse_json_vec(value: serde_json::Value) -> StoreResult<Vec<String>> {
    serde_json::from_value::<Vec<String>>(value).map_err(StoreError::Serde)
}

fn is_undefined_table(err: &tokio_postgres::Error) -> bool {
    err.as_db_error()
        .map(|db_error| db_error.code() == &SqlState::UNDEFINED_TABLE)
        .unwrap_or(false)
}

async fn schema_version_client(client: &tokio_postgres::Client) -> StoreResult<u32> {
    match client
        .query_opt("SELECT value FROM meta WHERE key = 'schema_version'", &[])
        .await
    {
        Ok(Some(row)) => {
            let value: String = row.get(0);
            let parsed = value.parse::<u32>().unwrap_or(SCHEMA_VERSION);
            Ok(parsed)
        }
        Ok(None) => Ok(0),
        Err(err) => {
            if is_undefined_table(&err) {
                return Ok(0);
            }
            Err(err.into())
        }
    }
}

async fn schema_version_async(pool: &Pool) -> StoreResult<u32> {
    let client = pool.get().await?;
    schema_version_client(&client).await
}

async fn schema_version_direct(database_url: &str) -> StoreResult<u32> {
    let (client, connection) = tokio_postgres::connect(database_url, NoTls).await?;
    let connection_task = tokio::spawn(async move {
        let _ = connection.await;
    });
    let result = schema_version_client(&client).await;
    connection_task.abort();
    result
}

async fn migrate_async(database_url: &str) -> StoreResult<u32> {
    let current = schema_version_direct(database_url).await?;
    if current >= SCHEMA_VERSION {
        return Ok(current);
    }

    let (mut client, connection) = tokio_postgres::connect(database_url, NoTls).await?;
    let connection_task = tokio::spawn(async move {
        let _ = connection.await;
    });
    let transaction = client.transaction().await?;
    transaction.batch_execute(MIGRATION_001).await?;
    transaction.commit().await?;
    connection_task.abort();
    Ok(SCHEMA_VERSION)
}
