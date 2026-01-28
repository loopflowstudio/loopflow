use std::collections::HashSet;
use std::path::Path;
use std::sync::Mutex;

use prost_types::Timestamp;
use rusqlite::types::Type;
use rusqlite::{params, Connection, OptionalExtension, Row, ToSql};
use time::OffsetDateTime;

use crate::proto::control::{StepRun, Stimulus, StimulusKind, Wave};
use crate::store::{RunStore, StoreError, StoreResult};

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
            ",
        )?;
        ensure_column(&conn, "waves", "step_index", "INTEGER NOT NULL DEFAULT 0")?;
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

        let stimulus = wave.stimulus.clone().unwrap_or_else(default_stimulus);
        let created_at = wave
            .created_at
            .as_ref()
            .map(timestamp_to_unix)
            .unwrap_or_else(now_unix);

        conn.execute(
            "
            INSERT INTO waves (
                id, name, repo, flow, direction, area, stimulus_kind, stimulus_cron,
                paused, status, iteration, worktree, branch, pr_limit, merge_mode, pid,
                created_at, last_main_sha, consecutive_failures, pending_activations,
                step_index
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                repo = excluded.repo,
                flow = excluded.flow,
                direction = excluded.direction,
                area = excluded.area,
                stimulus_kind = excluded.stimulus_kind,
                stimulus_cron = excluded.stimulus_cron,
                paused = excluded.paused,
                status = excluded.status,
                iteration = excluded.iteration,
                worktree = excluded.worktree,
                branch = excluded.branch,
                pr_limit = excluded.pr_limit,
                merge_mode = excluded.merge_mode,
                pid = excluded.pid,
                created_at = excluded.created_at,
                last_main_sha = excluded.last_main_sha,
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
                stimulus.kind,
                stimulus.cron,
                if wave.paused { 1 } else { 0 },
                wave.status,
                wave.iteration,
                wave.worktree,
                wave.branch,
                wave.pr_limit,
                wave.merge_mode,
                wave.pid.map(|value| value as i64),
                created_at,
                wave.last_main_sha,
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

    fn list_waves_by_stimulus(&self, kind: i32) -> StoreResult<Vec<Wave>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "
            SELECT id, name, repo, flow, direction, area, stimulus_kind, stimulus_cron,
                   paused, status, iteration, worktree, branch, pr_limit, merge_mode, pid,
                   created_at, last_main_sha, consecutive_failures, pending_activations,
                   step_index
            FROM waves
            WHERE stimulus_kind = ?1
            ORDER BY created_at DESC
            ",
        )?;

        let rows = stmt.query_map(params![kind], map_wave_row)?;
        let mut waves = Vec::new();
        for wave in rows {
            waves.push(wave?);
        }
        Ok(waves)
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

        let wave = stmt
            .query_row(params![wave_id], |row| map_wave_row(row))
            .optional()?;

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
        conn.execute("DELETE FROM waves WHERE id = ?1", params![wave_id])?;
        Ok(())
    }

    fn increment_pending_activations(&self, wave_id: &str) -> StoreResult<u32> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let updated = conn.execute(
            "UPDATE waves SET pending_activations = pending_activations + 1 WHERE id = ?1",
            params![wave_id],
        )?;
        if updated == 0 {
            return Err(StoreError::NotFound);
        }
        let count: i64 = conn.query_row(
            "SELECT pending_activations FROM waves WHERE id = ?1",
            params![wave_id],
            |row| row.get(0),
        )?;
        Ok(count as u32)
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

fn default_stimulus() -> Stimulus {
    Stimulus {
        kind: StimulusKind::StimulusOnce as i32,
        cron: String::new(),
    }
}

fn map_wave_row(row: &Row<'_>) -> Result<Wave, rusqlite::Error> {
    let direction_json: String = row.get(4)?;
    let area_json: String = row.get(5)?;
    let direction = parse_json_vec(&direction_json)?;
    let area = parse_json_vec(&area_json)?;

    let stimulus = Stimulus {
        kind: row.get::<_, i64>(6)? as i32,
        cron: row.get(7)?,
    };

    let created_at = unix_to_timestamp(row.get::<_, i64>(16)?);
    let pid: Option<i64> = row.get(15)?;

    Ok(Wave {
        id: row.get(0)?,
        name: row.get(1)?,
        repo: row.get(2)?,
        flow: row.get(3)?,
        direction,
        area,
        stimulus: Some(stimulus),
        paused: row.get::<_, i64>(8)? != 0,
        status: row.get::<_, i64>(9)? as i32,
        iteration: row.get::<_, i64>(10)? as u32,
        worktree: row.get(11)?,
        branch: row.get(12)?,
        pr_limit: row.get::<_, i64>(13)? as u32,
        merge_mode: row.get::<_, i64>(14)? as i32,
        pid: pid.map(|value| value as u32),
        created_at: Some(created_at),
        last_main_sha: row.get(17)?,
        consecutive_failures: row.get::<_, i64>(18)? as u32,
        pending_activations: row.get::<_, i64>(19)? as u32,
        step_index: row.get::<_, i64>(20)? as u32,
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
