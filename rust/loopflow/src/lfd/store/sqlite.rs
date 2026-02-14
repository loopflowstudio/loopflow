use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension, ToSql};

use crate::lfd::id::LfdId;
use crate::lfd::store::rows::{
    map_agent_row, map_fork_run_row, map_pending_activation_row, map_stimulus_row, map_summary_row,
    map_wave_row, map_wave_run_row, now_unix, serialize_pr,
};
use crate::lfd::store::{ForkRun, RunStore, StoreError, StoreResult};
use crate::lfd::types::{
    Agent, AgentStatus, PendingActivation, Stimulus, Summary, Wave, WaveRun, WaveRunStatus,
    WaveStatus,
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
        conn.execute_batch(
            "PRAGMA journal_mode = WAL; PRAGMA busy_timeout = 5000; PRAGMA foreign_keys = ON;",
        )?;

        super::migrations::apply_sqlite(&conn)?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn read_waves(&self, repo: Option<&str>) -> StoreResult<Vec<Wave>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let (sql, params): (&str, Vec<Box<dyn ToSql>>) = if let Some(repo) = repo {
            (
                "SELECT id, name, repo, flow, direction, area, paused, status, iteration, created_at
                 FROM waves WHERE repo = ?1 ORDER BY created_at DESC",
                vec![Box::new(repo.to_string())],
            )
        } else {
            (
                "SELECT id, name, repo, flow, direction, area, paused, status, iteration, created_at
                 FROM waves ORDER BY created_at DESC",
                vec![],
            )
        };
        let mut stmt = conn.prepare(sql)?;
        let params_iter = params.iter().map(|v| v.as_ref() as &dyn ToSql);
        let rows = stmt.query_map(rusqlite::params_from_iter(params_iter), |row| {
            Ok(map_wave_row(row))
        })?;

        let mut waves = Vec::new();
        for wave in rows {
            waves.push(wave??);
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

        conn.execute(
            "INSERT INTO waves (
                id, name, repo, flow, direction, area, paused, status, iteration, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                repo = excluded.repo,
                flow = excluded.flow,
                direction = excluded.direction,
                area = excluded.area,
                paused = excluded.paused,
                status = excluded.status,
                iteration = excluded.iteration,
                created_at = excluded.created_at",
            params![
                wave.id,
                wave.name,
                wave.repo,
                wave.flow,
                direction_json,
                area_json,
                if wave.status == WaveStatus::Paused {
                    1i64
                } else {
                    0i64
                },
                wave.status.as_i32() as i64,
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
        super::migrations::latest_version_sqlite(&conn)
    }

    fn list_waves(&self, repo: Option<&str>) -> StoreResult<Vec<Wave>> {
        self.read_waves(repo)
    }

    fn get_wave(&self, wave_id: &LfdId) -> StoreResult<Option<Wave>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, name, repo, flow, direction, area, paused, status, iteration, created_at
             FROM waves WHERE id = ?1",
        )?;
        let wave = stmt
            .query_row(params![wave_id], |row| Ok(map_wave_row(row)))
            .optional()?;
        wave.transpose()
    }

    fn get_wave_by_name(&self, name: &str) -> StoreResult<Option<Wave>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, name, repo, flow, direction, area, paused, status, iteration, created_at
             FROM waves WHERE name = ?1",
        )?;
        let wave = stmt
            .query_row(params![name], |row| Ok(map_wave_row(row)))
            .optional()?;
        wave.transpose()
    }

    fn create_wave(&self, wave: &Wave) -> StoreResult<()> {
        self.upsert_wave(wave)
    }

    fn update_wave(&self, wave: &Wave) -> StoreResult<()> {
        self.upsert_wave(wave)
    }

    fn delete_wave(&self, wave_id: &LfdId) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
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
            "SELECT id, wave_id, iteration, step_index, status, worktree, branch,
                    started_at, ended_at, error, snapshot_repo, snapshot_flow, snapshot_direction,
                    snapshot_area, snapshot_pr, flow_parents, run_kind, sidecar_kind
             FROM wave_runs",
        );
        let mut params_vec: Vec<Box<dyn ToSql>> = Vec::new();
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
        let params_iter = params_vec.iter().map(|v| v.as_ref() as &dyn ToSql);
        let rows = stmt.query_map(rusqlite::params_from_iter(params_iter), |row| {
            Ok(map_wave_run_row(row))
        })?;

        let mut runs = Vec::new();
        for run in rows {
            runs.push(run??);
        }
        Ok(runs)
    }

    fn get_wave_run(&self, wave_run_id: &LfdId) -> StoreResult<Option<WaveRun>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, wave_id, iteration, step_index, status, worktree, branch,
                    started_at, ended_at, error, snapshot_repo, snapshot_flow, snapshot_direction,
                    snapshot_area, snapshot_pr, flow_parents, run_kind, sidecar_kind
             FROM wave_runs WHERE id = ?1",
        )?;
        let run = stmt
            .query_row(params![wave_run_id], |row| Ok(map_wave_run_row(row)))
            .optional()?;
        run.transpose()
    }

    fn get_active_wave_run(&self, wave_id: &LfdId) -> StoreResult<Option<WaveRun>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, wave_id, iteration, step_index, status, worktree, branch,
                    started_at, ended_at, error, snapshot_repo, snapshot_flow, snapshot_direction,
                    snapshot_area, snapshot_pr, flow_parents, run_kind, sidecar_kind
             FROM wave_runs
             WHERE wave_id = ?1 AND status IN (?2, ?3, ?4) AND run_kind = ?5
             ORDER BY started_at DESC LIMIT 1",
        )?;
        let run = stmt
            .query_row(
                params![
                    wave_id,
                    WaveRunStatus::Pending.as_i32() as i64,
                    WaveRunStatus::Running.as_i32() as i64,
                    WaveRunStatus::Waiting.as_i32() as i64,
                    crate::lfd::types::WaveRunKind::Main.as_i32() as i64,
                ],
                |row| Ok(map_wave_run_row(row)),
            )
            .optional()?;
        run.transpose()
    }

    fn get_latest_wave_run(&self, wave_id: &LfdId) -> StoreResult<Option<WaveRun>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, wave_id, iteration, step_index, status, worktree, branch,
                    started_at, ended_at, error, snapshot_repo, snapshot_flow, snapshot_direction,
                    snapshot_area, snapshot_pr, flow_parents, run_kind, sidecar_kind
             FROM wave_runs WHERE wave_id = ?1 AND run_kind = ?2
             ORDER BY started_at DESC LIMIT 1",
        )?;
        let run = stmt
            .query_row(
                params![
                    wave_id,
                    crate::lfd::types::WaveRunKind::Main.as_i32() as i64
                ],
                |row| Ok(map_wave_run_row(row)),
            )
            .optional()?;
        run.transpose()
    }

    fn create_wave_run(&self, run: &WaveRun) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let started_at = run
            .started_at
            .map(|dt| dt.unix_timestamp())
            .unwrap_or_else(now_unix);
        let flow_parents_json = serde_json::to_string(&run.flow_parents)?;
        conn.execute(
            "INSERT INTO wave_runs (
                id, wave_id, iteration, step_index, status, worktree, branch,
                started_at, ended_at, error, snapshot_repo, snapshot_flow, snapshot_direction,
                snapshot_area, snapshot_pr, flow_parents, run_kind, sidecar_kind
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
            params![
                run.id,
                run.wave_id,
                run.iteration as i64,
                run.step_index as i64,
                run.status.as_i32() as i64,
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
                run.run_kind.as_i32() as i64,
                run.sidecar_kind.map(|kind| kind.as_i32() as i64),
            ],
        )?;
        Ok(())
    }

    fn update_wave_run(&self, run: &WaveRun) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let flow_parents_json = serde_json::to_string(&run.flow_parents)?;
        let updated = conn.execute(
            "UPDATE wave_runs
             SET iteration = ?1, step_index = ?2, status = ?3, worktree = ?4,
                 branch = ?5, started_at = ?6, ended_at = ?7, error = ?8,
                 snapshot_repo = ?9, snapshot_flow = ?10, snapshot_direction = ?11,
                 snapshot_area = ?12, snapshot_pr = ?13, flow_parents = ?14,
                 run_kind = ?15, sidecar_kind = ?16
             WHERE id = ?17",
            params![
                run.iteration as i64,
                run.step_index as i64,
                run.status.as_i32() as i64,
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
                run.run_kind.as_i32() as i64,
                run.sidecar_kind.map(|kind| kind.as_i32() as i64),
                run.id,
            ],
        )?;
        if updated == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    fn fail_orphaned_runs(&self) -> StoreResult<u32> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let updated = conn.execute(
            "UPDATE wave_runs SET status = ?1, error = ?2, ended_at = ?3
             WHERE status IN (?4, ?5, ?6)",
            params![
                WaveRunStatus::Failed.as_i32() as i64,
                "orphaned: lfd restarted",
                now_unix(),
                WaveRunStatus::Pending.as_i32() as i64,
                WaveRunStatus::Running.as_i32() as i64,
                WaveRunStatus::Waiting.as_i32() as i64,
            ],
        )?;
        Ok(updated as u32)
    }

    fn list_stimuli(&self, wave_id: Option<&LfdId>) -> StoreResult<Vec<Stimulus>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let (query, params): (&str, Vec<Box<dyn ToSql>>) = if let Some(wave_id) = wave_id {
            (
                "SELECT id, wave_id, kind, cron, last_main_sha, last_triggered_at, created_at, enabled
                 FROM stimuli WHERE wave_id = ?1 ORDER BY created_at",
                vec![Box::new(wave_id.clone())],
            )
        } else {
            (
                "SELECT id, wave_id, kind, cron, last_main_sha, last_triggered_at, created_at, enabled
                 FROM stimuli ORDER BY created_at",
                vec![],
            )
        };

        let mut stmt = conn.prepare(query)?;
        let params_iter = params.iter().map(|v| v.as_ref() as &dyn ToSql);
        let rows = stmt.query_map(rusqlite::params_from_iter(params_iter), |row| {
            Ok(map_stimulus_row(row))
        })?;

        let mut stimuli = Vec::new();
        for stimulus in rows {
            stimuli.push(stimulus??);
        }
        Ok(stimuli)
    }

    fn list_stimuli_by_kind(&self, kind: i32) -> StoreResult<Vec<Stimulus>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, wave_id, kind, cron, last_main_sha, last_triggered_at, created_at, enabled
             FROM stimuli WHERE kind = ?1 ORDER BY created_at",
        )?;
        let rows = stmt.query_map(params![kind as i64], |row| Ok(map_stimulus_row(row)))?;
        let mut stimuli = Vec::new();
        for stimulus in rows {
            stimuli.push(stimulus??);
        }
        Ok(stimuli)
    }

    fn get_stimulus(&self, stimulus_id: &LfdId) -> StoreResult<Option<Stimulus>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, wave_id, kind, cron, last_main_sha, last_triggered_at, created_at, enabled
             FROM stimuli WHERE id = ?1",
        )?;
        let stimulus = stmt
            .query_row(params![stimulus_id], |row| Ok(map_stimulus_row(row)))
            .optional()?;
        stimulus.transpose()
    }

    fn create_stimulus(&self, stimulus: &Stimulus) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let created_at = stimulus
            .created_at
            .map(|dt| dt.unix_timestamp())
            .unwrap_or_else(now_unix);

        conn.execute(
            "INSERT INTO stimuli (id, wave_id, kind, cron, last_main_sha, last_triggered_at, created_at, enabled)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                stimulus.id,
                stimulus.wave_id,
                stimulus.kind.as_i32() as i64,
                stimulus.cron,
                stimulus.last_main_sha,
                stimulus.last_triggered_at,
                created_at,
                stimulus.enabled as i64,
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
                stimulus.kind.as_i32() as i64,
                stimulus.cron,
                stimulus.last_main_sha,
                stimulus.last_triggered_at,
                stimulus.enabled as i64,
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

    fn list_pending_activations(&self, wave_id: &LfdId) -> StoreResult<Vec<PendingActivation>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, wave_id, stimulus_id, from_sha, to_sha, queued_at
             FROM pending_activations WHERE wave_id = ?1 ORDER BY queued_at",
        )?;
        let rows = stmt.query_map(params![wave_id], |row| Ok(map_pending_activation_row(row)))?;
        let mut activations = Vec::new();
        for activation in rows {
            activations.push(activation??);
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
                Ok(map_pending_activation_row(row))
            })
            .optional()?;
        activation.transpose()
    }

    fn list_fork_runs(&self, wave_run_id: &LfdId, step_index: u32) -> StoreResult<Vec<ForkRun>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, wave_run_id, step_index, branch_index, status, worktree
             FROM fork_runs WHERE wave_run_id = ?1 AND step_index = ?2
             ORDER BY branch_index ASC",
        )?;
        let rows = stmt.query_map(params![wave_run_id, step_index as i64], |row| {
            Ok(map_fork_run_row(row))
        })?;
        let mut runs = Vec::new();
        for run in rows {
            runs.push(run??);
        }
        Ok(runs)
    }

    fn upsert_fork_run(&self, fork_run: &ForkRun) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO fork_runs (id, wave_run_id, step_index, branch_index, status, worktree)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
                 wave_run_id = excluded.wave_run_id,
                 step_index = excluded.step_index,
                 branch_index = excluded.branch_index,
                 status = excluded.status,
                 worktree = excluded.worktree",
            params![
                fork_run.id,
                fork_run.wave_run_id,
                fork_run.step_index as i64,
                fork_run.branch_index as i64,
                fork_run.status as i32 as i64,
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
            "SELECT id, step, repo, worktree, wave_run_id, status,
                    started_at, ended_at, pid, container_id, model, run_mode
             FROM agents",
        );
        let mut params_vec: Vec<Box<dyn ToSql>> = Vec::new();
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
        let params_iter = params_vec.iter().map(|v| v.as_ref() as &dyn ToSql);
        let rows = stmt.query_map(rusqlite::params_from_iter(params_iter), |row| {
            Ok(map_agent_row(row))
        })?;

        let mut runs = Vec::new();
        for run in rows {
            runs.push(run??);
        }
        Ok(runs)
    }

    fn get_agent(&self, agent_id: &LfdId) -> StoreResult<Option<Agent>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, step, repo, worktree, wave_run_id, status,
                    started_at, ended_at, pid, container_id, model, run_mode
             FROM agents WHERE id = ?1",
        )?;
        let run = stmt
            .query_row(params![agent_id], |row| Ok(map_agent_row(row)))
            .optional()?;
        run.transpose()
    }

    fn get_waiting_agent_for_wave(&self, wave_id: &LfdId) -> StoreResult<Option<Agent>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT a.id, a.step, a.repo, a.worktree, a.wave_run_id, a.status,
                    a.started_at, a.ended_at, a.pid, a.container_id, a.model, a.run_mode
             FROM agents a JOIN wave_runs r ON a.wave_run_id = r.id
             WHERE r.wave_id = ?1 AND a.status = ?2
             ORDER BY a.started_at DESC LIMIT 1",
        )?;
        let run = stmt
            .query_row(
                params![wave_id, AgentStatus::Waiting.as_i32() as i64],
                |row| Ok(map_agent_row(row)),
            )
            .optional()?;
        run.transpose()
    }

    fn start_agent(&self, agent: &Agent) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let started_at = agent
            .started_at
            .map(|dt| dt.unix_timestamp())
            .unwrap_or_else(now_unix);
        conn.execute(
            "INSERT INTO agents (
                id, step, repo, worktree, wave_run_id, status, started_at,
                ended_at, pid, container_id, model, run_mode
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                agent.id,
                agent.step,
                agent.repo,
                agent.worktree,
                agent.wave_run_id,
                agent.status.as_i32() as i64,
                started_at,
                agent.ended_at.map(|dt| dt.unix_timestamp()),
                agent.pid.map(|v| v as i64),
                agent.container_id.as_deref(),
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
        container_id: Option<&str>,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let updated = conn.execute(
            "UPDATE agents
             SET status = ?1,
                 pid = COALESCE(?2, pid),
                 container_id = COALESCE(?3, container_id)
             WHERE id = ?4",
            params![status as i64, pid.map(|v| v as i64), container_id, agent_id],
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
            params![status as i64, ended_at, agent_id],
        )?;
        if updated == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    fn get_active_agents_for_wave(&self, wave_id: &LfdId) -> StoreResult<Vec<Agent>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT a.id, a.step, a.repo, a.worktree, a.wave_run_id, a.status,
                    a.started_at, a.ended_at, a.pid, a.container_id, a.model, a.run_mode
             FROM agents a JOIN wave_runs r ON a.wave_run_id = r.id
             WHERE r.wave_id = ?1 AND a.ended_at IS NULL
             ORDER BY a.started_at DESC",
        )?;
        let rows = stmt.query_map(params![wave_id], |row| Ok(map_agent_row(row)))?;
        let mut agents = Vec::new();
        for row in rows {
            agents.push(row??);
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
            params![status as i64, ended_at, wave_id.as_str()],
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
            "SELECT id, step, repo, worktree, wave_run_id, status,
                    started_at, ended_at, pid, container_id, model, run_mode
             FROM agents WHERE ended_at IS NULL AND started_at <= ?1
             ORDER BY started_at ASC",
        )?;
        let rows = stmt.query_map(params![cutoff], |row| Ok(map_agent_row(row)))?;
        let mut runs = Vec::new();
        for run in rows {
            runs.push(run??);
        }
        Ok(runs)
    }

    fn get_summary(&self, wave_id: &LfdId) -> StoreResult<Option<Summary>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, wave_id, content, source_hash, token_budget, model, created_at
             FROM summaries WHERE wave_id = ?1",
        )?;
        let summary = stmt
            .query_row(params![wave_id], |row| Ok(map_summary_row(row)))
            .optional()?;
        summary.transpose()
    }

    fn upsert_summary(&self, summary: &Summary) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let created_at = summary
            .created_at
            .map(|dt| dt.unix_timestamp())
            .unwrap_or_else(now_unix);

        conn.execute(
            "INSERT INTO summaries (id, wave_id, content, source_hash, token_budget, model, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(wave_id) DO UPDATE SET
                 content = excluded.content,
                 source_hash = excluded.source_hash,
                 token_budget = excluded.token_budget,
                 model = excluded.model,
                 created_at = excluded.created_at",
            params![
                summary.id,
                summary.wave_id,
                summary.content,
                summary.source_hash,
                summary.token_budget as i64,
                summary.model,
                created_at,
            ],
        )?;
        Ok(())
    }
}
