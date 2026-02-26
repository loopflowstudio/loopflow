use std::path::Path;
use std::sync::{Arc, Mutex};

use rusqlite::{params, Connection, OptionalExtension, ToSql};

use crate::lfd::id::LfdId;
use crate::lfd::sessions::types::{
    PersistedSessionEvent, Session, SessionConfig, SessionEvent, SessionStatus,
};
use crate::lfd::store::catalog::{
    list_agent_history_query, list_stimuli_query, list_wave_runs_query, list_waves_query, sql,
    Query, SqlDialect,
};
use crate::lfd::store::rows::{
    map_activation_log_row, map_agent_row, map_chat_memory_block_row, map_chat_message_row,
    map_chord_row,
    map_fork_run_row, map_live_pr_state_row, map_pending_activation_row, map_stimulus_row,
    map_summary_row, map_wave_row, map_wave_run_row, now_unix, serialize_pr,
};
use crate::lfd::store::{ForkRun, ForkRunStatus, StoreError, StoreResult};
use crate::lfd::types::{
    ActivationLog, AgentRun, AgentStatus, ChatMemoryBlock, ChatMessage, Chord, LivePullRequestState,
    PendingActivation, QueueBlock, QueueBlockReason, QueueMergeEvent, Stimulus, Summary, Wave,
    WaveRun, WaveRunStatus, WaveStatus,
};

#[derive(Debug, Clone)]
pub struct SqliteStore {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteStore {
    fn sql(query: Query) -> &'static str {
        sql(query, SqlDialect::Sqlite)
    }

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
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    fn read_waves(&self, repo: Option<&str>) -> StoreResult<Vec<Wave>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let query = Self::sql(list_waves_query(repo.is_some()));
        let params: Vec<Box<dyn ToSql>> = if let Some(repo) = repo {
            vec![Box::new(repo.to_string())]
        } else {
            vec![]
        };
        let mut stmt = conn.prepare(query)?;
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
        let direction_json = serde_json::to_string(wave.direction())?;
        let area_json = serde_json::to_string(wave.area())?;
        let created_at = wave
            .created_at()
            .map(|dt| dt.unix_timestamp())
            .unwrap_or_else(now_unix);

        conn.execute(
            Self::sql(Query::UpsertWave),
            params![
                wave.id(),
                wave.name(),
                wave.repo(),
                wave.flow(),
                direction_json,
                area_json,
                if wave.status() == WaveStatus::Paused {
                    1i64
                } else {
                    0i64
                },
                wave.status().as_i32() as i64,
                wave.iteration() as i64,
                created_at,
            ],
        )?;
        Ok(())
    }

    fn resource_exists(&self, query: &str, id: &LfdId) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(query)?;
        let exists = stmt
            .query_row(params![id], |_| Ok(()))
            .optional()?
            .is_some();
        Ok(exists)
    }

    fn chord_exists(&self, chord_id: &LfdId) -> StoreResult<bool> {
        self.resource_exists("SELECT 1 FROM chords WHERE id = ?1", chord_id)
    }

    fn wave_exists(&self, wave_id: &LfdId) -> StoreResult<bool> {
        self.resource_exists("SELECT 1 FROM waves WHERE id = ?1", wave_id)
    }

    fn ensure_chord_exists(&self, chord_id: &LfdId) -> StoreResult<()> {
        if self.chord_exists(chord_id)? {
            return Ok(());
        }
        Err(StoreError::NotFound)
    }

    fn ensure_wave_exists(&self, wave_id: &LfdId) -> StoreResult<()> {
        if self.wave_exists(wave_id)? {
            return Ok(());
        }
        Err(StoreError::NotFound)
    }

    fn ensure_chord_member_resources_exist(
        &self,
        chord_id: &LfdId,
        wave_id: &LfdId,
    ) -> StoreResult<()> {
        self.ensure_chord_exists(chord_id)?;
        self.ensure_wave_exists(wave_id)
    }

    fn map_session_row(row: &rusqlite::Row<'_>) -> Result<Session, rusqlite::Error> {
        let config_text: String = row.get(5)?;
        let config: SessionConfig = serde_json::from_str(&config_text).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::new(err))
        })?;
        Ok(Session {
            id: row.get(0)?,
            harness: row.get(1)?,
            status: SessionStatus::from_i32(row.get::<_, i64>(2)? as i32),
            wave_run_id: row.get(3)?,
            provider_session_id: row.get(4)?,
            config,
            created_at: crate::lfd::store::rows::unix_to_datetime(row.get(6)?),
            ended_at: row
                .get::<_, Option<i64>>(7)?
                .map(crate::lfd::store::rows::unix_to_datetime),
        })
    }
}

impl SqliteStore {
    pub fn health_check(&self) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(Self::sql(Query::HealthCheck), [])?;
        Ok(())
    }

    pub fn schema_version(&self) -> StoreResult<String> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        super::migrations::latest_version_sqlite(&conn)
    }

    pub fn create_session(&self, session: &Session) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO sessions (id, harness, status, wave_run_id, provider_session_id, config, created_at, ended_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                session.id,
                session.harness,
                session.status.as_i32() as i64,
                session.wave_run_id,
                session.provider_session_id,
                serde_json::to_string(&session.config)?,
                session.created_at.unix_timestamp(),
                session.ended_at.map(|dt| dt.unix_timestamp()),
            ],
        )?;
        Ok(())
    }

    pub fn get_session(&self, session_id: &LfdId) -> StoreResult<Option<Session>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, harness, status, wave_run_id, provider_session_id, config, created_at, ended_at
             FROM sessions WHERE id = ?1",
        )?;
        let row = stmt
            .query_row(params![session_id], Self::map_session_row)
            .optional()?;
        Ok(row)
    }

    pub fn get_active_session_for_wave_run(
        &self,
        wave_run_id: &str,
    ) -> StoreResult<Option<Session>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, harness, status, wave_run_id, provider_session_id, config, created_at, ended_at
             FROM sessions
             WHERE wave_run_id = ?1 AND status IN (?2, ?3, ?4)
             ORDER BY created_at DESC
             LIMIT 1",
        )?;
        let row = stmt
            .query_row(
                params![
                    wave_run_id,
                    SessionStatus::Starting.as_i32() as i64,
                    SessionStatus::Active.as_i32() as i64,
                    SessionStatus::Ending.as_i32() as i64
                ],
                Self::map_session_row,
            )
            .optional()?;
        Ok(row)
    }

    pub fn update_provider_session_id(
        &self,
        session_id: &LfdId,
        provider_session_id: &str,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let updated = conn.execute(
            "UPDATE sessions SET provider_session_id = ?2 WHERE id = ?1",
            params![session_id, provider_session_id],
        )?;
        if updated == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    pub fn update_session_status(
        &self,
        session_id: &LfdId,
        status: SessionStatus,
        ended_at: Option<i64>,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let updated = conn.execute(
            "UPDATE sessions
             SET status = ?2, ended_at = COALESCE(?3, ended_at)
             WHERE id = ?1",
            params![session_id, status.as_i32() as i64, ended_at],
        )?;
        if updated == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    pub fn append_session_event(
        &self,
        session_id: &LfdId,
        seq: i64,
        event: &SessionEvent,
        created_at: i64,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO session_events (session_id, seq, event_type, data, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                session_id,
                seq,
                event.event_type(),
                serde_json::to_string(event)?,
                created_at,
            ],
        )?;
        Ok(())
    }

    pub fn list_session_events(
        &self,
        session_id: &LfdId,
        after_seq: Option<i64>,
    ) -> StoreResult<Vec<PersistedSessionEvent>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = if after_seq.is_some() {
            conn.prepare(
                "SELECT session_id, seq, data, created_at
                 FROM session_events
                 WHERE session_id = ?1 AND seq > ?2
                 ORDER BY seq ASC",
            )?
        } else {
            conn.prepare(
                "SELECT session_id, seq, data, created_at
                 FROM session_events
                 WHERE session_id = ?1
                 ORDER BY seq ASC",
            )?
        };

        let mut rows = if let Some(after_seq) = after_seq {
            stmt.query(params![session_id, after_seq])?
        } else {
            stmt.query(params![session_id])?
        };

        let mut events = Vec::new();
        while let Some(row) = rows.next()? {
            let data: String = row.get(2)?;
            let event: SessionEvent = serde_json::from_str(&data)?;
            events.push(PersistedSessionEvent {
                session_id: row.get(0)?,
                seq: row.get(1)?,
                event,
                created_at: crate::lfd::store::rows::unix_to_datetime(row.get(3)?),
            });
        }
        Ok(events)
    }

    pub fn list_sessions_by_statuses(
        &self,
        statuses: &[SessionStatus],
    ) -> StoreResult<Vec<Session>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let placeholders: Vec<String> = statuses
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect();
        let sql = format!(
            "SELECT id, harness, status, wave_run_id, provider_session_id, config, created_at, ended_at
             FROM sessions WHERE status IN ({})
             ORDER BY created_at ASC",
            placeholders.join(", ")
        );
        let mut stmt = conn.prepare(&sql)?;
        let params: Vec<Box<dyn ToSql>> = statuses
            .iter()
            .map(|s| Box::new(s.as_i32() as i64) as Box<dyn ToSql>)
            .collect();
        let param_refs: Vec<&dyn ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let mut rows = stmt.query(param_refs.as_slice())?;
        let mut sessions = Vec::new();
        while let Some(row) = rows.next()? {
            sessions.push(Self::map_session_row(row)?);
        }
        Ok(sessions)
    }

    pub fn list_waves(&self, repo: Option<&str>) -> StoreResult<Vec<Wave>> {
        self.read_waves(repo)
    }

    pub fn get_wave(&self, wave_id: &LfdId) -> StoreResult<Option<Wave>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(Self::sql(Query::GetWaveById))?;
        let wave = stmt
            .query_row(params![wave_id], |row| Ok(map_wave_row(row)))
            .optional()?;
        wave.transpose()
    }

    pub fn get_wave_by_name(&self, name: &str) -> StoreResult<Option<Wave>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(Self::sql(Query::GetWaveByName))?;
        let wave = stmt
            .query_row(params![name], |row| Ok(map_wave_row(row)))
            .optional()?;
        wave.transpose()
    }

    pub fn create_wave(&self, wave: &Wave) -> StoreResult<()> {
        self.upsert_wave(wave)
    }

    pub fn update_wave(&self, wave: &Wave) -> StoreResult<()> {
        self.upsert_wave(wave)
    }

    pub fn delete_wave(&self, wave_id: &LfdId) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(Self::sql(Query::DeleteWaveById), params![wave_id])?;
        Ok(())
    }

    pub fn create_chord(&self, name: &str) -> StoreResult<Chord> {
        let chord = Chord {
            id: LfdId::new(),
            name: name.to_string(),
            is_default: false,
            created_at: Some(time::OffsetDateTime::now_utc()),
        };
        let created_at = chord
            .created_at
            .map(|value| value.unix_timestamp())
            .unwrap_or_else(now_unix);
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO chords (id, name, is_default, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                &chord.id,
                &chord.name,
                if chord.is_default { 1i64 } else { 0i64 },
                created_at
            ],
        )?;
        Ok(chord)
    }

    pub fn get_chord(&self, chord_id: &LfdId) -> StoreResult<Option<Chord>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt =
            conn.prepare("SELECT id, name, is_default, created_at FROM chords WHERE id = ?1")?;
        let chord = stmt
            .query_row(params![chord_id], |row| Ok(map_chord_row(row)))
            .optional()?;
        chord.transpose()
    }

    pub fn list_chords(&self) -> StoreResult<Vec<Chord>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, name, is_default, created_at FROM chords ORDER BY created_at, name",
        )?;
        let rows = stmt.query_map([], |row| Ok(map_chord_row(row)))?;
        let mut chords = Vec::new();
        for chord in rows {
            chords.push(chord??);
        }
        Ok(chords)
    }

    pub fn delete_chord(&self, chord_id: &LfdId) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let deleted = conn.execute("DELETE FROM chords WHERE id = ?1", params![chord_id])?;
        if deleted == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    pub fn add_chord_member(&self, chord_id: &LfdId, wave_id: &LfdId) -> StoreResult<()> {
        self.ensure_chord_member_resources_exist(chord_id, wave_id)?;
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT OR IGNORE INTO chord_members (chord_id, wave_id) VALUES (?1, ?2)",
            params![chord_id, wave_id],
        )?;
        Ok(())
    }

    pub fn remove_chord_member(&self, chord_id: &LfdId, wave_id: &LfdId) -> StoreResult<()> {
        self.ensure_chord_member_resources_exist(chord_id, wave_id)?;
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "DELETE FROM chord_members WHERE chord_id = ?1 AND wave_id = ?2",
            params![chord_id, wave_id],
        )?;
        Ok(())
    }

    pub fn list_chord_members(&self, chord_id: &LfdId) -> StoreResult<Vec<Wave>> {
        self.ensure_chord_exists(chord_id)?;

        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT w.id, w.name, w.repo, w.flow, w.direction, w.area, w.paused, w.status, w.iteration, w.created_at
             FROM waves w
             INNER JOIN chord_members cm ON cm.wave_id = w.id
             WHERE cm.chord_id = ?1
             ORDER BY w.created_at, w.name",
        )?;
        let rows = stmt.query_map(params![chord_id], |row| Ok(map_wave_row(row)))?;
        let mut waves = Vec::new();
        for wave in rows {
            waves.push(wave??);
        }
        Ok(waves)
    }

    pub fn list_chords_for_wave(&self, wave_id: &LfdId) -> StoreResult<Vec<Chord>> {
        self.ensure_wave_exists(wave_id)?;
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT c.id, c.name, c.is_default, c.created_at
             FROM chords c
             INNER JOIN chord_members cm ON cm.chord_id = c.id
             WHERE cm.wave_id = ?1
             ORDER BY c.created_at, c.name",
        )?;
        let rows = stmt.query_map(params![wave_id], |row| Ok(map_chord_row(row)))?;
        let mut chords = Vec::new();
        for chord in rows {
            chords.push(chord??);
        }
        Ok(chords)
    }

    pub fn list_wave_runs(
        &self,
        wave_id: Option<&LfdId>,
        limit: Option<u32>,
    ) -> StoreResult<Vec<WaveRun>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let query = Self::sql(list_wave_runs_query(wave_id.is_some(), limit.is_some()));
        let mut params_vec: Vec<Box<dyn ToSql>> = Vec::new();
        if let Some(wave_id) = wave_id {
            params_vec.push(Box::new(wave_id.clone()));
        }
        if let Some(limit) = limit {
            params_vec.push(Box::new(limit as i64));
        }

        let mut stmt = conn.prepare(query)?;
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

    pub fn get_wave_run(&self, wave_run_id: &LfdId) -> StoreResult<Option<WaveRun>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(Self::sql(Query::GetWaveRunById))?;
        let run = stmt
            .query_row(params![wave_run_id], |row| Ok(map_wave_run_row(row)))
            .optional()?;
        run.transpose()
    }

    pub fn get_active_wave_run(&self, wave_id: &LfdId) -> StoreResult<Option<WaveRun>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(Self::sql(Query::GetActiveWaveRun))?;
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

    pub fn get_latest_wave_run(&self, wave_id: &LfdId) -> StoreResult<Option<WaveRun>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(Self::sql(Query::GetLatestWaveRun))?;
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

    pub fn create_wave_run(&self, run: &WaveRun) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let started_at = run
            .started_at
            .map(|dt| dt.unix_timestamp())
            .unwrap_or_else(now_unix);
        let flow_parents_json = serde_json::to_string(&run.flow_parents)?;
        conn.execute(
            Self::sql(Query::InsertWaveRun),
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
                run.activation_log_id.as_ref(),
                run.run_kind.as_i32() as i64,
                run.sidecar_kind.map(|kind| kind.as_i32() as i64),
                run.parent_run_id.as_ref(),
                run.parent_pr_number.map(|value| value as i64),
                run.stack_position as i64,
                run.stack_group_id,
                run.stack_status.as_i32() as i64,
                if run.lineage_inferred { 1i64 } else { 0i64 },
            ],
        )?;
        Ok(())
    }

    pub fn update_wave_run(&self, run: &WaveRun) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let flow_parents_json = serde_json::to_string(&run.flow_parents)?;
        let updated = conn.execute(
            Self::sql(Query::UpdateWaveRun),
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
                run.activation_log_id.as_ref(),
                run.run_kind.as_i32() as i64,
                run.sidecar_kind.map(|kind| kind.as_i32() as i64),
                run.parent_run_id.as_ref(),
                run.parent_pr_number.map(|value| value as i64),
                run.stack_position as i64,
                run.stack_group_id,
                run.stack_status.as_i32() as i64,
                if run.lineage_inferred { 1i64 } else { 0i64 },
                run.id,
            ],
        )?;
        if updated == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    pub fn list_stack_runs(&self, wave_id: &LfdId) -> StoreResult<Vec<WaveRun>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(Self::sql(Query::ListStackRuns))?;
        let rows = stmt.query_map(
            params![
                wave_id,
                crate::lfd::types::WaveRunKind::Main.as_i32() as i64
            ],
            |row| Ok(map_wave_run_row(row)),
        )?;

        let mut runs = Vec::new();
        for run in rows {
            runs.push(run??);
        }
        Ok(runs)
    }

    pub fn get_live_pr_state(
        &self,
        repo_id: &str,
        pr_number: u32,
    ) -> StoreResult<Option<LivePullRequestState>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(Self::sql(Query::GetLivePrState))?;
        let state = stmt
            .query_row(params![repo_id, pr_number as i64], |row| {
                Ok(map_live_pr_state_row(row))
            })
            .optional()?;
        state.transpose()
    }

    pub fn upsert_live_pr_state(&self, state: &LivePullRequestState) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            Self::sql(Query::UpsertLivePrState),
            params![
                state.repo_id,
                state.pr_number as i64,
                state.state.as_i32() as i64,
                if state.is_draft { 1i64 } else { 0i64 },
                state.head_ref,
                state.head_sha,
                state.base_ref,
                state.updated_at.unix_timestamp(),
                state.merged_at.map(|value| value.unix_timestamp()),
                state.synced_at.unix_timestamp(),
            ],
        )?;
        Ok(())
    }

    pub fn list_queue_blocks(&self, wave_id: &LfdId) -> StoreResult<Vec<QueueBlock>> {
        struct RawQueueBlock {
            wave_id: LfdId,
            run_id: LfdId,
            reason: String,
            attempted_at: i64,
            conflict_files: Vec<String>,
            error: Option<String>,
        }

        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT wave_id, run_id, reason, attempted_at, conflict_files, error
             FROM wave_queue_blocks
             WHERE wave_id = ?1
             ORDER BY attempted_at DESC",
        )?;
        let rows = stmt.query_map(params![wave_id], |row| {
            let conflict_files = row
                .get::<_, String>(4)
                .ok()
                .and_then(|value| serde_json::from_str::<Vec<String>>(&value).ok())
                .unwrap_or_default();
            Ok(RawQueueBlock {
                wave_id: LfdId::from_raw(row.get::<_, String>(0)?),
                run_id: LfdId::from_raw(row.get::<_, String>(1)?),
                reason: row.get(2)?,
                attempted_at: row.get::<_, i64>(3)?,
                conflict_files,
                error: row.get(5)?,
            })
        })?;
        let mut blocks = Vec::new();
        for raw in rows {
            let raw = raw?;
            let reason = raw
                .reason
                .parse::<QueueBlockReason>()
                .map_err(StoreError::InvalidData)?;
            blocks.push(QueueBlock {
                wave_id: raw.wave_id,
                run_id: raw.run_id,
                reason,
                attempted_at: crate::lfd::store::rows::unix_to_datetime(raw.attempted_at),
                conflict_files: raw.conflict_files,
                error: raw.error,
            });
        }
        Ok(blocks)
    }

    pub fn upsert_queue_block(&self, block: &QueueBlock) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO wave_queue_blocks (wave_id, run_id, reason, attempted_at, conflict_files, error)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(wave_id, run_id) DO UPDATE SET
                reason = excluded.reason,
                attempted_at = excluded.attempted_at,
                conflict_files = excluded.conflict_files,
                error = excluded.error",
            params![
                block.wave_id,
                block.run_id,
                block.reason.as_str(),
                block.attempted_at.unix_timestamp(),
                serde_json::to_string(&block.conflict_files)?,
                block.error,
            ],
        )?;
        Ok(())
    }

    pub fn delete_queue_block(&self, wave_id: &LfdId, run_id: &LfdId) -> StoreResult<u32> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let deleted = conn.execute(
            "DELETE FROM wave_queue_blocks WHERE wave_id = ?1 AND run_id = ?2",
            params![wave_id, run_id],
        )?;
        Ok(deleted as u32)
    }

    pub fn record_merge_event(&self, event: &QueueMergeEvent) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let inserted = conn.execute(
            "INSERT INTO wave_pr_merge_events (wave_id, pr_number, merged_at, processed_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(wave_id, pr_number, merged_at) DO NOTHING",
            params![
                event.wave_id,
                event.pr_number as i64,
                event.merged_at.unix_timestamp(),
                event.processed_at.unix_timestamp(),
            ],
        )?;
        Ok(inserted > 0)
    }

    pub fn fail_orphaned_runs(&self) -> StoreResult<u32> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let updated = conn.execute(
            Self::sql(Query::FailOrphanedRuns),
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

    pub fn list_stimuli(&self, wave_id: Option<&LfdId>) -> StoreResult<Vec<Stimulus>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let query = Self::sql(list_stimuli_query(wave_id.is_some()));
        let params: Vec<Box<dyn ToSql>> = if let Some(wave_id) = wave_id {
            vec![Box::new(wave_id.clone())]
        } else {
            vec![]
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

    pub fn list_stimuli_by_kind(&self, kind: i32) -> StoreResult<Vec<Stimulus>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(Self::sql(Query::ListStimuliByKind))?;
        let rows = stmt.query_map(params![kind as i64], |row| Ok(map_stimulus_row(row)))?;
        let mut stimuli = Vec::new();
        for stimulus in rows {
            stimuli.push(stimulus??);
        }
        Ok(stimuli)
    }

    pub fn get_stimulus(&self, stimulus_id: &LfdId) -> StoreResult<Option<Stimulus>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(Self::sql(Query::GetStimulusById))?;
        let stimulus = stmt
            .query_row(params![stimulus_id], |row| Ok(map_stimulus_row(row)))
            .optional()?;
        stimulus.transpose()
    }

    pub fn create_stimulus(&self, stimulus: &Stimulus) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let created_at = stimulus
            .created_at
            .map(|dt| dt.unix_timestamp())
            .unwrap_or_else(now_unix);

        conn.execute(
            Self::sql(Query::InsertStimulus),
            params![
                stimulus.id,
                stimulus.wave_id,
                stimulus.kind.as_i32() as i64,
                stimulus.cron,
                stimulus.last_main_sha,
                stimulus.last_triggered_at,
                created_at,
                stimulus.enabled as i64,
                stimulus.source_wave_id,
            ],
        )?;
        Ok(())
    }

    pub fn update_stimulus(&self, stimulus: &Stimulus) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let updated = conn.execute(
            Self::sql(Query::UpdateStimulus),
            params![
                stimulus.kind.as_i32() as i64,
                stimulus.cron,
                stimulus.last_main_sha,
                stimulus.last_triggered_at,
                stimulus.enabled as i64,
                stimulus.source_wave_id,
                stimulus.id,
            ],
        )?;
        if updated == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    pub fn delete_stimulus(&self, stimulus_id: &LfdId) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(Self::sql(Query::DeleteStimulusById), params![stimulus_id])?;
        Ok(())
    }

    pub fn list_pending_activations(&self, wave_id: &LfdId) -> StoreResult<Vec<PendingActivation>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(Self::sql(Query::ListPendingActivationsByWave))?;
        let rows = stmt.query_map(params![wave_id], |row| Ok(map_pending_activation_row(row)))?;
        let mut activations = Vec::new();
        for activation in rows {
            activations.push(activation??);
        }
        Ok(activations)
    }

    pub fn create_pending_activation(&self, activation: &PendingActivation) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            Self::sql(Query::InsertPendingActivation),
            params![
                activation.id,
                activation.wave_id,
                activation.stimulus_id,
                activation.source.as_i32() as i64,
                activation.reason,
                activation.from_sha,
                activation.to_sha,
                activation.queued_at,
            ],
        )?;
        Ok(())
    }

    pub fn update_pending_activation(&self, activation: &PendingActivation) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let updated = conn.execute(
            Self::sql(Query::UpdatePendingActivation),
            params![
                activation.source.as_i32() as i64,
                activation.reason,
                activation.from_sha,
                activation.to_sha,
                activation.id
            ],
        )?;
        if updated == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    pub fn delete_pending_activations(&self, wave_id: &LfdId) -> StoreResult<u32> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let deleted = conn.execute(
            Self::sql(Query::DeletePendingActivationsByWave),
            params![wave_id],
        )?;
        Ok(deleted as u32)
    }

    pub fn get_pending_for_stimulus(
        &self,
        wave_id: &LfdId,
        stimulus_id: &LfdId,
    ) -> StoreResult<Option<PendingActivation>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(Self::sql(Query::GetPendingActivationForStimulus))?;
        let activation = stmt
            .query_row(params![wave_id, stimulus_id], |row| {
                Ok(map_pending_activation_row(row))
            })
            .optional()?;
        activation.transpose()
    }

    pub fn delete_pending_activation_by_id(&self, activation_id: &LfdId) -> StoreResult<u32> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let deleted = conn.execute(
            Self::sql(Query::DeletePendingActivationById),
            params![activation_id],
        )?;
        Ok(deleted as u32)
    }

    pub fn create_activation_log(&self, log: &ActivationLog) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            Self::sql(Query::InsertActivationLog),
            params![
                log.id,
                log.wave_id,
                log.stimulus_id,
                log.source.as_i32() as i64,
                log.reason,
                log.outcome.as_str(),
                log.created_at,
            ],
        )?;
        Ok(())
    }

    pub fn list_activation_log(
        &self,
        wave_id: &LfdId,
        limit: u32,
    ) -> StoreResult<Vec<ActivationLog>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(Self::sql(Query::ListActivationLogByWave))?;
        let rows = stmt.query_map(params![wave_id, limit as i64], |row| {
            Ok(map_activation_log_row(row))
        })?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row??);
        }
        Ok(entries)
    }

    pub fn get_activation_log(
        &self,
        activation_log_id: &LfdId,
    ) -> StoreResult<Option<ActivationLog>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(Self::sql(Query::GetActivationLogById))?;
        let entry = stmt
            .query_row(params![activation_log_id], |row| {
                Ok(map_activation_log_row(row))
            })
            .optional()?;
        entry.transpose()
    }

    pub fn list_fork_runs(
        &self,
        wave_run_id: &LfdId,
        step_index: u32,
    ) -> StoreResult<Vec<ForkRun>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(Self::sql(Query::ListForkRuns))?;
        let rows = stmt.query_map(params![wave_run_id, step_index as i64], |row| {
            Ok(map_fork_run_row(row))
        })?;
        let mut runs = Vec::new();
        for run in rows {
            runs.push(run??);
        }
        Ok(runs)
    }

    pub fn list_orphaned_fork_runs(&self) -> StoreResult<Vec<ForkRun>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT fr.id, fr.wave_run_id, fr.step_index, fr.branch_index, fr.status, fr.worktree
             FROM fork_runs fr
             LEFT JOIN wave_runs wr ON wr.id = fr.wave_run_id
             WHERE fr.status IN (?1, ?2)
               AND (
                 wr.id IS NULL
                 OR wr.status NOT IN (?3, ?4, ?5)
                 OR fr.step_index != wr.step_index
               )
             ORDER BY fr.wave_run_id ASC, fr.step_index ASC, fr.branch_index ASC",
        )?;
        let rows = stmt.query_map(
            params![
                ForkRunStatus::Pending as i32 as i64,
                ForkRunStatus::Running as i32 as i64,
                WaveRunStatus::Pending.as_i32() as i64,
                WaveRunStatus::Running.as_i32() as i64,
                WaveRunStatus::Waiting.as_i32() as i64
            ],
            |row| Ok(map_fork_run_row(row)),
        )?;
        let mut runs = Vec::new();
        for run in rows {
            runs.push(run??);
        }
        Ok(runs)
    }

    pub fn upsert_fork_run(&self, fork_run: &ForkRun) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            Self::sql(Query::UpsertForkRun),
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

    pub fn delete_fork_runs(&self, wave_run_id: &LfdId, step_index: u32) -> StoreResult<u32> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let deleted = conn.execute(
            Self::sql(Query::DeleteForkRuns),
            params![wave_run_id, step_index as i64],
        )?;
        Ok(deleted as u32)
    }

    pub fn list_agents(&self) -> StoreResult<Vec<AgentRun>> {
        self.list_agent_history(None, None, None)
    }

    pub fn list_agent_history(
        &self,
        worktree: Option<&str>,
        repo: Option<&str>,
        limit: Option<u32>,
    ) -> StoreResult<Vec<AgentRun>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let query = Self::sql(list_agent_history_query(
            worktree.is_some(),
            repo.is_some(),
            limit.is_some(),
        ));
        let mut params_vec: Vec<Box<dyn ToSql>> = Vec::new();

        if let Some(worktree) = worktree {
            params_vec.push(Box::new(worktree.to_string()));
        }
        if let Some(repo) = repo {
            params_vec.push(Box::new(repo.to_string()));
        }
        if let Some(limit) = limit {
            params_vec.push(Box::new(limit as i64));
        }

        let mut stmt = conn.prepare(query)?;
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

    pub fn get_agent(&self, agent_id: &LfdId) -> StoreResult<Option<AgentRun>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(Self::sql(Query::GetAgentById))?;
        let run = stmt
            .query_row(params![agent_id], |row| Ok(map_agent_row(row)))
            .optional()?;
        run.transpose()
    }

    pub fn get_waiting_agent_for_wave(&self, wave_id: &LfdId) -> StoreResult<Option<AgentRun>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(Self::sql(Query::GetWaitingAgentForWave))?;
        let run = stmt
            .query_row(
                params![wave_id, AgentStatus::Waiting.as_i32() as i64],
                |row| Ok(map_agent_row(row)),
            )
            .optional()?;
        run.transpose()
    }

    pub fn start_agent(&self, agent: &AgentRun) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let started_at = agent
            .started_at
            .map(|dt| dt.unix_timestamp())
            .unwrap_or_else(now_unix);
        conn.execute(
            Self::sql(Query::InsertAgent),
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

    pub fn update_agent_status(
        &self,
        agent_id: &LfdId,
        status: i32,
        pid: Option<u32>,
        container_id: Option<&str>,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let updated = conn.execute(
            Self::sql(Query::UpdateAgentStatus),
            params![status as i64, pid.map(|v| v as i64), container_id, agent_id],
        )?;
        if updated == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    pub fn end_agent(&self, agent_id: &LfdId, status: i32, ended_at: i64) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let updated = conn.execute(
            Self::sql(Query::EndAgent),
            params![status as i64, ended_at, agent_id],
        )?;
        if updated == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    pub fn get_active_agents_for_wave(&self, wave_id: &LfdId) -> StoreResult<Vec<AgentRun>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(Self::sql(Query::GetActiveAgentsForWave))?;
        let rows = stmt.query_map(params![wave_id], |row| Ok(map_agent_row(row)))?;
        let mut agents = Vec::new();
        for row in rows {
            agents.push(row??);
        }
        Ok(agents)
    }

    pub fn end_active_agent_for_wave(
        &self,
        wave_id: &LfdId,
        status: i32,
        ended_at: i64,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let updated = conn.execute(
            Self::sql(Query::EndActiveAgentsForWave),
            params![status as i64, ended_at, wave_id.as_str()],
        )?;
        if updated == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    pub fn get_stuck_agents(&self, older_than_secs: u64) -> StoreResult<Vec<AgentRun>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let cutoff = now_unix() - older_than_secs as i64;
        let mut stmt = conn.prepare(Self::sql(Query::GetStuckAgents))?;
        let rows = stmt.query_map(params![cutoff], |row| Ok(map_agent_row(row)))?;
        let mut runs = Vec::new();
        for run in rows {
            runs.push(run??);
        }
        Ok(runs)
    }

    pub fn get_summary(&self, wave_id: &LfdId) -> StoreResult<Option<Summary>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(Self::sql(Query::GetSummaryByWave))?;
        let summary = stmt
            .query_row(params![wave_id], |row| Ok(map_summary_row(row)))
            .optional()?;
        summary.transpose()
    }

    pub fn upsert_summary(&self, summary: &Summary) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let created_at = summary
            .created_at
            .map(|dt| dt.unix_timestamp())
            .unwrap_or_else(now_unix);

        conn.execute(
            Self::sql(Query::UpsertSummary),
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

    pub fn list_chat_memory_blocks(&self, wave_id: &LfdId) -> StoreResult<Vec<ChatMemoryBlock>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(Self::sql(Query::ListChatMemoryBlocks))?;
        let rows = stmt.query_map(params![wave_id], |row| Ok(map_chat_memory_block_row(row)))?;
        let mut blocks = Vec::new();
        for row in rows {
            blocks.push(row??);
        }
        Ok(blocks)
    }

    pub fn upsert_chat_memory_block(&self, block: &ChatMemoryBlock) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let updated_at = block
            .updated_at
            .map(|dt| dt.unix_timestamp())
            .unwrap_or_else(now_unix);
        conn.execute(
            Self::sql(Query::UpsertChatMemoryBlock),
            params![
                block.wave_id,
                block.name,
                block.content,
                block.position as i64,
                updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn delete_chat_memory_block(&self, wave_id: &LfdId, name: &str) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            Self::sql(Query::DeleteChatMemoryBlock),
            params![wave_id, name],
        )?;
        Ok(())
    }

    pub fn list_chat_messages(&self, wave_id: &LfdId) -> StoreResult<Vec<ChatMessage>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, wave_id, role, content, created_at
             FROM chat_messages
             WHERE wave_id = ?1
             ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map(params![wave_id], |row| Ok(map_chat_message_row(row)))?;
        let mut messages = Vec::new();
        for row in rows {
            messages.push(row??);
        }
        Ok(messages)
    }

    pub fn create_chat_message(&self, message: &ChatMessage) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO chat_messages (id, wave_id, role, content, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                message.id,
                message.wave_id,
                message.role,
                message.content,
                message.created_at.unix_timestamp(),
            ],
        )?;
        Ok(())
    }
}
