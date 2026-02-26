use std::time::Duration;

use deadpool_postgres::{Manager, Pool};
use tokio_postgres::types::ToSql;
use tokio_postgres::NoTls;

use crate::lfd::id::LfdId;
use crate::lfd::sessions::types::{
    PersistedSessionEvent, Session, SessionConfig, SessionEvent, SessionStatus,
};
use crate::lfd::store::catalog::{
    list_agent_history_query, list_stimuli_query, list_wave_runs_query, list_waves_query, sql,
    Query, SqlDialect,
};
use crate::lfd::store::rows::{
    map_agent_row, map_chat_memory_block_row, map_chat_message_row, map_chord_row,
    map_fork_run_row, map_live_pr_state_row, map_pending_activation_row, map_stimulus_row,
    map_summary_row, map_wave_row, map_wave_run_row, now_unix, serialize_pr,
};
use crate::lfd::store::{ForkRun, ForkRunStatus, StoreError, StoreResult};
use crate::lfd::types::{
    AgentRun, AgentStatus, ChatMemoryBlock, ChatMessage, Chord, LivePullRequestState,
    PendingActivation, QueueBlock, QueueBlockReason, QueueMergeEvent, Stimulus, Summary, Wave,
    WaveRun, WaveRunStatus, WaveStatus,
};

const RETRY_DELAYS: [Duration; 3] = [
    Duration::from_millis(100),
    Duration::from_millis(500),
    Duration::from_secs(2),
];

#[derive(Debug)]
pub struct PostgresStore {
    pool: Pool,
}

impl PostgresStore {
    fn sql(query: Query) -> &'static str {
        sql(query, SqlDialect::Postgres)
    }

    pub async fn connect_async(database_url: &str) -> StoreResult<Self> {
        let pool = build_pool(database_url)?;
        let version = super::migrations::latest_version_postgres_pool(&pool).await?;
        if version.is_empty() {
            return Err(StoreError::InvalidData(
                "postgres schema missing; run `lfd migrate`".to_string(),
            ));
        }
        Ok(Self { pool })
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

    async fn with_client<T, F, Fut>(&self, func: F) -> StoreResult<T>
    where
        F: FnOnce(deadpool_postgres::Client) -> Fut,
        Fut: std::future::Future<Output = StoreResult<T>>,
    {
        let client = get_client_with_retry(&self.pool).await?;
        func(client).await
    }

    async fn read_waves(&self, repo: Option<&str>) -> StoreResult<Vec<Wave>> {
        self.with_client(|client| async move {
            let rows = if let Some(repo) = repo {
                client
                    .query(Self::sql(list_waves_query(true)), &[&repo])
                    .await?
            } else {
                client
                    .query(Self::sql(list_waves_query(false)), &[])
                    .await?
            };
            rows.iter().map(map_wave_row).collect()
        })
        .await
    }

    async fn upsert_wave(&self, wave: &Wave) -> StoreResult<()> {
        self.with_client(|client| async move {
            let direction_json = serde_json::to_string(wave.direction())?;
            let area_json = serde_json::to_string(wave.area())?;
            let paused: i32 = if wave.status() == WaveStatus::Paused {
                1
            } else {
                0
            };
            let created_at = wave.created_at().map(|dt| dt.unix_timestamp()).unwrap_or(0);

            client
                .execute(
                    Self::sql(Query::UpsertWave),
                    &[
                        &wave.id().as_str(),
                        &wave.name().as_str(),
                        &wave.repo().as_str(),
                        &wave.flow().as_str(),
                        &direction_json.as_str(),
                        &area_json.as_str(),
                        &paused,
                        &(wave.status().as_i32()),
                        &(wave.iteration() as i32),
                        &created_at,
                    ],
                )
                .await?;
            Ok(())
        })
        .await
    }

    async fn resource_exists(&self, query: &'static str, id: &LfdId) -> StoreResult<bool> {
        self.with_client(|client| async move {
            let row = client.query_opt(query, &[&id]).await?;
            Ok(row.is_some())
        })
        .await
    }

    async fn chord_exists(&self, chord_id: &LfdId) -> StoreResult<bool> {
        self.resource_exists("SELECT 1 FROM chords WHERE id = $1", chord_id)
            .await
    }

    async fn wave_exists(&self, wave_id: &LfdId) -> StoreResult<bool> {
        self.resource_exists("SELECT 1 FROM waves WHERE id = $1", wave_id)
            .await
    }

    async fn ensure_chord_exists(&self, chord_id: &LfdId) -> StoreResult<()> {
        if self.chord_exists(chord_id).await? {
            return Ok(());
        }
        Err(StoreError::NotFound)
    }

    async fn ensure_wave_exists(&self, wave_id: &LfdId) -> StoreResult<()> {
        if self.wave_exists(wave_id).await? {
            return Ok(());
        }
        Err(StoreError::NotFound)
    }

    async fn ensure_chord_member_resources_exist(
        &self,
        chord_id: &LfdId,
        wave_id: &LfdId,
    ) -> StoreResult<()> {
        self.ensure_chord_exists(chord_id).await?;
        self.ensure_wave_exists(wave_id).await
    }

    fn map_session_row(row: &tokio_postgres::Row) -> StoreResult<Session> {
        let config: SessionConfig = serde_json::from_str(row.get::<_, &str>(5))?;
        Ok(Session {
            id: row.get(0),
            harness: row.get(1),
            status: SessionStatus::from_i32(row.get::<_, i32>(2)),
            wave_run_id: row.get(3),
            provider_session_id: row.get(4),
            config,
            created_at: crate::lfd::store::rows::unix_to_datetime(row.get(6)),
            ended_at: row
                .get::<_, Option<i64>>(7)
                .map(crate::lfd::store::rows::unix_to_datetime),
        })
    }
}

impl PostgresStore {
    pub async fn health_check(&self) -> StoreResult<()> {
        self.with_client(|client| async move {
            client.execute(Self::sql(Query::HealthCheck), &[]).await?;
            Ok(())
        })
        .await
    }

    pub async fn schema_version(&self) -> StoreResult<String> {
        super::migrations::latest_version_postgres_pool(&self.pool).await
    }

    pub async fn create_session(&self, session: &Session) -> StoreResult<()> {
        self.with_client(|client| async move {
            client
                .execute(
                    "INSERT INTO sessions (id, harness, status, wave_run_id, provider_session_id, config, created_at, ended_at)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                    &[
                        &session.id,
                        &session.harness,
                        &session.status.as_i32(),
                        &session.wave_run_id,
                        &session.provider_session_id,
                        &serde_json::to_string(&session.config)?,
                        &session.created_at.unix_timestamp(),
                        &session.ended_at.map(|dt| dt.unix_timestamp()),
                    ],
                )
                .await?;
            Ok(())
        })
        .await
    }

    pub async fn get_session(&self, session_id: &LfdId) -> StoreResult<Option<Session>> {
        self.with_client(|client| async move {
            let row = client
                .query_opt(
                    "SELECT id, harness, status, wave_run_id, provider_session_id, config, created_at, ended_at
                     FROM sessions
                     WHERE id = $1",
                    &[&session_id],
                )
                .await?;
            row.as_ref().map(Self::map_session_row).transpose()
        })
        .await
    }

    pub async fn get_active_session_for_wave_run(
        &self,
        wave_run_id: &str,
    ) -> StoreResult<Option<Session>> {
        let wave_run_id = wave_run_id.to_string();
        self.with_client(|client| async move {
            let row = client
                .query_opt(
                    "SELECT id, harness, status, wave_run_id, provider_session_id, config, created_at, ended_at
                     FROM sessions
                     WHERE wave_run_id = $1 AND status = ANY($2)
                     ORDER BY created_at DESC
                     LIMIT 1",
                    &[
                        &wave_run_id,
                        &&[
                            SessionStatus::Starting.as_i32(),
                            SessionStatus::Active.as_i32(),
                            SessionStatus::Ending.as_i32(),
                        ][..],
                    ],
                )
                .await?;
            row.as_ref().map(Self::map_session_row).transpose()
        })
        .await
    }

    pub async fn update_provider_session_id(
        &self,
        session_id: &LfdId,
        provider_session_id: &str,
    ) -> StoreResult<()> {
        let provider_session_id = provider_session_id.to_string();
        self.with_client(|client| async move {
            let updated = client
                .execute(
                    "UPDATE sessions SET provider_session_id = $2 WHERE id = $1",
                    &[&session_id, &provider_session_id],
                )
                .await?;
            if updated == 0 {
                return Err(StoreError::NotFound);
            }
            Ok(())
        })
        .await
    }

    pub async fn update_session_status(
        &self,
        session_id: &LfdId,
        status: SessionStatus,
        ended_at: Option<i64>,
    ) -> StoreResult<()> {
        self.with_client(|client| async move {
            let updated = client
                .execute(
                    "UPDATE sessions
                     SET status = $2, ended_at = COALESCE($3, ended_at)
                     WHERE id = $1",
                    &[&session_id, &status.as_i32(), &ended_at],
                )
                .await?;
            if updated == 0 {
                return Err(StoreError::NotFound);
            }
            Ok(())
        })
        .await
    }

    pub async fn append_session_event(
        &self,
        session_id: &LfdId,
        seq: i64,
        event: &SessionEvent,
        created_at: i64,
    ) -> StoreResult<()> {
        self.with_client(|client| async move {
            client
                .execute(
                    "INSERT INTO session_events (session_id, seq, event_type, data, created_at)
                     VALUES ($1, $2, $3, $4, $5)",
                    &[
                        &session_id,
                        &seq,
                        &event.event_type(),
                        &serde_json::to_string(event)?,
                        &created_at,
                    ],
                )
                .await?;
            Ok(())
        })
        .await
    }

    pub async fn list_session_events(
        &self,
        session_id: &LfdId,
        after_seq: Option<i64>,
    ) -> StoreResult<Vec<PersistedSessionEvent>> {
        self.with_client(|client| async move {
            let rows = if let Some(after_seq) = after_seq {
                client
                    .query(
                        "SELECT session_id, seq, data, created_at
                         FROM session_events
                         WHERE session_id = $1 AND seq > $2
                         ORDER BY seq ASC",
                        &[&session_id, &after_seq],
                    )
                    .await?
            } else {
                client
                    .query(
                        "SELECT session_id, seq, data, created_at
                         FROM session_events
                         WHERE session_id = $1
                         ORDER BY seq ASC",
                        &[&session_id],
                    )
                    .await?
            };

            rows.iter()
                .map(|row| {
                    let event: SessionEvent = serde_json::from_str(row.get::<_, &str>(2))?;
                    Ok(PersistedSessionEvent {
                        session_id: row.get(0),
                        seq: row.get(1),
                        event,
                        created_at: crate::lfd::store::rows::unix_to_datetime(row.get(3)),
                    })
                })
                .collect()
        })
        .await
    }

    pub async fn list_sessions_by_statuses(
        &self,
        statuses: &[SessionStatus],
    ) -> StoreResult<Vec<Session>> {
        let status_ints: Vec<i32> = statuses.iter().map(|s| s.as_i32()).collect();
        self.with_client(|client| async move {
            let rows = client
                .query(
                    "SELECT id, harness, status, wave_run_id, provider_session_id, config, created_at, ended_at
                     FROM sessions WHERE status = ANY($1)
                     ORDER BY created_at ASC",
                    &[&status_ints],
                )
                .await?;
            rows.iter().map(Self::map_session_row).collect()
        })
        .await
    }

    pub async fn list_waves(&self, repo: Option<&str>) -> StoreResult<Vec<Wave>> {
        self.read_waves(repo).await
    }

    pub async fn get_wave(&self, wave_id: &LfdId) -> StoreResult<Option<Wave>> {
        self.with_client(|client| async move {
            let row = client
                .query_opt(Self::sql(Query::GetWaveById), &[&wave_id])
                .await?;
            row.as_ref().map(map_wave_row).transpose()
        })
        .await
    }

    pub async fn get_wave_by_name(&self, name: &str) -> StoreResult<Option<Wave>> {
        let name = name.to_string();
        self.with_client(|client| async move {
            let row = client
                .query_opt(Self::sql(Query::GetWaveByName), &[&name])
                .await?;
            row.as_ref().map(map_wave_row).transpose()
        })
        .await
    }

    pub async fn create_wave(&self, wave: &Wave) -> StoreResult<()> {
        self.upsert_wave(wave).await
    }

    pub async fn update_wave(&self, wave: &Wave) -> StoreResult<()> {
        self.upsert_wave(wave).await
    }

    pub async fn delete_wave(&self, wave_id: &LfdId) -> StoreResult<()> {
        self.with_client(|client| async move {
            client
                .execute(Self::sql(Query::DeleteWaveById), &[&wave_id])
                .await?;
            Ok(())
        })
        .await
    }

    pub async fn create_chord(&self, name: &str) -> StoreResult<Chord> {
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
        let is_default = if chord.is_default { 1i32 } else { 0i32 };

        self.with_client(|client| async move {
            client
                .execute(
                    "INSERT INTO chords (id, name, is_default, created_at)
                     VALUES ($1, $2, $3, $4)",
                    &[&chord.id, &chord.name, &is_default, &created_at],
                )
                .await?;
            Ok(chord)
        })
        .await
    }

    pub async fn get_chord(&self, chord_id: &LfdId) -> StoreResult<Option<Chord>> {
        self.with_client(|client| async move {
            let row = client
                .query_opt(
                    "SELECT id, name, is_default, created_at
                     FROM chords
                     WHERE id = $1",
                    &[&chord_id],
                )
                .await?;
            row.as_ref().map(map_chord_row).transpose()
        })
        .await
    }

    pub async fn list_chords(&self) -> StoreResult<Vec<Chord>> {
        self.with_client(|client| async move {
            let rows = client
                .query(
                    "SELECT id, name, is_default, created_at
                     FROM chords
                     ORDER BY created_at, name",
                    &[],
                )
                .await?;
            rows.iter().map(map_chord_row).collect()
        })
        .await
    }

    pub async fn delete_chord(&self, chord_id: &LfdId) -> StoreResult<()> {
        self.with_client(|client| async move {
            let deleted = client
                .execute("DELETE FROM chords WHERE id = $1", &[&chord_id])
                .await?;
            if deleted == 0 {
                return Err(StoreError::NotFound);
            }
            Ok(())
        })
        .await
    }

    pub async fn add_chord_member(&self, chord_id: &LfdId, wave_id: &LfdId) -> StoreResult<()> {
        self.ensure_chord_member_resources_exist(chord_id, wave_id)
            .await?;

        self.with_client(|client| async move {
            client
                .execute(
                    "INSERT INTO chord_members (chord_id, wave_id)
                     VALUES ($1, $2)
                     ON CONFLICT (chord_id, wave_id) DO NOTHING",
                    &[&chord_id, &wave_id],
                )
                .await?;
            Ok(())
        })
        .await
    }

    pub async fn remove_chord_member(&self, chord_id: &LfdId, wave_id: &LfdId) -> StoreResult<()> {
        self.ensure_chord_member_resources_exist(chord_id, wave_id)
            .await?;

        self.with_client(|client| async move {
            client
                .execute(
                    "DELETE FROM chord_members WHERE chord_id = $1 AND wave_id = $2",
                    &[&chord_id, &wave_id],
                )
                .await?;
            Ok(())
        })
        .await
    }

    pub async fn list_chord_members(&self, chord_id: &LfdId) -> StoreResult<Vec<Wave>> {
        self.ensure_chord_exists(chord_id).await?;
        self.with_client(|client| async move {
            let rows = client
                .query(
                    "SELECT w.id, w.name, w.repo, w.flow, w.direction, w.area, w.paused, w.status, w.iteration, w.created_at
                     FROM waves w
                     INNER JOIN chord_members cm ON cm.wave_id = w.id
                     WHERE cm.chord_id = $1
                     ORDER BY w.created_at, w.name",
                    &[&chord_id],
                )
                .await?;
            rows.iter().map(map_wave_row).collect()
        })
        .await
    }

    pub async fn list_chords_for_wave(&self, wave_id: &LfdId) -> StoreResult<Vec<Chord>> {
        self.ensure_wave_exists(wave_id).await?;
        self.with_client(|client| async move {
            let rows = client
                .query(
                    "SELECT c.id, c.name, c.is_default, c.created_at
                     FROM chords c
                     INNER JOIN chord_members cm ON cm.chord_id = c.id
                     WHERE cm.wave_id = $1
                     ORDER BY c.created_at, c.name",
                    &[&wave_id],
                )
                .await?;
            rows.iter().map(map_chord_row).collect()
        })
        .await
    }

    pub async fn list_wave_runs(
        &self,
        wave_id: Option<&LfdId>,
        limit: Option<u32>,
    ) -> StoreResult<Vec<WaveRun>> {
        self.with_client(|client| async move {
            let query = Self::sql(list_wave_runs_query(wave_id.is_some(), limit.is_some()));
            let mut params: Vec<Box<dyn ToSql + Sync + Send>> = Vec::new();
            if let Some(wave_id) = wave_id {
                params.push(Box::new(wave_id.clone()));
            }
            if let Some(limit) = limit {
                params.push(Box::new(limit as i64));
            }

            let params_ref: Vec<&(dyn ToSql + Sync)> = params
                .iter()
                .map(|v| v.as_ref() as &(dyn ToSql + Sync))
                .collect();
            let rows = client.query(query, &params_ref).await?;
            rows.iter().map(map_wave_run_row).collect()
        })
        .await
    }

    pub async fn get_wave_run(&self, wave_run_id: &LfdId) -> StoreResult<Option<WaveRun>> {
        self.with_client(|client| async move {
            let row = client
                .query_opt(Self::sql(Query::GetWaveRunById), &[&wave_run_id])
                .await?;
            row.as_ref().map(map_wave_run_row).transpose()
        })
        .await
    }

    pub async fn get_active_wave_run(&self, wave_id: &LfdId) -> StoreResult<Option<WaveRun>> {
        self.with_client(|client| async move {
            let statuses = [
                WaveRunStatus::Pending.as_i32(),
                WaveRunStatus::Running.as_i32(),
                WaveRunStatus::Waiting.as_i32(),
            ];
            let row = client
                .query_opt(
                    Self::sql(Query::GetActiveWaveRun),
                    &[
                        &wave_id,
                        &&statuses[..],
                        &crate::lfd::types::WaveRunKind::Main.as_i32(),
                    ],
                )
                .await?;
            row.as_ref().map(map_wave_run_row).transpose()
        })
        .await
    }

    pub async fn get_latest_wave_run(&self, wave_id: &LfdId) -> StoreResult<Option<WaveRun>> {
        self.with_client(|client| async move {
            let row = client
                .query_opt(
                    Self::sql(Query::GetLatestWaveRun),
                    &[&wave_id, &crate::lfd::types::WaveRunKind::Main.as_i32()],
                )
                .await?;
            row.as_ref().map(map_wave_run_row).transpose()
        })
        .await
    }

    pub async fn create_wave_run(&self, run: &WaveRun) -> StoreResult<()> {
        self.with_client(|client| async move {
            let started_at = run
                .started_at
                .map(|dt| dt.unix_timestamp())
                .unwrap_or_else(now_unix);
            let ended_at = run.ended_at.map(|dt| dt.unix_timestamp());
            let flow_parents_json = serde_json::to_string(&run.flow_parents)?;
            client
                .execute(
                    Self::sql(Query::InsertWaveRun),
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
        .await
    }

    pub async fn update_wave_run(&self, run: &WaveRun) -> StoreResult<()> {
        self.with_client(|client| async move {
            let flow_parents_json = serde_json::to_string(&run.flow_parents)?;
            let updated = client
                .execute(
                    Self::sql(Query::UpdateWaveRun),
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
        .await
    }

    pub async fn list_stack_runs(&self, wave_id: &LfdId) -> StoreResult<Vec<WaveRun>> {
        self.with_client(|client| async move {
            let rows = client
                .query(
                    Self::sql(Query::ListStackRuns),
                    &[&wave_id, &crate::lfd::types::WaveRunKind::Main.as_i32()],
                )
                .await?;
            rows.iter().map(map_wave_run_row).collect()
        })
        .await
    }

    pub async fn get_live_pr_state(
        &self,
        repo_id: &str,
        pr_number: u32,
    ) -> StoreResult<Option<LivePullRequestState>> {
        let repo_id = repo_id.to_string();
        self.with_client(|client| async move {
            let row = client
                .query_opt(
                    Self::sql(Query::GetLivePrState),
                    &[&repo_id, &(pr_number as i64)],
                )
                .await?;
            row.as_ref().map(map_live_pr_state_row).transpose()
        })
        .await
    }

    pub async fn upsert_live_pr_state(&self, state: &LivePullRequestState) -> StoreResult<()> {
        let state = state.clone();
        self.with_client(|client| async move {
            client
                .execute(
                    Self::sql(Query::UpsertLivePrState),
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
        .await
    }

    pub async fn list_queue_blocks(&self, wave_id: &LfdId) -> StoreResult<Vec<QueueBlock>> {
        let wave_id = wave_id.clone();
        self.with_client(|client| async move {
            let rows = client
                .query(
                    "SELECT wave_id, run_id, reason, attempted_at, conflict_files, error
                     FROM wave_queue_blocks
                     WHERE wave_id = $1
                     ORDER BY attempted_at DESC",
                    &[&wave_id],
                )
                .await?;
            rows.into_iter()
                .map(|row| {
                    let conflict_files = row
                        .try_get::<_, String>(4)
                        .ok()
                        .and_then(|value| serde_json::from_str::<Vec<String>>(&value).ok())
                        .unwrap_or_default();
                    let reason_raw: String = row.try_get(2)?;
                    let reason = reason_raw
                        .parse::<QueueBlockReason>()
                        .map_err(StoreError::InvalidData)?;
                    Ok(QueueBlock {
                        wave_id: LfdId::from_raw(row.try_get::<_, String>(0)?),
                        run_id: LfdId::from_raw(row.try_get::<_, String>(1)?),
                        reason,
                        attempted_at: crate::lfd::store::rows::unix_to_datetime(row.try_get(3)?),
                        conflict_files,
                        error: row.try_get(5)?,
                    })
                })
                .collect()
        })
        .await
    }

    pub async fn upsert_queue_block(&self, block: &QueueBlock) -> StoreResult<()> {
        let block = block.clone();
        self.with_client(|client| async move {
            client
                .execute(
                    "INSERT INTO wave_queue_blocks (wave_id, run_id, reason, attempted_at, conflict_files, error)
                     VALUES ($1, $2, $3, $4, $5, $6)
                     ON CONFLICT(wave_id, run_id) DO UPDATE SET
                        reason = excluded.reason,
                        attempted_at = excluded.attempted_at,
                        conflict_files = excluded.conflict_files,
                        error = excluded.error",
                    &[
                        &block.wave_id,
                        &block.run_id,
                        &block.reason.as_str(),
                        &block.attempted_at.unix_timestamp(),
                        &serde_json::to_string(&block.conflict_files)?,
                        &block.error,
                    ],
                )
                .await?;
            Ok(())
        })
        .await
    }

    pub async fn delete_queue_block(&self, wave_id: &LfdId, run_id: &LfdId) -> StoreResult<u32> {
        let wave_id = wave_id.clone();
        let run_id = run_id.clone();
        self.with_client(|client| async move {
            let deleted = client
                .execute(
                    "DELETE FROM wave_queue_blocks WHERE wave_id = $1 AND run_id = $2",
                    &[&wave_id, &run_id],
                )
                .await?;
            Ok(deleted as u32)
        })
        .await
    }

    pub async fn record_merge_event(&self, event: &QueueMergeEvent) -> StoreResult<bool> {
        let event = event.clone();
        self.with_client(|client| async move {
            let inserted = client
                .execute(
                    "INSERT INTO wave_pr_merge_events (wave_id, pr_number, merged_at, processed_at)
                     VALUES ($1, $2, $3, $4)
                     ON CONFLICT(wave_id, pr_number, merged_at) DO NOTHING",
                    &[
                        &event.wave_id,
                        &(event.pr_number as i64),
                        &event.merged_at.unix_timestamp(),
                        &event.processed_at.unix_timestamp(),
                    ],
                )
                .await?;
            Ok(inserted > 0)
        })
        .await
    }

    pub async fn fail_orphaned_runs(&self) -> StoreResult<u32> {
        self.with_client(|client| async move {
            let statuses = [
                WaveRunStatus::Pending.as_i32(),
                WaveRunStatus::Running.as_i32(),
                WaveRunStatus::Waiting.as_i32(),
            ];
            let updated = client
                .execute(
                    Self::sql(Query::FailOrphanedRuns),
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
        .await
    }

    pub async fn list_stimuli(&self, wave_id: Option<&LfdId>) -> StoreResult<Vec<Stimulus>> {
        self.with_client(|client| async move {
            let rows = if let Some(wave_id) = wave_id {
                client
                    .query(Self::sql(list_stimuli_query(true)), &[&wave_id])
                    .await?
            } else {
                client
                    .query(Self::sql(list_stimuli_query(false)), &[])
                    .await?
            };
            rows.iter().map(map_stimulus_row).collect()
        })
        .await
    }

    pub async fn list_stimuli_by_kind(&self, kind: i32) -> StoreResult<Vec<Stimulus>> {
        self.with_client(|client| async move {
            let rows = client
                .query(Self::sql(Query::ListStimuliByKind), &[&kind])
                .await?;
            rows.iter().map(map_stimulus_row).collect()
        })
        .await
    }

    pub async fn get_stimulus(&self, stimulus_id: &LfdId) -> StoreResult<Option<Stimulus>> {
        self.with_client(|client| async move {
            let row = client
                .query_opt(Self::sql(Query::GetStimulusById), &[&stimulus_id])
                .await?;
            row.as_ref().map(map_stimulus_row).transpose()
        })
        .await
    }

    pub async fn create_stimulus(&self, stimulus: &Stimulus) -> StoreResult<()> {
        self.with_client(|client| async move {
            let created_at = stimulus
                .created_at
                .map(|dt| dt.unix_timestamp())
                .unwrap_or_else(now_unix);
            let enabled: i32 = if stimulus.enabled { 1 } else { 0 };

            client
                .execute(
                    Self::sql(Query::InsertStimulus),
                    &[
                        &stimulus.id,
                        &stimulus.wave_id,
                        &stimulus.kind.as_i32(),
                        &stimulus.cron,
                        &stimulus.last_main_sha,
                        &stimulus.last_triggered_at,
                        &created_at,
                        &enabled,
                        &stimulus.source_wave_id,
                    ],
                )
                .await?;
            Ok(())
        })
        .await
    }

    pub async fn update_stimulus(&self, stimulus: &Stimulus) -> StoreResult<()> {
        self.with_client(|client| async move {
            let enabled: i32 = if stimulus.enabled { 1 } else { 0 };
            let updated = client
                .execute(
                    Self::sql(Query::UpdateStimulus),
                    &[
                        &stimulus.kind.as_i32(),
                        &stimulus.cron,
                        &stimulus.last_main_sha,
                        &stimulus.last_triggered_at,
                        &enabled,
                        &stimulus.source_wave_id,
                        &stimulus.id,
                    ],
                )
                .await?;
            if updated == 0 {
                return Err(StoreError::NotFound);
            }
            Ok(())
        })
        .await
    }

    pub async fn delete_stimulus(&self, stimulus_id: &LfdId) -> StoreResult<()> {
        self.with_client(|client| async move {
            client
                .execute(Self::sql(Query::DeleteStimulusById), &[&stimulus_id])
                .await?;
            Ok(())
        })
        .await
    }

    pub async fn list_pending_activations(
        &self,
        wave_id: &LfdId,
    ) -> StoreResult<Vec<PendingActivation>> {
        self.with_client(|client| async move {
            let rows = client
                .query(Self::sql(Query::ListPendingActivationsByWave), &[&wave_id])
                .await?;
            rows.iter().map(map_pending_activation_row).collect()
        })
        .await
    }

    pub async fn create_pending_activation(
        &self,
        activation: &PendingActivation,
    ) -> StoreResult<()> {
        self.with_client(|client| async move {
            client
                .execute(
                    Self::sql(Query::InsertPendingActivation),
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
        .await
    }

    pub async fn update_pending_activation(
        &self,
        activation: &PendingActivation,
    ) -> StoreResult<()> {
        self.with_client(|client| async move {
            let updated = client
                .execute(
                    Self::sql(Query::UpdatePendingActivation),
                    &[&activation.from_sha, &activation.to_sha, &activation.id],
                )
                .await?;
            if updated == 0 {
                return Err(StoreError::NotFound);
            }
            Ok(())
        })
        .await
    }

    pub async fn delete_pending_activations(&self, wave_id: &LfdId) -> StoreResult<u32> {
        self.with_client(|client| async move {
            let deleted = client
                .execute(
                    Self::sql(Query::DeletePendingActivationsByWave),
                    &[&wave_id],
                )
                .await?;
            Ok(deleted as u32)
        })
        .await
    }

    pub async fn get_pending_for_stimulus(
        &self,
        wave_id: &LfdId,
        stimulus_id: &LfdId,
    ) -> StoreResult<Option<PendingActivation>> {
        self.with_client(|client| async move {
            let row = client
                .query_opt(
                    Self::sql(Query::GetPendingActivationForStimulus),
                    &[&wave_id, &stimulus_id],
                )
                .await?;
            row.as_ref().map(map_pending_activation_row).transpose()
        })
        .await
    }

    pub async fn list_fork_runs(
        &self,
        wave_run_id: &LfdId,
        step_index: u32,
    ) -> StoreResult<Vec<ForkRun>> {
        self.with_client(|client| async move {
            let rows = client
                .query(
                    Self::sql(Query::ListForkRuns),
                    &[&wave_run_id, &(step_index as i32)],
                )
                .await?;
            rows.iter().map(map_fork_run_row).collect()
        })
        .await
    }

    pub async fn list_orphaned_fork_runs(&self) -> StoreResult<Vec<ForkRun>> {
        self.with_client(|client| async move {
            let rows = client
                .query(
                    "SELECT fr.id, fr.wave_run_id, fr.step_index, fr.branch_index, fr.status, fr.worktree
                     FROM fork_runs fr
                     LEFT JOIN wave_runs wr ON wr.id = fr.wave_run_id
                     WHERE fr.status IN ($1, $2)
                       AND (
                         wr.id IS NULL
                         OR wr.status NOT IN ($3, $4, $5)
                         OR fr.step_index <> wr.step_index
                       )
                     ORDER BY fr.wave_run_id ASC, fr.step_index ASC, fr.branch_index ASC",
                    &[
                        &(ForkRunStatus::Pending as i32),
                        &(ForkRunStatus::Running as i32),
                        &WaveRunStatus::Pending.as_i32(),
                        &WaveRunStatus::Running.as_i32(),
                        &WaveRunStatus::Waiting.as_i32(),
                    ],
                )
                .await?;
            rows.iter().map(map_fork_run_row).collect()
        })
        .await
    }

    pub async fn upsert_fork_run(&self, fork_run: &ForkRun) -> StoreResult<()> {
        self.with_client(|client| async move {
            client
                .execute(
                    Self::sql(Query::UpsertForkRun),
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
        .await
    }

    pub async fn delete_fork_runs(&self, wave_run_id: &LfdId, step_index: u32) -> StoreResult<u32> {
        self.with_client(|client| async move {
            let deleted = client
                .execute(
                    Self::sql(Query::DeleteForkRuns),
                    &[&wave_run_id, &(step_index as i32)],
                )
                .await?;
            Ok(deleted as u32)
        })
        .await
    }

    pub async fn list_agents(&self) -> StoreResult<Vec<AgentRun>> {
        self.list_agent_history(None, None, None).await
    }

    pub async fn list_agent_history(
        &self,
        worktree: Option<&str>,
        repo: Option<&str>,
        limit: Option<u32>,
    ) -> StoreResult<Vec<AgentRun>> {
        self.with_client(|client| async move {
            let query = Self::sql(list_agent_history_query(
                worktree.is_some(),
                repo.is_some(),
                limit.is_some(),
            ));
            let mut params: Vec<Box<dyn ToSql + Sync + Send>> = Vec::new();

            if let Some(worktree) = worktree {
                params.push(Box::new(worktree.to_string()));
            }
            if let Some(repo) = repo {
                params.push(Box::new(repo.to_string()));
            }
            if let Some(limit) = limit {
                params.push(Box::new(limit as i64));
            }

            let params_ref: Vec<&(dyn ToSql + Sync)> = params
                .iter()
                .map(|v| v.as_ref() as &(dyn ToSql + Sync))
                .collect();
            let rows = client.query(query, &params_ref).await?;
            rows.iter().map(map_agent_row).collect()
        })
        .await
    }

    pub async fn get_agent(&self, agent_id: &LfdId) -> StoreResult<Option<AgentRun>> {
        self.with_client(|client| async move {
            let row = client
                .query_opt(Self::sql(Query::GetAgentById), &[&agent_id])
                .await?;
            row.as_ref().map(map_agent_row).transpose()
        })
        .await
    }

    pub async fn get_waiting_agent_for_wave(
        &self,
        wave_id: &LfdId,
    ) -> StoreResult<Option<AgentRun>> {
        self.with_client(|client| async move {
            let row = client
                .query_opt(
                    Self::sql(Query::GetWaitingAgentForWave),
                    &[&wave_id, &AgentStatus::Waiting.as_i32()],
                )
                .await?;
            row.as_ref().map(map_agent_row).transpose()
        })
        .await
    }

    pub async fn start_agent(&self, agent: &AgentRun) -> StoreResult<()> {
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
                    Self::sql(Query::InsertAgent),
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
        .await
    }

    pub async fn update_agent_status(
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
                    Self::sql(Query::UpdateAgentStatus),
                    &[&status, &pid, &container_id, &agent_id],
                )
                .await?;
            if updated == 0 {
                return Err(StoreError::NotFound);
            }
            Ok(())
        })
        .await
    }

    pub async fn end_agent(&self, agent_id: &LfdId, status: i32, ended_at: i64) -> StoreResult<()> {
        self.with_client(|client| async move {
            let updated = client
                .execute(Self::sql(Query::EndAgent), &[&status, &ended_at, &agent_id])
                .await?;
            if updated == 0 {
                return Err(StoreError::NotFound);
            }
            Ok(())
        })
        .await
    }

    pub async fn get_active_agents_for_wave(&self, wave_id: &LfdId) -> StoreResult<Vec<AgentRun>> {
        self.with_client(|client| async move {
            let rows = client
                .query(Self::sql(Query::GetActiveAgentsForWave), &[&wave_id])
                .await?;
            rows.iter().map(map_agent_row).collect()
        })
        .await
    }

    pub async fn end_active_agent_for_wave(
        &self,
        wave_id: &LfdId,
        status: i32,
        ended_at: i64,
    ) -> StoreResult<()> {
        self.with_client(|client| async move {
            let updated = client
                .execute(
                    Self::sql(Query::EndActiveAgentsForWave),
                    &[&status, &ended_at, &wave_id.as_str()],
                )
                .await?;
            if updated == 0 {
                return Err(StoreError::NotFound);
            }
            Ok(())
        })
        .await
    }

    pub async fn get_stuck_agents(&self, older_than_secs: u64) -> StoreResult<Vec<AgentRun>> {
        self.with_client(|client| async move {
            let cutoff = now_unix() - older_than_secs as i64;
            let rows = client
                .query(Self::sql(Query::GetStuckAgents), &[&cutoff])
                .await?;
            rows.iter().map(map_agent_row).collect()
        })
        .await
    }

    pub async fn get_summary(&self, wave_id: &LfdId) -> StoreResult<Option<Summary>> {
        self.with_client(|client| async move {
            let row = client
                .query_opt(Self::sql(Query::GetSummaryByWave), &[&wave_id])
                .await?;
            row.as_ref().map(map_summary_row).transpose()
        })
        .await
    }

    pub async fn upsert_summary(&self, summary: &Summary) -> StoreResult<()> {
        self.with_client(|client| async move {
            let created_at = summary
                .created_at
                .map(|dt| dt.unix_timestamp())
                .unwrap_or_else(now_unix);

            client
                .execute(
                    Self::sql(Query::UpsertSummary),
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
        .await
    }

    pub async fn list_chat_memory_blocks(
        &self,
        wave_id: &LfdId,
    ) -> StoreResult<Vec<ChatMemoryBlock>> {
        self.with_client(|client| async move {
            let rows = client
                .query(Self::sql(Query::ListChatMemoryBlocks), &[&wave_id])
                .await?;
            rows.iter().map(map_chat_memory_block_row).collect()
        })
        .await
    }

    pub async fn upsert_chat_memory_block(&self, block: &ChatMemoryBlock) -> StoreResult<()> {
        self.with_client(|client| async move {
            let updated_at = block
                .updated_at
                .map(|dt| dt.unix_timestamp())
                .unwrap_or_else(now_unix);
            client
                .execute(
                    Self::sql(Query::UpsertChatMemoryBlock),
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
        .await
    }

    pub async fn delete_chat_memory_block(&self, wave_id: &LfdId, name: &str) -> StoreResult<()> {
        let name = name.to_string();
        self.with_client(|client| async move {
            client
                .execute(Self::sql(Query::DeleteChatMemoryBlock), &[&wave_id, &name])
                .await?;
            Ok(())
        })
        .await
    }

    pub async fn list_chat_messages(&self, wave_id: &LfdId) -> StoreResult<Vec<ChatMessage>> {
        self.with_client(|client| async move {
            let rows = client
                .query(
                    "SELECT id, wave_id, role, content, created_at
                     FROM chat_messages
                     WHERE wave_id = $1
                     ORDER BY created_at ASC",
                    &[&wave_id],
                )
                .await?;
            rows.iter().map(map_chat_message_row).collect()
        })
        .await
    }

    pub async fn create_chat_message(&self, message: &ChatMessage) -> StoreResult<()> {
        self.with_client(|client| async move {
            client
                .execute(
                    "INSERT INTO chat_messages (id, wave_id, role, content, created_at)
                     VALUES ($1, $2, $3, $4, $5)",
                    &[
                        &message.id,
                        &message.wave_id,
                        &message.role,
                        &message.content,
                        &message.created_at.unix_timestamp(),
                    ],
                )
                .await?;
            Ok(())
        })
        .await
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
