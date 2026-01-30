use std::collections::HashSet;
use std::path::Path;
use std::sync::Mutex;

use prost_types::Timestamp;
use rusqlite::types::Type;
use rusqlite::{params, Connection, OptionalExtension, Row, ToSql};
use time::OffsetDateTime;

use crate::proto::control::{PendingActivation, StepRun, StepRunStatus, Stimulus, Wave};
use crate::store::{ForkRun, ForkRunStatus, RunStore, StoreError, StoreResult};

const SCHEMA_VERSION: u32 = 1;

#[derive(Debug)]
pub struct SqliteStore {
    conn: Mutex<Connection>,
}

impl SqliteStore {
    pub fn new(path: &Path) -> StoreResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| {
                StoreError::InvalidData(format!("failed to create db dir: {err}"))
            })?;
        }

        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode = WAL;")?;

        let store = Self {
            conn: Mutex::new(conn),
        };
        store.ensure_schema()?;
        Ok(store)
    }

    fn ensure_schema(&self) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            INSERT OR IGNORE INTO meta (key, value) VALUES ('schema_version', '1');

            CREATE TABLE IF NOT EXISTS waves (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                repo TEXT NOT NULL,
                flow TEXT NOT NULL,
                direction TEXT NOT NULL,
                area TEXT NOT NULL,
                stimulus_kind INTEGER NOT NULL,
                stimulus_cron TEXT NOT NULL,
                paused INTEGER NOT NULL,
                status INTEGER NOT NULL,
                iteration INTEGER NOT NULL,
                worktree TEXT NOT NULL,
                branch TEXT NOT NULL,
                pr_limit INTEGER NOT NULL,
                merge_mode INTEGER NOT NULL,
                pid INTEGER,
                created_at INTEGER NOT NULL,
                last_main_sha TEXT,
                consecutive_failures INTEGER NOT NULL,
                pending_activations INTEGER NOT NULL,
                step_index INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS step_runs (
                id TEXT PRIMARY KEY,
                step TEXT NOT NULL,
                repo TEXT NOT NULL,
                worktree TEXT NOT NULL,
                flow_run_id TEXT,
                wave_id TEXT,
                status INTEGER NOT NULL,
                started_at INTEGER NOT NULL,
                ended_at INTEGER,
                pid INTEGER,
                model TEXT NOT NULL,
                run_mode TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS stimuli (
                id TEXT PRIMARY KEY,
                wave_id TEXT NOT NULL,
                kind INTEGER NOT NULL,
                cron TEXT NOT NULL DEFAULT '',
                area TEXT NOT NULL DEFAULT '[]',
                last_main_sha TEXT,
                last_triggered_at INTEGER,
                enabled INTEGER NOT NULL DEFAULT 1,
                created_at INTEGER NOT NULL,
                FOREIGN KEY (wave_id) REFERENCES waves(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_stimuli_wave_id ON stimuli(wave_id);
            CREATE INDEX IF NOT EXISTS idx_stimuli_kind ON stimuli(kind);

            CREATE TABLE IF NOT EXISTS pending_activations (
                id TEXT PRIMARY KEY,
                wave_id TEXT NOT NULL,
                stimulus_id TEXT NOT NULL,
                from_sha TEXT NOT NULL DEFAULT '',
                to_sha TEXT NOT NULL DEFAULT '',
                queued_at INTEGER NOT NULL,
                FOREIGN KEY (wave_id) REFERENCES waves(id) ON DELETE CASCADE,
                FOREIGN KEY (stimulus_id) REFERENCES stimuli(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_pending_wave_id ON pending_activations(wave_id);

            CREATE TABLE IF NOT EXISTS fork_runs (
                id TEXT PRIMARY KEY,
                wave_id TEXT NOT NULL,
                step_index INTEGER NOT NULL,
                branch_index INTEGER NOT NULL,
                status INTEGER NOT NULL,
                worktree TEXT NOT NULL,
                FOREIGN KEY (wave_id) REFERENCES waves(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_fork_runs_wave_id ON fork_runs(wave_id, step_index);
            ",
        )?;
        ensure_column(&conn, "waves", "step_index", "INTEGER NOT NULL DEFAULT 0")?;

        // Migrate existing stimulus data from waves to stimuli table
        Self::migrate_stimuli_from_waves(&conn)?;

        Ok(())
    }

    fn migrate_stimuli_from_waves(conn: &Connection) -> StoreResult<()> {
        // Check if we have old-style waves with stimulus_kind that haven't been migrated
        let needs_migration: bool = conn
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM waves w
                    WHERE w.stimulus_kind IN (2, 3, 4)
                    AND NOT EXISTS (SELECT 1 FROM stimuli s WHERE s.wave_id = w.id)
                )",
                [],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if !needs_migration {
            return Ok(());
        }

        // Migrate waves with loop/watch/cron stimuli
        conn.execute(
            "
            INSERT INTO stimuli (id, wave_id, kind, cron, last_main_sha, enabled, created_at)
            SELECT
                lower(hex(randomblob(16))),
                id,
                stimulus_kind,
                COALESCE(stimulus_cron, ''),
                last_main_sha,
                1,
                created_at
            FROM waves
            WHERE stimulus_kind IN (2, 3, 4)
            AND NOT EXISTS (SELECT 1 FROM stimuli WHERE stimuli.wave_id = waves.id)
            ",
            [],
        )?;

        tracing::info!("migrated stimulus data from waves to stimuli table");
        Ok(())
    }

    fn read_waves(&self, repo: Option<&str>) -> StoreResult<Vec<Wave>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = if repo.is_some() {
            conn.prepare(
                "
                SELECT id, name, repo, flow, direction, area, stimulus_kind, stimulus_cron,
                       paused, status, iteration, worktree, branch, pr_limit, merge_mode, pid,
                       created_at, last_main_sha, consecutive_failures, pending_activations,
                       step_index
                FROM waves
                WHERE repo = ?1
                ORDER BY created_at DESC
                ",
            )?
        } else {
            conn.prepare(
                "
                SELECT id, name, repo, flow, direction, area, stimulus_kind, stimulus_cron,
                       paused, status, iteration, worktree, branch, pr_limit, merge_mode, pid,
                       created_at, last_main_sha, consecutive_failures, pending_activations,
                       step_index
                FROM waves
                ORDER BY created_at DESC
                ",
            )?
        };

        let rows = if repo.is_some() {
            stmt.query_map(params![repo], map_wave_row)?
        } else {
            stmt.query_map([], map_wave_row)?
        };

        let mut waves = Vec::new();
        for wave in rows {
            waves.push(wave?);
        }
        Ok(waves)
    }

    fn upsert_wave(&self, wave: &Wave) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let direction_json = serde_json::to_string(&wave.direction)?;
        let area_json = serde_json::to_string(&wave.area)?;

        let created_at = wave
            .created_at
            .as_ref()
            .map(timestamp_to_unix)
            .unwrap_or_else(now_unix);

        // Note: stimulus_kind and stimulus_cron columns kept for backwards compat
        // but no longer used - stimuli are now in separate table
        conn.execute(
            "
            INSERT INTO waves (
                id, name, repo, flow, direction, area, stimulus_kind, stimulus_cron,
                paused, status, iteration, worktree, branch, pr_limit, merge_mode, pid,
                created_at, last_main_sha, consecutive_failures, pending_activations,
                step_index
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, '', ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, NULL, ?16, ?17, ?18)
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
            params![
                wave.id,
                wave.name,
                wave.repo,
                wave.flow,
                direction_json,
                area_json,
                if wave.paused { 1 } else { 0 },
                wave.status,
                wave.iteration,
                wave.worktree,
                wave.branch,
                wave.pr_limit,
                wave.merge_mode,
                wave.pid.map(|value| value as i64),
                created_at,
                wave.consecutive_failures,
                wave.pending_activations,
                wave.step_index,
            ],
        )?;
        Ok(())
    }
}

impl RunStore for SqliteStore {
    fn health_check(&self) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute("SELECT 1", [])?;
        Ok(())
    }

    fn schema_version(&self) -> StoreResult<u32> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let version: Option<String> = conn
            .query_row(
                "SELECT value FROM meta WHERE key = 'schema_version'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        let parsed = version
            .as_deref()
            .unwrap_or("1")
            .parse::<u32>()
            .unwrap_or(SCHEMA_VERSION);
        Ok(parsed)
    }

    fn list_waves(&self, repo: Option<&str>) -> StoreResult<Vec<Wave>> {
        self.read_waves(repo)
    }

    fn get_wave(&self, wave_id: &str) -> StoreResult<Option<Wave>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "
            SELECT id, name, repo, flow, direction, area, stimulus_kind, stimulus_cron,
                   paused, status, iteration, worktree, branch, pr_limit, merge_mode, pid,
                   created_at, last_main_sha, consecutive_failures, pending_activations,
                   step_index
            FROM waves
            WHERE id = ?1
            ",
        )?;

        let wave = stmt.query_row(params![wave_id], map_wave_row).optional()?;

        Ok(wave)
    }

    fn create_wave(&self, wave: &Wave) -> StoreResult<()> {
        self.upsert_wave(wave)
    }

    fn update_wave(&self, wave: &Wave) -> StoreResult<()> {
        self.upsert_wave(wave)
    }

    fn delete_wave(&self, wave_id: &str) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        // Stimuli and pending_activations cascade delete via foreign key
        conn.execute("DELETE FROM waves WHERE id = ?1", params![wave_id])?;
        Ok(())
    }

    // Stimulus methods

    fn list_stimuli(&self, wave_id: Option<&str>) -> StoreResult<Vec<Stimulus>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let (query, params): (&str, Vec<Box<dyn ToSql>>) = if let Some(wave_id) = wave_id {
            (
                "SELECT id, wave_id, kind, cron, last_main_sha, last_triggered_at, enabled, created_at
                 FROM stimuli WHERE wave_id = ?1 ORDER BY created_at",
                vec![Box::new(wave_id.to_string())],
            )
        } else {
            (
                "SELECT id, wave_id, kind, cron, last_main_sha, last_triggered_at, enabled, created_at
                 FROM stimuli ORDER BY created_at",
                vec![],
            )
        };

        let mut stmt = conn.prepare(query)?;
        let params_iter = params.iter().map(|v| v.as_ref() as &dyn ToSql);
        let rows = stmt.query_map(rusqlite::params_from_iter(params_iter), map_stimulus_row)?;

        let mut stimuli = Vec::new();
        for stimulus in rows {
            stimuli.push(stimulus?);
        }
        Ok(stimuli)
    }

    fn list_stimuli_by_kind(&self, kind: i32) -> StoreResult<Vec<Stimulus>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, wave_id, kind, cron, last_main_sha, last_triggered_at, enabled, created_at
             FROM stimuli WHERE kind = ?1 ORDER BY created_at",
        )?;

        let rows = stmt.query_map(params![kind], map_stimulus_row)?;
        let mut stimuli = Vec::new();
        for stimulus in rows {
            stimuli.push(stimulus?);
        }
        Ok(stimuli)
    }

    fn get_stimulus(&self, stimulus_id: &str) -> StoreResult<Option<Stimulus>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, wave_id, kind, cron, last_main_sha, last_triggered_at, enabled, created_at
             FROM stimuli WHERE id = ?1",
        )?;

        let stimulus = stmt
            .query_row(params![stimulus_id], map_stimulus_row)
            .optional()?;

        Ok(stimulus)
    }

    fn create_stimulus(&self, stimulus: &Stimulus) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let created_at = stimulus
            .created_at
            .as_ref()
            .map(timestamp_to_unix)
            .unwrap_or_else(now_unix);

        conn.execute(
            "INSERT INTO stimuli (id, wave_id, kind, cron, last_main_sha, last_triggered_at, enabled, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                stimulus.id,
                stimulus.wave_id,
                stimulus.kind,
                stimulus.cron,
                stimulus.last_main_sha,
                stimulus.last_triggered_at,
                if stimulus.enabled { 1 } else { 0 },
                created_at,
            ],
        )?;
        Ok(())
    }

    fn update_stimulus(&self, stimulus: &Stimulus) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");

        let updated = conn.execute(
            "UPDATE stimuli SET
                kind = ?1, cron = ?2, last_main_sha = ?3,
                last_triggered_at = ?4, enabled = ?5
             WHERE id = ?6",
            params![
                stimulus.kind,
                stimulus.cron,
                stimulus.last_main_sha,
                stimulus.last_triggered_at,
                if stimulus.enabled { 1 } else { 0 },
                stimulus.id,
            ],
        )?;
        if updated == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    fn delete_stimulus(&self, stimulus_id: &str) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute("DELETE FROM stimuli WHERE id = ?1", params![stimulus_id])?;
        Ok(())
    }

    fn delete_stimuli_for_wave(&self, wave_id: &str) -> StoreResult<u32> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let deleted = conn.execute("DELETE FROM stimuli WHERE wave_id = ?1", params![wave_id])?;
        Ok(deleted as u32)
    }

    // Pending activation methods

    fn list_pending_activations(&self, wave_id: &str) -> StoreResult<Vec<PendingActivation>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, wave_id, stimulus_id, from_sha, to_sha, queued_at
             FROM pending_activations WHERE wave_id = ?1 ORDER BY queued_at",
        )?;

        let rows = stmt.query_map(params![wave_id], |row| {
            Ok(PendingActivation {
                id: row.get(0)?,
                wave_id: row.get(1)?,
                stimulus_id: row.get(2)?,
                from_sha: row.get(3)?,
                to_sha: row.get(4)?,
                queued_at: row.get(5)?,
            })
        })?;

        let mut activations = Vec::new();
        for activation in rows {
            activations.push(activation?);
        }
        Ok(activations)
    }

    fn create_pending_activation(&self, activation: &PendingActivation) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO pending_activations (id, wave_id, stimulus_id, from_sha, to_sha, queued_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                activation.id,
                activation.wave_id,
                activation.stimulus_id,
                activation.from_sha,
                activation.to_sha,
                activation.queued_at,
            ],
        )?;

        // Update wave's pending_activations count
        conn.execute(
            "UPDATE waves SET pending_activations = (
                SELECT COUNT(*) FROM pending_activations WHERE wave_id = ?1
             ) WHERE id = ?1",
            params![activation.wave_id],
        )?;

        Ok(())
    }

    fn update_pending_activation(&self, activation: &PendingActivation) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let updated = conn.execute(
            "UPDATE pending_activations SET from_sha = ?1, to_sha = ?2 WHERE id = ?3",
            params![activation.from_sha, activation.to_sha, activation.id],
        )?;
        if updated == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    fn delete_pending_activations(&self, wave_id: &str) -> StoreResult<u32> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let deleted = conn.execute(
            "DELETE FROM pending_activations WHERE wave_id = ?1",
            params![wave_id],
        )?;

        // Update wave's pending_activations count to 0
        conn.execute(
            "UPDATE waves SET pending_activations = 0 WHERE id = ?1",
            params![wave_id],
        )?;

        Ok(deleted as u32)
    }

    fn get_pending_for_stimulus(
        &self,
        wave_id: &str,
        stimulus_id: &str,
    ) -> StoreResult<Option<PendingActivation>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, wave_id, stimulus_id, from_sha, to_sha, queued_at
             FROM pending_activations WHERE wave_id = ?1 AND stimulus_id = ?2",
        )?;

        let activation = stmt
            .query_row(params![wave_id, stimulus_id], |row| {
                Ok(PendingActivation {
                    id: row.get(0)?,
                    wave_id: row.get(1)?,
                    stimulus_id: row.get(2)?,
                    from_sha: row.get(3)?,
                    to_sha: row.get(4)?,
                    queued_at: row.get(5)?,
                })
            })
            .optional()?;

        Ok(activation)
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
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = String::from(
            "
            SELECT id, step, repo, worktree, flow_run_id, wave_id, status,
                   started_at, ended_at, pid, model, run_mode
            FROM step_runs
            ",
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        let mut where_clauses = Vec::new();

        if let Some(worktree) = worktree {
            where_clauses.push("worktree = ?".to_string());
            params_vec.push(Box::new(worktree.to_string()));
        }
        if let Some(repo) = repo {
            where_clauses.push("repo = ?".to_string());
            params_vec.push(Box::new(repo.to_string()));
        }
        if !where_clauses.is_empty() {
            query.push_str(" WHERE ");
            query.push_str(&where_clauses.join(" AND "));
        }
        query.push_str(" ORDER BY started_at DESC");
        if let Some(limit) = limit {
            query.push_str(" LIMIT ?");
            params_vec.push(Box::new(limit as i64));
        }

        let mut stmt = conn.prepare(&query)?;
        let params_iter = params_vec.iter().map(|value| value.as_ref() as &dyn ToSql);
        let rows = stmt.query_map(rusqlite::params_from_iter(params_iter), |row| {
            let started_at = unix_to_timestamp(row.get::<_, i64>(7)?);
            let ended_at: Option<i64> = row.get(8)?;
            let pid: Option<i64> = row.get(9)?;

            Ok(StepRun {
                id: row.get(0)?,
                step: row.get(1)?,
                repo: row.get(2)?,
                worktree: row.get(3)?,
                flow_run_id: row.get(4)?,
                wave_id: row.get(5)?,
                status: row.get::<_, i64>(6)? as i32,
                started_at: Some(started_at),
                ended_at: ended_at.map(unix_to_timestamp),
                pid: pid.map(|value| value as u32),
                model: row.get(10)?,
                run_mode: row.get(11)?,
            })
        })?;

        let mut runs = Vec::new();
        for run in rows {
            runs.push(run?);
        }
        Ok(runs)
    }

    fn list_fork_runs(&self, wave_id: &str, step_index: u32) -> StoreResult<Vec<ForkRun>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, wave_id, step_index, branch_index, status, worktree
             FROM fork_runs WHERE wave_id = ?1 AND step_index = ?2
             ORDER BY branch_index ASC",
        )?;

        let rows = stmt.query_map(params![wave_id, step_index as i64], |row| {
            let status_value: i64 = row.get(4)?;
            let status = ForkRunStatus::from_i64(status_value).ok_or_else(|| {
                rusqlite::Error::FromSqlConversionFailure(
                    4,
                    Type::Integer,
                    Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "invalid fork run status",
                    )),
                )
            })?;

            Ok(ForkRun {
                id: row.get(0)?,
                wave_id: row.get(1)?,
                step_index: row.get::<_, i64>(2)? as u32,
                branch_index: row.get::<_, i64>(3)? as u32,
                status,
                worktree: row.get(5)?,
            })
        })?;

        let mut runs = Vec::new();
        for run in rows {
            runs.push(run?);
        }
        Ok(runs)
    }

    fn upsert_fork_run(&self, fork_run: &ForkRun) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "
            INSERT INTO fork_runs (id, wave_id, step_index, branch_index, status, worktree)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(id) DO UPDATE SET
                wave_id = excluded.wave_id,
                step_index = excluded.step_index,
                branch_index = excluded.branch_index,
                status = excluded.status,
                worktree = excluded.worktree
            ",
            params![
                fork_run.id,
                fork_run.wave_id,
                fork_run.step_index as i64,
                fork_run.branch_index as i64,
                fork_run.status as i32,
                fork_run.worktree,
            ],
        )?;
        Ok(())
    }

    fn delete_fork_runs(&self, wave_id: &str, step_index: u32) -> StoreResult<u32> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let deleted = conn.execute(
            "DELETE FROM fork_runs WHERE wave_id = ?1 AND step_index = ?2",
            params![wave_id, step_index as i64],
        )?;
        Ok(deleted as u32)
    }

    fn get_step_run(&self, step_run_id: &str) -> StoreResult<Option<StepRun>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "
            SELECT id, step, repo, worktree, flow_run_id, wave_id, status,
                   started_at, ended_at, pid, model, run_mode
            FROM step_runs
            WHERE id = ?1
            ",
        )?;

        let run = stmt
            .query_row(params![step_run_id], |row| {
                let started_at = unix_to_timestamp(row.get::<_, i64>(7)?);
                let ended_at: Option<i64> = row.get(8)?;
                let pid: Option<i64> = row.get(9)?;

                Ok(StepRun {
                    id: row.get(0)?,
                    step: row.get(1)?,
                    repo: row.get(2)?,
                    worktree: row.get(3)?,
                    flow_run_id: row.get(4)?,
                    wave_id: row.get(5)?,
                    status: row.get::<_, i64>(6)? as i32,
                    started_at: Some(started_at),
                    ended_at: ended_at.map(unix_to_timestamp),
                    pid: pid.map(|value| value as u32),
                    model: row.get(10)?,
                    run_mode: row.get(11)?,
                })
            })
            .optional()?;

        Ok(run)
    }

    fn get_waiting_step_run(&self, wave_id: &str) -> StoreResult<Option<StepRun>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "
            SELECT id, step, repo, worktree, flow_run_id, wave_id, status,
                   started_at, ended_at, pid, model, run_mode
            FROM step_runs
            WHERE wave_id = ?1 AND status = ?2
            ORDER BY started_at DESC
            LIMIT 1
            ",
        )?;

        let run = stmt
            .query_row(
                params![wave_id, StepRunStatus::StepWaiting as i32],
                |row| {
                    let started_at = unix_to_timestamp(row.get::<_, i64>(7)?);
                    let ended_at: Option<i64> = row.get(8)?;
                    let pid: Option<i64> = row.get(9)?;

                    Ok(StepRun {
                        id: row.get(0)?,
                        step: row.get(1)?,
                        repo: row.get(2)?,
                        worktree: row.get(3)?,
                        flow_run_id: row.get(4)?,
                        wave_id: row.get(5)?,
                        status: row.get::<_, i64>(6)? as i32,
                        started_at: Some(started_at),
                        ended_at: ended_at.map(unix_to_timestamp),
                        pid: pid.map(|value| value as u32),
                        model: row.get(10)?,
                        run_mode: row.get(11)?,
                    })
                },
            )
            .optional()?;

        Ok(run)
    }

    fn start_step_run(&self, step_run: &StepRun) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let started_at = step_run
            .started_at
            .as_ref()
            .map(timestamp_to_unix)
            .unwrap_or_else(now_unix);
        conn.execute(
            "
            INSERT INTO step_runs (
                id, step, repo, worktree, flow_run_id, wave_id, status, started_at,
                ended_at, pid, model, run_mode
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            ",
            params![
                step_run.id,
                step_run.step,
                step_run.repo,
                step_run.worktree,
                step_run.flow_run_id,
                step_run.wave_id,
                step_run.status,
                started_at,
                step_run.ended_at.as_ref().map(timestamp_to_unix),
                step_run.pid.map(|value| value as i64),
                step_run.model,
                step_run.run_mode,
            ],
        )?;
        Ok(())
    }

    fn update_step_run_status(
        &self,
        step_run_id: &str,
        status: i32,
        pid: Option<u32>,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let updated = conn.execute(
            "UPDATE step_runs SET status = ?1, pid = COALESCE(?2, pid) WHERE id = ?3",
            params![status, pid.map(|value| value as i64), step_run_id],
        )?;
        if updated == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    fn end_step_run(&self, step_run_id: &str, status: i32, ended_at: i64) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let updated = conn.execute(
            "UPDATE step_runs SET status = ?1, ended_at = ?2 WHERE id = ?3",
            params![status, ended_at, step_run_id],
        )?;
        if updated == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    fn get_stuck_step_runs(&self, older_than_secs: u64) -> StoreResult<Vec<StepRun>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let cutoff = now_unix() - older_than_secs as i64;
        let mut stmt = conn.prepare(
            "
            SELECT id, step, repo, worktree, flow_run_id, wave_id, status,
                   started_at, ended_at, pid, model, run_mode
            FROM step_runs
            WHERE ended_at IS NULL AND started_at <= ?1
            ORDER BY started_at ASC
            ",
        )?;

        let rows = stmt.query_map(params![cutoff], |row| {
            let started_at = unix_to_timestamp(row.get::<_, i64>(7)?);
            let ended_at: Option<i64> = row.get(8)?;
            let pid: Option<i64> = row.get(9)?;

            Ok(StepRun {
                id: row.get(0)?,
                step: row.get(1)?,
                repo: row.get(2)?,
                worktree: row.get(3)?,
                flow_run_id: row.get(4)?,
                wave_id: row.get(5)?,
                status: row.get::<_, i64>(6)? as i32,
                started_at: Some(started_at),
                ended_at: ended_at.map(unix_to_timestamp),
                pid: pid.map(|value| value as u32),
                model: row.get(10)?,
                run_mode: row.get(11)?,
            })
        })?;

        let mut runs = Vec::new();
        for run in rows {
            runs.push(run?);
        }
        Ok(runs)
    }
}

fn unix_to_timestamp(seconds: i64) -> Timestamp {
    Timestamp { seconds, nanos: 0 }
}

fn timestamp_to_unix(ts: &Timestamp) -> i64 {
    ts.seconds
}

fn now_unix() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp()
}

fn map_wave_row(row: &Row<'_>) -> Result<Wave, rusqlite::Error> {
    let direction_json: String = row.get(4)?;
    let area_json: String = row.get(5)?;
    let direction = parse_json_vec(&direction_json)?;
    let area = parse_json_vec(&area_json)?;

    // Note: stimulus_kind (6), stimulus_cron (7), last_main_sha (17) columns
    // still exist for backwards compat but are ignored - data is in stimuli table

    let created_at = unix_to_timestamp(row.get::<_, i64>(16)?);
    let pid: Option<i64> = row.get(15)?;

    Ok(Wave {
        id: row.get(0)?,
        name: row.get(1)?,
        repo: row.get(2)?,
        flow: row.get(3)?,
        direction,
        area,
        paused: row.get::<_, i64>(8)? != 0,
        status: row.get::<_, i64>(9)? as i32,
        iteration: row.get::<_, i64>(10)? as u32,
        worktree: row.get(11)?,
        branch: row.get(12)?,
        pr_limit: row.get::<_, i64>(13)? as u32,
        merge_mode: row.get::<_, i64>(14)? as i32,
        pid: pid.map(|value| value as u32),
        created_at: Some(created_at),
        consecutive_failures: row.get::<_, i64>(18)? as u32,
        pending_activations: row.get::<_, i64>(19)? as u32,
        step_index: row.get::<_, i64>(20)? as u32,
    })
}

fn map_stimulus_row(row: &Row<'_>) -> Result<Stimulus, rusqlite::Error> {
    let created_at = unix_to_timestamp(row.get::<_, i64>(7)?);

    Ok(Stimulus {
        id: row.get(0)?,
        wave_id: row.get(1)?,
        kind: row.get::<_, i64>(2)? as i32,
        cron: row.get(3)?,
        last_main_sha: row.get(4)?,
        last_triggered_at: row.get(5)?,
        enabled: row.get::<_, i64>(6)? != 0,
        created_at: Some(created_at),
    })
}

fn parse_json_vec(value: &str) -> Result<Vec<String>, rusqlite::Error> {
    serde_json::from_str::<Vec<String>>(value)
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(0, Type::Text, Box::new(err)))
}

fn ensure_column(
    conn: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> StoreResult<()> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    let mut columns = HashSet::new();
    for name in rows {
        columns.insert(name?);
    }
    if !columns.contains(column) {
        conn.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
            [],
        )?;
    }
    Ok(())
}
