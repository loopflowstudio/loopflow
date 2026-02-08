use std::collections::HashSet;
use std::path::Path;
use std::sync::Mutex;

use rusqlite::types::Type;
use rusqlite::{params, Connection, OptionalExtension, Row, ToSql};
use time::OffsetDateTime;

use crate::lfd::id::LfdId;
use crate::lfd::store::{
    schema::SCHEMA_VERSION, ForkRun, ForkRunStatus, RunStore, StoreError, StoreResult,
};
use crate::lfd::types::{
    Agent, AgentStatus, PendingActivation, PullRequest, Stimulus, StimulusKind, Wave, WaveRun,
    WaveRunSnapshot, WaveRunStatus, WaveStatus,
};

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
            INSERT OR IGNORE INTO meta (key, value) VALUES ('schema_version', '');

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
                step_index INTEGER NOT NULL DEFAULT 0,
                UNIQUE(name, repo)
            );
            CREATE INDEX IF NOT EXISTS idx_waves_name ON waves(name);

            CREATE TABLE IF NOT EXISTS agents (
                id TEXT PRIMARY KEY,
                step TEXT NOT NULL,
                repo TEXT NOT NULL,
                worktree TEXT NOT NULL,
                wave_run_id TEXT,
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

            CREATE TABLE IF NOT EXISTS wave_runs (
                id TEXT PRIMARY KEY,
                wave_id TEXT NOT NULL,
                iteration INTEGER NOT NULL,
                step_index INTEGER NOT NULL DEFAULT 0,
                status INTEGER NOT NULL,
                worktree TEXT NOT NULL,
                branch TEXT NOT NULL,
                started_at INTEGER NOT NULL,
                ended_at INTEGER,
                error TEXT,
                snapshot_repo TEXT NOT NULL DEFAULT '',
                snapshot_flow TEXT NOT NULL DEFAULT '',
                snapshot_direction TEXT NOT NULL DEFAULT '[]',
                snapshot_area TEXT NOT NULL DEFAULT '[]',
                snapshot_pr TEXT,
                flow_parents TEXT NOT NULL DEFAULT '[]',
                FOREIGN KEY (wave_id) REFERENCES waves(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_wave_runs_wave_id ON wave_runs(wave_id, started_at);

            CREATE TABLE IF NOT EXISTS fork_runs (
                id TEXT PRIMARY KEY,
                wave_run_id TEXT,
                step_index INTEGER NOT NULL,
                branch_index INTEGER NOT NULL,
                status INTEGER NOT NULL,
                worktree TEXT NOT NULL,
                FOREIGN KEY (wave_run_id) REFERENCES wave_runs(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_fork_runs_wave_run_id ON fork_runs(wave_run_id, step_index);
            ",
        )?;
        ensure_column(&conn, "waves", "step_index", "INTEGER NOT NULL DEFAULT 0")?;
        ensure_column(
            &conn,
            "wave_runs",
            "flow_parents",
            "TEXT NOT NULL DEFAULT '[]'",
        )?;
        ensure_column(
            &conn,
            "wave_runs",
            "snapshot_repo",
            "TEXT NOT NULL DEFAULT ''",
        )?;
        ensure_column(
            &conn,
            "wave_runs",
            "snapshot_flow",
            "TEXT NOT NULL DEFAULT ''",
        )?;
        ensure_column(
            &conn,
            "wave_runs",
            "snapshot_direction",
            "TEXT NOT NULL DEFAULT '[]'",
        )?;
        ensure_column(
            &conn,
            "wave_runs",
            "snapshot_area",
            "TEXT NOT NULL DEFAULT '[]'",
        )?;
        ensure_column(&conn, "wave_runs", "snapshot_pr", "TEXT")?;
        ensure_column(&conn, "fork_runs", "wave_run_id", "TEXT")?;

        conn.execute(
            "
            UPDATE wave_runs
            SET snapshot_repo = (SELECT repo FROM waves WHERE waves.id = wave_runs.wave_id),
                snapshot_flow = (SELECT flow FROM waves WHERE waves.id = wave_runs.wave_id),
                snapshot_direction = (SELECT direction FROM waves WHERE waves.id = wave_runs.wave_id),
                snapshot_area = (SELECT area FROM waves WHERE waves.id = wave_runs.wave_id)
            WHERE snapshot_repo = ''
            ",
            [],
        )?;

        // Migrate existing stimulus data from waves to stimuli table
        Self::migrate_stimuli_from_waves(&conn)?;
        conn.execute(
            "UPDATE meta SET value = ?1 WHERE key = 'schema_version'",
            params![SCHEMA_VERSION],
        )?;

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
                SELECT id, name, repo, flow, direction, area, paused, status, iteration, created_at
                FROM waves
                WHERE repo = ?1
                ORDER BY created_at DESC
                ",
            )?
        } else {
            conn.prepare(
                "
                SELECT id, name, repo, flow, direction, area, paused, status, iteration, created_at
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
            .map(|dt| dt.unix_timestamp())
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
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, '', ?7, ?8, ?9, '', '', 0, 0, NULL, ?10, NULL, 0, 0, 0)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                repo = excluded.repo,
                flow = excluded.flow,
                direction = excluded.direction,
                area = excluded.area,
                paused = excluded.paused,
                status = excluded.status,
                iteration = excluded.iteration,
                created_at = excluded.created_at
            ",
            params![
                wave.id,
                wave.name,
                wave.repo,
                wave.flow,
                direction_json,
                area_json,
                if wave.status == WaveStatus::Paused { 1 } else { 0 },
                wave.status.as_i32(),
                wave.iteration as i64,
                created_at,
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

    fn schema_version(&self) -> StoreResult<String> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let version: Option<String> = conn
            .query_row(
                "SELECT value FROM meta WHERE key = 'schema_version'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        Ok(version.unwrap_or_else(|| SCHEMA_VERSION.to_string()))
    }

    fn list_waves(&self, repo: Option<&str>) -> StoreResult<Vec<Wave>> {
        self.read_waves(repo)
    }

    fn get_wave(&self, wave_id: &LfdId) -> StoreResult<Option<Wave>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "
            SELECT id, name, repo, flow, direction, area, paused, status, iteration, created_at
            FROM waves
            WHERE id = ?1
            ",
        )?;

        let wave = stmt.query_row(params![wave_id], map_wave_row).optional()?;

        Ok(wave)
    }

    fn get_wave_by_name(&self, name: &str) -> StoreResult<Option<Wave>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "
            SELECT id, name, repo, flow, direction, area, paused, status, iteration, created_at
            FROM waves
            WHERE name = ?1
            ",
        )?;
        let wave = stmt.query_row(params![name], map_wave_row).optional()?;
        Ok(wave)
    }

    fn create_wave(&self, wave: &Wave) -> StoreResult<()> {
        self.upsert_wave(wave)
    }

    fn update_wave(&self, wave: &Wave) -> StoreResult<()> {
        self.upsert_wave(wave)
    }

    fn delete_wave(&self, wave_id: &LfdId) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        // Stimuli and pending_activations cascade delete via foreign key
        conn.execute("DELETE FROM waves WHERE id = ?1", params![wave_id])?;
        Ok(())
    }

    fn list_wave_runs(
        &self,
        wave_id: Option<&LfdId>,
        limit: Option<u32>,
    ) -> StoreResult<Vec<WaveRun>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = String::from(
            "
            SELECT id, wave_id, iteration, step_index, status, worktree, branch,
                   started_at, ended_at, error, snapshot_repo, snapshot_flow, snapshot_direction,
                   snapshot_area, snapshot_pr, flow_parents
            FROM wave_runs
            ",
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        if let Some(wave_id) = wave_id {
            query.push_str(" WHERE wave_id = ?");
            params_vec.push(Box::new(wave_id.clone()));
        }
        query.push_str(" ORDER BY started_at DESC");
        if let Some(limit) = limit {
            query.push_str(" LIMIT ?");
            params_vec.push(Box::new(limit as i64));
        }

        let mut stmt = conn.prepare(&query)?;
        let params_iter = params_vec.iter().map(|value| value.as_ref() as &dyn ToSql);
        let rows = stmt.query_map(rusqlite::params_from_iter(params_iter), map_wave_run_row)?;

        let mut runs = Vec::new();
        for run in rows {
            runs.push(run?);
        }
        Ok(runs)
    }

    fn get_wave_run(&self, wave_run_id: &LfdId) -> StoreResult<Option<WaveRun>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "
            SELECT id, wave_id, iteration, step_index, status, worktree, branch,
                   started_at, ended_at, error, snapshot_repo, snapshot_flow, snapshot_direction,
                   snapshot_area, snapshot_pr, flow_parents
            FROM wave_runs
            WHERE id = ?1
            ",
        )?;
        let run = stmt
            .query_row(params![wave_run_id], map_wave_run_row)
            .optional()?;
        Ok(run)
    }

    fn get_active_wave_run(&self, wave_id: &LfdId) -> StoreResult<Option<WaveRun>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "
            SELECT id, wave_id, iteration, step_index, status, worktree, branch,
                   started_at, ended_at, error, snapshot_repo, snapshot_flow, snapshot_direction,
                   snapshot_area, snapshot_pr, flow_parents
            FROM wave_runs
            WHERE wave_id = ?1 AND status IN (?2, ?3, ?4, ?5)
            ORDER BY started_at DESC
            LIMIT 1
            ",
        )?;
        let run = stmt
            .query_row(
                params![
                    wave_id,
                    WaveRunStatus::Pending.as_i32(),
                    WaveRunStatus::Running.as_i32(),
                    WaveRunStatus::Waiting.as_i32(),
                    WaveRunStatus::Failed.as_i32(),
                ],
                map_wave_run_row,
            )
            .optional()?;
        Ok(run)
    }

    fn create_wave_run(&self, run: &WaveRun) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let started_at = run
            .started_at
            .map(|dt| dt.unix_timestamp())
            .unwrap_or_else(now_unix);
        let flow_parents_json = serde_json::to_string(&run.flow_parents)?;
        conn.execute(
            "
            INSERT INTO wave_runs (
                id, wave_id, iteration, step_index, status, worktree, branch,
                started_at, ended_at, error, snapshot_repo, snapshot_flow, snapshot_direction,
                snapshot_area, snapshot_pr, flow_parents
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
            ",
            params![
                run.id,
                run.wave_id,
                run.iteration as i64,
                run.step_index as i64,
                run.status.as_i32(),
                run.worktree,
                run.branch,
                started_at,
                run.ended_at.map(|dt| dt.unix_timestamp()),
                run.error,
                run.snapshot.repo,
                run.snapshot.flow,
                serde_json::to_string(&run.snapshot.direction)?,
                serde_json::to_string(&run.snapshot.area)?,
                serialize_pr(&run.snapshot.pr)?,
                flow_parents_json,
            ],
        )?;
        Ok(())
    }

    fn update_wave_run(&self, run: &WaveRun) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let flow_parents_json = serde_json::to_string(&run.flow_parents)?;
        let updated = conn.execute(
            "
            UPDATE wave_runs
            SET iteration = ?1,
                step_index = ?2,
                status = ?3,
                worktree = ?4,
                branch = ?5,
                started_at = ?6,
                ended_at = ?7,
                error = ?8,
                snapshot_repo = ?9,
                snapshot_flow = ?10,
                snapshot_direction = ?11,
                snapshot_area = ?12,
                snapshot_pr = ?13,
                flow_parents = ?14
            WHERE id = ?15
            ",
            params![
                run.iteration as i64,
                run.step_index as i64,
                run.status.as_i32(),
                run.worktree,
                run.branch,
                run.started_at.map(|dt| dt.unix_timestamp()),
                run.ended_at.map(|dt| dt.unix_timestamp()),
                run.error,
                run.snapshot.repo,
                run.snapshot.flow,
                serde_json::to_string(&run.snapshot.direction)?,
                serde_json::to_string(&run.snapshot.area)?,
                serialize_pr(&run.snapshot.pr)?,
                flow_parents_json,
                run.id,
            ],
        )?;
        if updated == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    // Stimulus methods

    fn list_stimuli(&self, wave_id: Option<&LfdId>) -> StoreResult<Vec<Stimulus>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let (query, params): (&str, Vec<Box<dyn ToSql>>) = if let Some(wave_id) = wave_id {
            (
                "SELECT id, wave_id, kind, cron, last_main_sha, last_triggered_at, enabled, created_at
                 FROM stimuli WHERE wave_id = ?1 ORDER BY created_at",
                vec![Box::new(wave_id.clone())],
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

    fn get_stimulus(&self, stimulus_id: &LfdId) -> StoreResult<Option<Stimulus>> {
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
            .map(|dt| dt.unix_timestamp())
            .unwrap_or_else(now_unix);

        conn.execute(
            "INSERT INTO stimuli (id, wave_id, kind, cron, last_main_sha, last_triggered_at, enabled, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                stimulus.id,
                stimulus.wave_id,
                stimulus.kind.as_i32(),
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
                stimulus.kind.as_i32(),
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

    fn delete_stimulus(&self, stimulus_id: &LfdId) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute("DELETE FROM stimuli WHERE id = ?1", params![stimulus_id])?;
        Ok(())
    }

    fn delete_stimuli_for_wave(&self, wave_id: &LfdId) -> StoreResult<u32> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let deleted = conn.execute("DELETE FROM stimuli WHERE wave_id = ?1", params![wave_id])?;
        Ok(deleted as u32)
    }

    // Pending activation methods

    fn list_pending_activations(&self, wave_id: &LfdId) -> StoreResult<Vec<PendingActivation>> {
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

    fn delete_pending_activations(&self, wave_id: &LfdId) -> StoreResult<u32> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let deleted = conn.execute(
            "DELETE FROM pending_activations WHERE wave_id = ?1",
            params![wave_id],
        )?;

        Ok(deleted as u32)
    }

    fn get_pending_for_stimulus(
        &self,
        wave_id: &LfdId,
        stimulus_id: &LfdId,
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

    fn list_agents(&self) -> StoreResult<Vec<Agent>> {
        self.list_agent_history(None, None, None)
    }

    fn list_agent_history(
        &self,
        worktree: Option<&str>,
        repo: Option<&str>,
        limit: Option<u32>,
    ) -> StoreResult<Vec<Agent>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = String::from(
            "
            SELECT id, step, repo, worktree, wave_run_id, status,
                   started_at, ended_at, pid, model, run_mode
            FROM agents
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
        let rows = stmt.query_map(rusqlite::params_from_iter(params_iter), map_agent_row)?;

        let mut runs = Vec::new();
        for run in rows {
            runs.push(run?);
        }
        Ok(runs)
    }

    fn list_fork_runs(&self, wave_run_id: &LfdId, step_index: u32) -> StoreResult<Vec<ForkRun>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, wave_run_id, step_index, branch_index, status, worktree
             FROM fork_runs WHERE wave_run_id = ?1 AND step_index = ?2
             ORDER BY branch_index ASC",
        )?;

        let rows = stmt.query_map(params![wave_run_id, step_index as i64], |row| {
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
                id: LfdId::from_raw(row.get::<_, String>(0)?),
                wave_run_id: LfdId::from_raw(row.get::<_, String>(1)?),
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
            INSERT INTO fork_runs (id, wave_run_id, step_index, branch_index, status, worktree)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(id) DO UPDATE SET
                wave_run_id = excluded.wave_run_id,
                step_index = excluded.step_index,
                branch_index = excluded.branch_index,
                status = excluded.status,
                worktree = excluded.worktree
            ",
            params![
                fork_run.id,
                fork_run.wave_run_id,
                fork_run.step_index as i64,
                fork_run.branch_index as i64,
                fork_run.status as i32,
                fork_run.worktree,
            ],
        )?;
        Ok(())
    }

    fn delete_fork_runs(&self, wave_run_id: &LfdId, step_index: u32) -> StoreResult<u32> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let deleted = conn.execute(
            "DELETE FROM fork_runs WHERE wave_run_id = ?1 AND step_index = ?2",
            params![wave_run_id, step_index as i64],
        )?;
        Ok(deleted as u32)
    }

    fn get_agent(&self, agent_id: &LfdId) -> StoreResult<Option<Agent>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "
            SELECT id, step, repo, worktree, wave_run_id, status,
                   started_at, ended_at, pid, model, run_mode
            FROM agents
            WHERE id = ?1
            ",
        )?;

        let run = stmt
            .query_row(params![agent_id], map_agent_row)
            .optional()?;

        Ok(run)
    }

    fn get_waiting_agent_for_wave(&self, wave_id: &LfdId) -> StoreResult<Option<Agent>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "
            SELECT a.id, a.step, a.repo, a.worktree, a.wave_run_id, a.status,
                   a.started_at, a.ended_at, a.pid, a.model, a.run_mode
            FROM agents a
            JOIN wave_runs r ON a.wave_run_id = r.id
            WHERE r.wave_id = ?1 AND a.status = ?2
            ORDER BY a.started_at DESC
            LIMIT 1
            ",
        )?;

        let run = stmt
            .query_row(
                params![wave_id, AgentStatus::Waiting.as_i32()],
                map_agent_row,
            )
            .optional()?;

        Ok(run)
    }

    fn start_agent(&self, agent: &Agent) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let started_at = agent
            .started_at
            .map(|dt| dt.unix_timestamp())
            .unwrap_or_else(now_unix);
        conn.execute(
            "
            INSERT INTO agents (
                id, step, repo, worktree, wave_run_id, status, started_at,
                ended_at, pid, model, run_mode
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ",
            params![
                agent.id,
                agent.step,
                agent.repo,
                agent.worktree,
                agent.wave_run_id,
                agent.status.as_i32(),
                started_at,
                agent.ended_at.map(|dt| dt.unix_timestamp()),
                agent.pid.map(|value| value as i64),
                agent.model,
                agent.run_mode,
            ],
        )?;
        Ok(())
    }

    fn update_agent_status(
        &self,
        agent_id: &LfdId,
        status: i32,
        pid: Option<u32>,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let updated = conn.execute(
            "UPDATE agents SET status = ?1, pid = COALESCE(?2, pid) WHERE id = ?3",
            params![status, pid.map(|value| value as i64), agent_id],
        )?;
        if updated == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    fn end_agent(&self, agent_id: &LfdId, status: i32, ended_at: i64) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let updated = conn.execute(
            "UPDATE agents SET status = ?1, ended_at = ?2 WHERE id = ?3",
            params![status, ended_at, agent_id],
        )?;
        if updated == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    fn get_active_agents_for_wave(&self, wave_id: &LfdId) -> StoreResult<Vec<Agent>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "
            SELECT a.id, a.step, a.repo, a.worktree, a.wave_run_id, a.status,
                   a.started_at, a.ended_at, a.pid, a.model, a.run_mode
            FROM agents a
            JOIN wave_runs r ON a.wave_run_id = r.id
            WHERE r.wave_id = ?1 AND a.ended_at IS NULL
            ORDER BY a.started_at DESC
            ",
        )?;
        let rows = stmt.query_map(params![wave_id], map_agent_row)?;
        let mut agents = Vec::new();
        for row in rows {
            agents.push(row?);
        }
        Ok(agents)
    }

    fn end_active_agent_for_wave(
        &self,
        wave_id: &LfdId,
        status: i32,
        ended_at: i64,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let updated = conn.execute(
            "UPDATE agents SET status = ?1, ended_at = ?2
             WHERE wave_run_id IN (SELECT id FROM wave_runs WHERE wave_id = ?3)
             AND ended_at IS NULL",
            params![status, ended_at, wave_id.as_str()],
        )?;
        if updated == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    fn get_stuck_agents(&self, older_than_secs: u64) -> StoreResult<Vec<Agent>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let cutoff = now_unix() - older_than_secs as i64;
        let mut stmt = conn.prepare(
            "
            SELECT id, step, repo, worktree, wave_run_id, status,
                   started_at, ended_at, pid, model, run_mode
            FROM agents
            WHERE ended_at IS NULL AND started_at <= ?1
            ORDER BY started_at ASC
            ",
        )?;

        let rows = stmt.query_map(params![cutoff], map_agent_row)?;

        let mut runs = Vec::new();
        for run in rows {
            runs.push(run?);
        }
        Ok(runs)
    }
}

fn unix_to_datetime(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(seconds).unwrap_or(OffsetDateTime::UNIX_EPOCH)
}

fn now_unix() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp()
}

fn map_wave_row(row: &Row<'_>) -> Result<Wave, rusqlite::Error> {
    let direction_json: String = row.get(4)?;
    let area_json: String = row.get(5)?;
    let direction = parse_json_vec(&direction_json)?;
    let area = parse_json_vec(&area_json)?;
    let paused = row.get::<_, i64>(6)? != 0;
    let status_value: i64 = row.get(7)?;
    let iteration: i64 = row.get(8)?;
    let created_at = unix_to_datetime(row.get::<_, i64>(9)?);
    let mut status = WaveStatus::from_i32(status_value as i32);
    if paused {
        status = WaveStatus::Paused;
    }

    Ok(Wave {
        id: row.get(0)?,
        name: row.get(1)?,
        repo: row.get(2)?,
        flow: row.get(3)?,
        direction,
        area,
        status,
        iteration: iteration as u32,
        created_at: Some(created_at),
    })
}

fn map_wave_run_row(row: &Row<'_>) -> Result<WaveRun, rusqlite::Error> {
    let started_at = unix_to_datetime(row.get::<_, i64>(7)?);
    let ended_at: Option<i64> = row.get(8)?;
    let snapshot_direction_json: String = row.get(12)?;
    let snapshot_area_json: String = row.get(13)?;
    let snapshot_pr_json: Option<String> = row.get(14)?;
    let flow_parents_json: String = row.get(15)?;
    let flow_parents = parse_json_vec(&flow_parents_json)?;
    let snapshot = WaveRunSnapshot {
        repo: row.get(10)?,
        flow: row.get(11)?,
        direction: parse_json_vec(&snapshot_direction_json)?,
        area: parse_json_vec(&snapshot_area_json)?,
        pr: parse_pr(snapshot_pr_json)?,
    };

    Ok(WaveRun {
        id: row.get(0)?,
        wave_id: row.get(1)?,
        snapshot,
        iteration: row.get::<_, i64>(2)? as u32,
        step_index: row.get::<_, i64>(3)? as u32,
        status: WaveRunStatus::from_i32(row.get::<_, i64>(4)? as i32),
        worktree: row.get(5)?,
        branch: row.get(6)?,
        started_at: Some(started_at),
        ended_at: ended_at.map(unix_to_datetime),
        error: row.get(9)?,
        flow_parents,
    })
}

fn map_stimulus_row(row: &Row<'_>) -> Result<Stimulus, rusqlite::Error> {
    let created_at = unix_to_datetime(row.get::<_, i64>(7)?);

    Ok(Stimulus {
        id: row.get(0)?,
        wave_id: row.get(1)?,
        kind: StimulusKind::from_i32(row.get::<_, i64>(2)? as i32),
        cron: row.get(3)?,
        last_main_sha: row.get(4)?,
        last_triggered_at: row.get(5)?,
        enabled: row.get::<_, i64>(6)? != 0,
        created_at: Some(created_at),
    })
}

fn map_agent_row(row: &Row<'_>) -> Result<Agent, rusqlite::Error> {
    let started_at = unix_to_datetime(row.get::<_, i64>(6)?);
    let ended_at: Option<i64> = row.get(7)?;
    let pid: Option<i64> = row.get(8)?;
    let wave_run_id: Option<String> = row.get(4)?;

    Ok(Agent {
        id: row.get(0)?,
        step: row.get(1)?,
        repo: row.get(2)?,
        worktree: row.get(3)?,
        wave_run_id: wave_run_id.map(LfdId::from_raw),
        status: AgentStatus::from_i32(row.get::<_, i64>(5)? as i32),
        started_at: Some(started_at),
        ended_at: ended_at.map(unix_to_datetime),
        pid: pid.map(|value| value as u32),
        model: row.get(9)?,
        run_mode: row.get(10)?,
    })
}

fn parse_json_vec(value: &str) -> Result<Vec<String>, rusqlite::Error> {
    serde_json::from_str::<Vec<String>>(value)
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(0, Type::Text, Box::new(err)))
}

fn serialize_pr(value: &Option<PullRequest>) -> Result<Option<String>, StoreError> {
    Ok(match value {
        Some(pr) => Some(serde_json::to_string(pr)?),
        None => None,
    })
}

fn parse_pr(value: Option<String>) -> Result<Option<PullRequest>, rusqlite::Error> {
    if let Some(raw) = value {
        if raw.trim().is_empty() {
            return Ok(None);
        }
        match serde_json::from_str::<PullRequest>(&raw) {
            Ok(parsed) => Ok(Some(parsed)),
            Err(_) => Ok(None),
        }
    } else {
        Ok(None)
    }
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
