use std::path::Path;
use std::sync::{Arc, Mutex};

use rusqlite::{params, Connection, OptionalExtension, ToSql};

use crate::lfd::attention::{queue_block_attention_item, queue_block_from_attention};
use crate::lfd::id::LfdId;
use crate::lfd::types::{
    AttentionItem, AttentionKind, AttentionStatus, ChatMemoryBlock, ChatMessage,
    LivePullRequestState, QueueBlock, Repo, RepoEdge, RepoId, Run, RunStatus, Session,
    SessionStatus, SessionUse, Summary, Wave, WaveStatus,
};
use crate::lfdb::catalog::{list_runs_query, list_waves_query, sql, Query, SqlDialect};
use crate::lfdb::rows::{
    map_chat_memory_block_row, map_chat_message_row, map_fork_run_row, map_live_pr_state_row,
    map_repo_edge_row, map_repo_row, map_run_row, map_summary_row, map_wave_row, now_unix,
    serialize_pr,
};
use crate::lfdb::token_crypto;
use crate::lfdb::{
    BusMessage, ForkRun, ForkRunStatus, PmSnapshotRow, RunEventRow, StoreError, StoreResult,
};
use crate::trace::{
    AgentLaunchRow, AgentTurnRow, ContextAsset, ContextAssetKind, ContextAssetRow, ContextChannel,
    ContextDecision, ContextDecisionKind, ContextDecisionRow, ContextScope,
};

#[derive(Debug, Clone)]
pub struct SqliteStore {
    conn: Arc<Mutex<Connection>>,
}

/// How long a bus frame survives. The bus is a wire, not a log: long enough
/// that a mind asleep between passes still catches its hands' reports, short
/// enough that the table never becomes a record anyone is tempted to read.
pub const BUS_WINDOW_SECS: i64 = 60 * 60;

/// The newest bus id ever assigned. `bus_messages` is `AUTOINCREMENT`, so
/// `sqlite_sequence` holds a mark the sweeper cannot erase — which is what lets
/// a subscriber see the gap even when every frame it missed has been deleted.
fn bus_high_water(conn: &Connection) -> StoreResult<i64> {
    let high_water = conn.query_row(
        "SELECT COALESCE((SELECT seq FROM sqlite_sequence WHERE name = 'bus_messages'), 0)",
        [],
        |row| row.get(0),
    )?;
    Ok(high_water)
}

fn migrate_plaintext_provider_tokens(conn: &mut Connection) -> StoreResult<()> {
    let mut scan = conn.prepare(
        "SELECT provider, access_token, refresh_token
         FROM provider_tokens
         WHERE encrypted = 0",
    )?;
    let rows = scan.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
        ))
    })?;

    let mut pending = Vec::new();
    for row in rows {
        pending.push(row?);
    }
    drop(scan);

    if pending.is_empty() {
        return Ok(());
    }

    let tx = conn.transaction()?;
    for (provider, access_token, refresh_token) in pending {
        let encrypted_access = token_crypto::encrypt_token(&access_token).map_err(|error| {
            StoreError::InvalidData(format!(
                "failed to encrypt existing access token for provider '{provider}': {error}"
            ))
        })?;
        let encrypted_refresh =
            token_crypto::encrypt_optional(refresh_token.as_deref()).map_err(|error| {
                StoreError::InvalidData(format!(
                    "failed to encrypt existing refresh token for provider '{provider}': {error}"
                ))
            })?;
        tx.execute(
            "UPDATE provider_tokens
             SET access_token = ?1,
                 refresh_token = ?2,
                 encrypted = 1
             WHERE provider = ?3",
            params![encrypted_access, encrypted_refresh, provider],
        )?;
    }
    tx.commit()?;
    Ok(())
}

type TokenRow = (
    String,
    String,
    Option<String>,
    Option<String>,
    Option<i64>,
    Option<String>,
    i64,
    String,
    bool,
);

fn read_token_row(row: &rusqlite::Row) -> rusqlite::Result<TokenRow> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
        row.get(8)?,
    ))
}

fn decrypt_token_row(row: TokenRow) -> StoreResult<super::ProviderToken> {
    let (
        provider,
        access_token,
        refresh_token,
        oauth_client_id,
        expires_at,
        login,
        updated_at,
        ct,
        encrypted,
    ) = row;
    let access_token =
        token_crypto::decrypt_if_needed(&access_token, encrypted).map_err(|error| {
            StoreError::InvalidData(format!(
                "failed to decrypt access token for provider '{provider}': {error}"
            ))
        })?;
    let refresh_token = refresh_token
        .as_deref()
        .map(|token| token_crypto::decrypt_if_needed(token, encrypted))
        .transpose()
        .map_err(|error| {
            StoreError::InvalidData(format!(
                "failed to decrypt refresh token for provider '{provider}': {error}"
            ))
        })?;
    Ok(super::ProviderToken {
        provider,
        access_token,
        refresh_token,
        oauth_client_id,
        expires_at,
        login,
        updated_at,
        credential_type: super::CredentialType::from_db(&ct),
    })
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

        let mut conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL; PRAGMA busy_timeout = 5000; PRAGMA foreign_keys = ON;",
        )?;

        super::migrations::apply_sqlite(&conn)?;
        validate_run_events_schema(&conn)?;
        migrate_plaintext_provider_tokens(&mut conn)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn put_pm_snapshot(&self, snapshot: &PmSnapshotRow) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO pm_snapshots (repo, wave, provider, initiative, synced_at, payload)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(repo, wave) DO UPDATE SET
               provider = excluded.provider,
               initiative = excluded.initiative,
               synced_at = excluded.synced_at,
               payload = excluded.payload",
            params![
                snapshot.repo,
                snapshot.wave,
                snapshot.provider,
                snapshot.initiative,
                snapshot.synced_at,
                snapshot.payload
            ],
        )?;
        Ok(())
    }

    pub fn pm_snapshot(&self, repo: &str, wave: &str) -> StoreResult<Option<PmSnapshotRow>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            "SELECT repo, wave, provider, initiative, synced_at, payload
             FROM pm_snapshots WHERE repo = ?1 AND wave = ?2",
            params![repo, wave],
            |row| {
                Ok(PmSnapshotRow {
                    repo: row.get(0)?,
                    wave: row.get(1)?,
                    provider: row.get(2)?,
                    initiative: row.get(3)?,
                    synced_at: row.get(4)?,
                    payload: row.get(5)?,
                })
            },
        )
        .optional()
        .map_err(StoreError::from)
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
        let metrics_json = serde_json::to_string(wave.metrics())?;
        let created_at = wave
            .created_at()
            .map(|dt| dt.unix_timestamp())
            .unwrap_or_else(now_unix);

        conn.execute(
            Self::sql(Query::UpsertWave),
            params![
                wave.id(),
                wave.name(),
                direction_json,
                area_json,
                if wave.status() == WaveStatus::Paused {
                    1i64
                } else {
                    0i64
                },
                created_at,
                wave.workers as i64,
                wave.goal(),
                metrics_json,
                wave.parent_wave_id(),
                wave.repo,
                wave.worktree,
                wave.branch,
                wave.status.as_i32() as i64,
                wave.iteration as i64,
                wave.cycle_start_iteration as i64,
            ],
        )?;
        Ok(())
    }

    fn map_control_session_row(row: &rusqlite::Row<'_>) -> Result<Session, rusqlite::Error> {
        let argv_text: String = row.get(8)?;
        let env_text: String = row.get(9)?;
        let argv: Vec<String> = serde_json::from_str(&argv_text).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(8, rusqlite::types::Type::Text, Box::new(err))
        })?;
        let env = serde_json::from_str(&env_text).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(9, rusqlite::types::Type::Text, Box::new(err))
        })?;
        let session_use_text: String = row.get(4)?;
        let session_use = SessionUse::try_from(session_use_text.as_str()).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(err))
        })?;
        Ok(Session {
            id: row.get(0)?,
            wave_id: row.get(1)?,
            run_id: row.get(2)?,
            parent_session_id: row.get(3)?,
            session_use,
            skill: row.get(5)?,
            agent: row.get(6)?,
            cwd: row.get(7)?,
            argv,
            env,
            source: row.get(10)?,
            tmux_name: row.get(11)?,
            status: SessionStatus::from_i32(row.get::<_, i64>(12)? as i32),
            completion_token: row.get(13)?,
            created_at: crate::lfdb::rows::unix_to_datetime(row.get(14)?),
            attached_at: row
                .get::<_, Option<i64>>(15)?
                .map(crate::lfdb::rows::unix_to_datetime),
            started_at: row
                .get::<_, Option<i64>>(16)?
                .map(crate::lfdb::rows::unix_to_datetime),
            completed_at: row
                .get::<_, Option<i64>>(17)?
                .map(crate::lfdb::rows::unix_to_datetime),
        })
    }
}

fn validate_run_events_schema(conn: &Connection) -> StoreResult<()> {
    conn.prepare(
        "SELECT run_id, process_id, parent_process_id, seq, ts, repo, worktree,
                wave, node, event, command, flow, skill, step_index, error,
                input_tokens, output_tokens, cache_read_tokens, cost_usd,
                duration_secs, provider, model
         FROM run_events LIMIT 0",
    )?;
    Ok(())
}

impl SqliteStore {
    pub fn health_check(&self) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(Self::sql(Query::HealthCheck), [], |_| Ok(()))?;
        Ok(())
    }

    pub fn schema_version(&self) -> StoreResult<String> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        super::migrations::latest_version_sqlite(&conn)
    }

    // -- Provider tokens -------------------------------------------------------

    pub fn get_provider_token(&self, provider: &str) -> StoreResult<Option<super::ProviderToken>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT provider, access_token, refresh_token, oauth_client_id, expires_at, login, updated_at, credential_type, encrypted
             FROM provider_tokens WHERE provider = ?1",
        )?;
        let row = stmt
            .query_row(params![provider], read_token_row)
            .optional()?;

        row.map(decrypt_token_row).transpose()
    }

    pub fn upsert_provider_token(&self, token: &super::ProviderToken) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let encrypted_access =
            token_crypto::encrypt_token(&token.access_token).map_err(|error| {
                StoreError::InvalidData(format!(
                    "failed to encrypt access token for provider '{}': {error}",
                    token.provider
                ))
            })?;
        let encrypted_refresh = token_crypto::encrypt_optional(token.refresh_token.as_deref())
            .map_err(|error| {
                StoreError::InvalidData(format!(
                    "failed to encrypt refresh token for provider '{}': {error}",
                    token.provider
                ))
            })?;
        conn.execute(
            "INSERT INTO provider_tokens (provider, access_token, refresh_token, oauth_client_id, expires_at, login, updated_at, credential_type, encrypted)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1)
             ON CONFLICT(provider) DO UPDATE SET
                access_token = excluded.access_token,
                refresh_token = excluded.refresh_token,
                oauth_client_id = excluded.oauth_client_id,
                expires_at = excluded.expires_at,
                login = excluded.login,
                updated_at = excluded.updated_at,
                credential_type = excluded.credential_type,
                encrypted = excluded.encrypted",
            params![
                token.provider,
                encrypted_access,
                encrypted_refresh,
                token.oauth_client_id,
                token.expires_at,
                token.login,
                token.updated_at,
                token.credential_type.as_str(),
            ],
        )?;
        Ok(())
    }

    pub fn delete_provider_token(&self, provider: &str) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "DELETE FROM provider_tokens WHERE provider = ?1",
            params![provider],
        )?;
        Ok(())
    }

    pub fn list_provider_tokens(&self) -> StoreResult<Vec<super::ProviderToken>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT provider, access_token, refresh_token, oauth_client_id, expires_at, login, updated_at, credential_type, encrypted
             FROM provider_tokens ORDER BY provider",
        )?;
        let rows = stmt.query_map([], read_token_row)?;
        let mut tokens = Vec::new();
        for row in rows {
            tokens.push(decrypt_token_row(row?)?);
        }
        Ok(tokens)
    }

    // -- Repos -----------------------------------------------------------------

    pub fn list_repos(&self) -> StoreResult<Vec<Repo>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt =
            conn.prepare("SELECT path, repo_id, name, added_at FROM repos ORDER BY path ASC")?;
        let rows = stmt.query_map([], |row| Ok(map_repo_row(row)))?;

        let mut repos = Vec::new();
        for row in rows {
            repos.push(row??);
        }
        Ok(repos)
    }

    pub fn get_repo(&self, path: &str) -> StoreResult<Option<Repo>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn
            .prepare("SELECT path, repo_id, name, added_at FROM repos WHERE path = ?1 LIMIT 1")?;
        let row = stmt
            .query_row(params![path], |row| Ok(map_repo_row(row)))
            .optional()?;
        row.transpose()
    }

    pub fn get_repo_by_repo_id(&self, repo_id: &RepoId) -> StoreResult<Option<Repo>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT path, repo_id, name, added_at FROM repos WHERE repo_id = ?1 LIMIT 1",
        )?;
        let row = stmt
            .query_row(params![repo_id.as_str()], |row| Ok(map_repo_row(row)))
            .optional()?;
        row.transpose()
    }

    pub fn upsert_repo(&self, repo: &Repo) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO repos (path, repo_id, name, added_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(path) DO UPDATE SET repo_id = excluded.repo_id, name = excluded.name, added_at = excluded.added_at",
            params![
                repo.path,
                repo.repo_id.as_str(),
                repo.name,
                repo.added_at.unix_timestamp()
            ],
        )?;
        Ok(())
    }

    pub fn delete_repo(&self, path: &str) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute("DELETE FROM repos WHERE path = ?1", params![path])?;
        Ok(())
    }

    pub fn list_edges(&self) -> StoreResult<Vec<RepoEdge>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT parent_repo_id, child_repo_id FROM repo_edges ORDER BY parent_repo_id, child_repo_id",
        )?;
        let rows = stmt.query_map([], |row| Ok(map_repo_edge_row(row)))?;

        let mut edges = Vec::new();
        for row in rows {
            edges.push(row??);
        }
        Ok(edges)
    }

    pub fn add_edge(&self, edge: &RepoEdge) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT OR IGNORE INTO repo_edges (parent_repo_id, child_repo_id) VALUES (?1, ?2)",
            params![edge.parent_repo_id.as_str(), edge.child_repo_id.as_str()],
        )?;
        Ok(())
    }

    pub fn remove_edge(&self, parent_id: &RepoId, child_id: &RepoId) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "DELETE FROM repo_edges WHERE parent_repo_id = ?1 AND child_repo_id = ?2",
            params![parent_id.as_str(), child_id.as_str()],
        )?;
        Ok(())
    }

    pub fn children(&self, repo_id: &RepoId) -> StoreResult<Vec<Repo>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT repos.path, repos.repo_id, repos.name, repos.added_at
             FROM repo_edges
             INNER JOIN repos ON repos.repo_id = repo_edges.child_repo_id
             WHERE repo_edges.parent_repo_id = ?1
             ORDER BY repos.path ASC",
        )?;
        let rows = stmt.query_map(params![repo_id.as_str()], |row| Ok(map_repo_row(row)))?;
        let mut repos = Vec::new();
        for row in rows {
            repos.push(row??);
        }
        Ok(repos)
    }

    pub fn parents(&self, repo_id: &RepoId) -> StoreResult<Vec<Repo>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT repos.path, repos.repo_id, repos.name, repos.added_at
             FROM repo_edges
             INNER JOIN repos ON repos.repo_id = repo_edges.parent_repo_id
             WHERE repo_edges.child_repo_id = ?1
             ORDER BY repos.path ASC",
        )?;
        let rows = stmt.query_map(params![repo_id.as_str()], |row| Ok(map_repo_row(row)))?;
        let mut repos = Vec::new();
        for row in rows {
            repos.push(row??);
        }
        Ok(repos)
    }

    const TERMINAL_SESSION_COLS: &str =
        "id, wave_id, run_id, parent_session_id, session_use, skill, agent, cwd, argv, env, source, tmux_name, status, \
         completion_token, created_at, attached_at, started_at, completed_at";

    pub fn create_control_session(&self, session: &Session) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            &format!(
                "INSERT INTO terminal_sessions ({}) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
                Self::TERMINAL_SESSION_COLS
            ),
            params![
                session.id,
                session.wave_id,
                session.run_id,
                session.parent_session_id,
                session.session_use.as_str(),
                session.skill,
                session.agent,
                session.cwd,
                serde_json::to_string(&session.argv)?,
                serde_json::to_string(&session.env)?,
                session.source,
                session.tmux_name,
                session.status.as_i32() as i64,
                session.completion_token,
                session.created_at.unix_timestamp(),
                session.attached_at.map(|dt| dt.unix_timestamp()),
                session.started_at.map(|dt| dt.unix_timestamp()),
                session.completed_at.map(|dt| dt.unix_timestamp()),
            ],
        )?;
        Ok(())
    }

    pub fn get_control_session(&self, session_id: &LfdId) -> StoreResult<Option<Session>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(&format!(
            "SELECT {} FROM terminal_sessions WHERE id = ?1",
            Self::TERMINAL_SESSION_COLS
        ))?;
        let row = stmt
            .query_row(params![session_id], Self::map_control_session_row)
            .optional()?;
        Ok(row)
    }

    pub fn list_control_sessions(
        &self,
        wave_id: Option<&LfdId>,
        statuses: Option<&[SessionStatus]>,
    ) -> StoreResult<Vec<Session>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut sql = format!(
            "SELECT {} FROM terminal_sessions",
            Self::TERMINAL_SESSION_COLS
        );
        let mut predicates = Vec::new();
        let mut params: Vec<Box<dyn ToSql>> = Vec::new();

        if let Some(wave_id) = wave_id {
            predicates.push(format!("wave_id = ?{}", params.len() + 1));
            params.push(Box::new(wave_id.clone()));
        }
        if let Some(statuses) = statuses {
            let placeholders = statuses
                .iter()
                .enumerate()
                .map(|(index, _)| format!("?{}", params.len() + index + 1))
                .collect::<Vec<_>>();
            predicates.push(format!("status IN ({})", placeholders.join(", ")));
            params.extend(
                statuses
                    .iter()
                    .map(|status| Box::new(status.as_i32() as i64) as Box<dyn ToSql>),
            );
        }
        if !predicates.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&predicates.join(" AND "));
        }
        sql.push_str(" ORDER BY created_at ASC");

        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn ToSql> = params.iter().map(|value| value.as_ref()).collect();
        let mut rows = stmt.query(param_refs.as_slice())?;
        let mut sessions = Vec::new();
        while let Some(row) = rows.next()? {
            sessions.push(Self::map_control_session_row(row)?);
        }
        Ok(sessions)
    }

    /// Live sessions plus sessions completed at or after `completed_since`
    /// (unix seconds) — bounded however much terminal history accumulates.
    pub fn list_recent_control_sessions(
        &self,
        wave_id: &LfdId,
        completed_since: i64,
    ) -> StoreResult<Vec<Session>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(&format!(
            "SELECT {} FROM terminal_sessions \
             WHERE wave_id = ?1 AND (status IN (?2, ?3, ?4) OR completed_at >= ?5) \
             ORDER BY created_at ASC",
            Self::TERMINAL_SESSION_COLS
        ))?;
        let mut rows = stmt.query(params![
            wave_id,
            SessionStatus::Pending.as_i32() as i64,
            SessionStatus::Attached.as_i32() as i64,
            SessionStatus::Running.as_i32() as i64,
            completed_since,
        ])?;
        let mut sessions = Vec::new();
        while let Some(row) = rows.next()? {
            sessions.push(Self::map_control_session_row(row)?);
        }
        Ok(sessions)
    }

    pub fn get_active_control_session_for_run(
        &self,
        run_id: &LfdId,
    ) -> StoreResult<Option<Session>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(&format!(
            "SELECT {} FROM terminal_sessions \
             WHERE run_id = ?1 AND status IN (?2, ?3, ?4) \
             ORDER BY created_at DESC LIMIT 1",
            Self::TERMINAL_SESSION_COLS
        ))?;
        let row = stmt
            .query_row(
                params![
                    run_id,
                    SessionStatus::Pending.as_i32() as i64,
                    SessionStatus::Attached.as_i32() as i64,
                    SessionStatus::Running.as_i32() as i64,
                ],
                Self::map_control_session_row,
            )
            .optional()?;
        Ok(row)
    }

    pub fn update_control_session(&self, session: &Session) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let updated = conn.execute(
            "UPDATE terminal_sessions
             SET wave_id = ?2,
                 run_id = ?3,
                 parent_session_id = ?4,
                 session_use = ?5,
                 skill = ?6,
                 agent = ?7,
                 cwd = ?8,
                 argv = ?9,
                 env = ?10,
                 source = ?11,
                 tmux_name = ?12,
                 status = ?13,
                 completion_token = ?14,
                 created_at = ?15,
                 attached_at = ?16,
                 started_at = ?17,
                 completed_at = ?18
             WHERE id = ?1",
            params![
                session.id,
                session.wave_id,
                session.run_id,
                session.parent_session_id,
                session.session_use.as_str(),
                session.skill,
                session.agent,
                session.cwd,
                serde_json::to_string(&session.argv)?,
                serde_json::to_string(&session.env)?,
                session.source,
                session.tmux_name,
                session.status.as_i32() as i64,
                session.completion_token,
                session.created_at.unix_timestamp(),
                session.attached_at.map(|dt| dt.unix_timestamp()),
                session.started_at.map(|dt| dt.unix_timestamp()),
                session.completed_at.map(|dt| dt.unix_timestamp()),
            ],
        )?;
        if updated == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    pub fn list_waves(&self, repo: Option<&str>) -> StoreResult<Vec<Wave>> {
        self.read_waves(repo)
    }

    pub fn list_child_waves(&self, parent: &LfdId) -> StoreResult<Vec<Wave>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(Self::sql(Query::ListChildWaves))?;
        let rows = stmt.query_map(params![parent], |row| Ok(map_wave_row(row)))?;
        let mut waves = Vec::new();
        for wave in rows {
            waves.push(wave??);
        }
        Ok(waves)
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
        // attention_items and terminal_sessions reference waves without
        // ON DELETE CASCADE; delete them explicitly or the wave row is
        // undeletable once either exists.
        conn.execute(
            "DELETE FROM attention_items WHERE wave_id = ?1",
            params![wave_id],
        )?;
        conn.execute(
            "DELETE FROM terminal_sessions WHERE wave_id = ?1",
            params![wave_id],
        )?;
        conn.execute(Self::sql(Query::DeleteWaveById), params![wave_id])?;
        Ok(())
    }

    pub fn list_runs(&self, wave_id: Option<&LfdId>, limit: Option<u32>) -> StoreResult<Vec<Run>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let query = Self::sql(list_runs_query(wave_id.is_some(), limit.is_some()));
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
            Ok(map_run_row(row))
        })?;

        let mut runs = Vec::new();
        for run in rows {
            runs.push(run??);
        }
        Ok(runs)
    }

    /// Non-terminal runs plus runs that ended at or after `ended_since` —
    /// the push bridge's working set (see `Query::ListRunsActiveOrEndedSince`).
    pub fn list_runs_active_or_ended_since(
        &self,
        ended_since: time::OffsetDateTime,
    ) -> StoreResult<Vec<Run>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(Self::sql(Query::ListRunsActiveOrEndedSince))?;
        let rows = stmt.query_map(
            params![
                RunStatus::Completed.as_i32() as i64,
                RunStatus::Failed.as_i32() as i64,
                ended_since.unix_timestamp(),
            ],
            |row| Ok(map_run_row(row)),
        )?;
        let mut runs = Vec::new();
        for run in rows {
            runs.push(run??);
        }
        Ok(runs)
    }

    pub fn get_run(&self, run_id: &LfdId) -> StoreResult<Option<Run>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(Self::sql(Query::GetRunById))?;
        let run = stmt
            .query_row(params![run_id], |row| Ok(map_run_row(row)))
            .optional()?;
        run.transpose()
    }

    pub fn get_active_run(&self, wave_id: &LfdId) -> StoreResult<Option<Run>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(Self::sql(Query::GetActiveRun))?;
        let run = stmt
            .query_row(
                params![
                    wave_id,
                    RunStatus::Pending.as_i32() as i64,
                    RunStatus::Running.as_i32() as i64,
                    RunStatus::Waiting.as_i32() as i64,
                ],
                |row| Ok(map_run_row(row)),
            )
            .optional()?;
        run.transpose()
    }

    pub fn count_active_runs(&self, wave_id: &LfdId) -> StoreResult<u32> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(Self::sql(Query::CountActiveRuns))?;
        let count = stmt.query_row(
            params![
                wave_id,
                RunStatus::Pending.as_i32() as i64,
                RunStatus::Running.as_i32() as i64,
                RunStatus::Waiting.as_i32() as i64,
            ],
            |row| row.get::<_, i64>(0),
        )?;
        Ok(count.max(0) as u32)
    }

    pub fn get_latest_run(&self, wave_id: &LfdId) -> StoreResult<Option<Run>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(Self::sql(Query::GetLatestRun))?;
        let run = stmt
            .query_row(params![wave_id], |row| Ok(map_run_row(row)))
            .optional()?;
        run.transpose()
    }

    pub fn create_run(&self, run: &Run) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let started_at = run
            .started_at
            .map(|dt| dt.unix_timestamp())
            .unwrap_or_else(now_unix);
        let flow_parents_json = serde_json::to_string(&run.flow_parents)?;
        let execution_cursor = run.execution_cursor.clone();
        conn.execute(
            Self::sql(Query::InsertRun),
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
                run.repo,
                run.flow,
                run.task,
                serde_json::to_string(&run.direction)?,
                serde_json::to_string(&run.area)?,
                serialize_pr(&run.pr)?,
                flow_parents_json,
                execution_cursor,
                run.parent_run_id.as_ref(),
                run.parent_pr_number.map(|value| value as i64),
                run.stack_position as i64,
                run.stack_group_id,
                run.stack_status.as_i32() as i64,
                if run.lineage_inferred { 1i64 } else { 0i64 },
                run.target_branch,
                run.repair_of.as_ref(),
            ],
        )?;
        Ok(())
    }

    pub fn update_run(&self, run: &Run) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let flow_parents_json = serde_json::to_string(&run.flow_parents)?;
        let execution_cursor = run.execution_cursor.clone();
        let updated = conn.execute(
            Self::sql(Query::UpdateRun),
            params![
                run.iteration as i64,
                run.step_index as i64,
                run.status.as_i32() as i64,
                run.worktree,
                run.branch,
                run.started_at.map(|dt| dt.unix_timestamp()),
                run.ended_at.map(|dt| dt.unix_timestamp()),
                run.error,
                run.repo,
                run.flow,
                run.task,
                serde_json::to_string(&run.direction)?,
                serde_json::to_string(&run.area)?,
                serialize_pr(&run.pr)?,
                flow_parents_json,
                execution_cursor,
                run.parent_run_id.as_ref(),
                run.parent_pr_number.map(|value| value as i64),
                run.stack_position as i64,
                run.stack_group_id,
                run.stack_status.as_i32() as i64,
                if run.lineage_inferred { 1i64 } else { 0i64 },
                run.target_branch,
                run.repair_of.as_ref(),
                run.id,
            ],
        )?;
        if updated == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    pub fn list_stack_runs(&self, wave_id: &LfdId) -> StoreResult<Vec<Run>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(Self::sql(Query::ListStackRuns))?;
        let rows = stmt.query_map(params![wave_id], |row| Ok(map_run_row(row)))?;

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

    fn map_attention_item_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AttentionItem> {
        let kind_raw: String = row.get(3)?;
        let status_raw: String = row.get(4)?;
        let context_raw: String = row.get(7)?;
        let kind = kind_raw.parse::<AttentionKind>().map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(
                3,
                rusqlite::types::Type::Text,
                Box::new(StoreError::InvalidData(err)),
            )
        })?;
        let status = status_raw.parse::<AttentionStatus>().map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(
                4,
                rusqlite::types::Type::Text,
                Box::new(StoreError::InvalidData(err)),
            )
        })?;

        Ok(AttentionItem {
            id: LfdId::from_raw(row.get::<_, String>(0)?),
            wave_id: LfdId::from_raw(row.get::<_, String>(1)?),
            run_id: row.get::<_, Option<String>>(2)?.map(LfdId::from_raw),
            kind,
            status,
            title: row.get(5)?,
            summary: row.get(6)?,
            context: serde_json::from_str(&context_raw)
                .unwrap_or(serde_json::Value::Object(Default::default())),
            surfaced_at: crate::lfdb::rows::unix_to_datetime(row.get(8)?),
            viewed_at: row
                .get::<_, Option<i64>>(9)?
                .map(crate::lfdb::rows::unix_to_datetime),
            resolved_at: row
                .get::<_, Option<i64>>(10)?
                .map(crate::lfdb::rows::unix_to_datetime),
        })
    }

    pub fn list_attention_items(
        &self,
        status: Option<AttentionStatus>,
        kind: Option<AttentionKind>,
    ) -> StoreResult<Vec<AttentionItem>> {
        let mut sql = String::from(
            "SELECT id, wave_id, run_id, kind, status, title, summary, context, surfaced_at, viewed_at, resolved_at\n             FROM attention_items",
        );
        let mut params: Vec<String> = Vec::new();
        let mut clauses: Vec<String> = Vec::new();
        if let Some(status) = status {
            clauses.push("status = ?".to_string());
            params.push(status.as_str().to_string());
        }
        if let Some(kind) = kind {
            clauses.push("kind = ?".to_string());
            params.push(kind.as_str().to_string());
        }
        if !clauses.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&clauses.join(" AND "));
        }
        sql.push_str(" ORDER BY surfaced_at DESC");

        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(
            rusqlite::params_from_iter(params.iter()),
            Self::map_attention_item_row,
        )?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(StoreError::from)
    }

    pub fn get_attention_item(&self, attention_id: &LfdId) -> StoreResult<Option<AttentionItem>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, wave_id, run_id, kind, status, title, summary, context, surfaced_at, viewed_at, resolved_at\n             FROM attention_items WHERE id = ?1",
        )?;
        stmt.query_row(
            rusqlite::params![attention_id],
            Self::map_attention_item_row,
        )
        .optional()
        .map_err(StoreError::from)
    }

    pub fn find_attention_item_for_run(
        &self,
        run_id: &LfdId,
        kind: AttentionKind,
    ) -> StoreResult<Option<AttentionItem>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, wave_id, run_id, kind, status, title, summary, context, surfaced_at, viewed_at, resolved_at
             FROM attention_items
             WHERE run_id = ?1 AND kind = ?2 AND status != ?3
             ORDER BY surfaced_at DESC
             LIMIT 1",
        )?;
        stmt.query_row(
            rusqlite::params![run_id, kind.as_str(), AttentionStatus::Resolved.as_str()],
            Self::map_attention_item_row,
        )
        .optional()
        .map_err(StoreError::from)
    }

    pub fn upsert_attention_item(&self, item: &AttentionItem) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO attention_items (id, wave_id, run_id, kind, status, title, summary, context, surfaced_at, viewed_at, resolved_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(id) DO UPDATE SET
                wave_id = excluded.wave_id,
                run_id = excluded.run_id,
                kind = excluded.kind,
                status = excluded.status,
                title = excluded.title,
                summary = excluded.summary,
                context = excluded.context,
                surfaced_at = excluded.surfaced_at,
                viewed_at = excluded.viewed_at,
                resolved_at = excluded.resolved_at",
            rusqlite::params![
                &item.id,
                &item.wave_id,
                &item.run_id,
                &item.kind.as_str(),
                &item.status.as_str(),
                &item.title,
                &item.summary,
                &serde_json::to_string(&item.context)?,
                &item.surfaced_at.unix_timestamp(),
                &item.viewed_at.map(|value: time::OffsetDateTime| value.unix_timestamp()),
                &item.resolved_at.map(|value: time::OffsetDateTime| value.unix_timestamp()),
            ],
        )?;
        Ok(())
    }

    pub fn delete_attention_item(&self, attention_id: &LfdId) -> StoreResult<u32> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let deleted = conn.execute(
            "DELETE FROM attention_items WHERE id = ?1",
            rusqlite::params![attention_id],
        )?;
        Ok(deleted as u32)
    }

    pub fn list_queue_blocks(&self, wave_id: &LfdId) -> StoreResult<Vec<QueueBlock>> {
        let blocks = self
            .list_attention_items(
                Some(AttentionStatus::Surfaced),
                Some(AttentionKind::Algedonic),
            )?
            .into_iter()
            .filter(|item| &item.wave_id == wave_id)
            .filter_map(|item| queue_block_from_attention(&item).ok().flatten())
            .collect();
        Ok(blocks)
    }

    pub fn upsert_queue_block(&self, block: &QueueBlock) -> StoreResult<()> {
        self.upsert_attention_item(&queue_block_attention_item(block))
    }

    pub fn delete_queue_block(&self, _wave_id: &LfdId, run_id: &LfdId) -> StoreResult<u32> {
        let id = crate::lfd::attention::attention_id_for_queue_block(run_id);
        let Some(mut item) = self.get_attention_item(&id)? else {
            return Ok(0);
        };
        item.status = AttentionStatus::Resolved;
        item.resolved_at = Some(time::OffsetDateTime::now_utc());
        self.upsert_attention_item(&item)?;
        Ok(1)
    }

    pub fn fail_orphaned_runs(&self) -> StoreResult<u32> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let updated = conn.execute(
            Self::sql(Query::FailOrphanedRuns),
            params![
                RunStatus::Failed.as_i32() as i64,
                "orphaned: lfd restarted",
                now_unix(),
                RunStatus::Pending.as_i32() as i64,
                RunStatus::Running.as_i32() as i64,
                RunStatus::Waiting.as_i32() as i64,
            ],
        )?;
        // Runs that were in flight are now Failed; the waves that owned them
        // would otherwise stay stuck in Running/Waiting and keep their action
        // buttons disabled. Reset them to Idle.
        conn.execute(
            Self::sql(Query::ResetStaleActiveWaves),
            params![
                WaveStatus::Idle.as_i32() as i64,
                WaveStatus::Running.as_i32() as i64,
                WaveStatus::Waiting.as_i32() as i64,
            ],
        )?;
        Ok(updated as u32)
    }

    pub fn list_fork_runs(&self, run_id: &LfdId, step_index: u32) -> StoreResult<Vec<ForkRun>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(Self::sql(Query::ListForkRuns))?;
        let rows = stmt.query_map(params![run_id, step_index as i64], |row| {
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
            "SELECT fr.id, fr.run_id, fr.step_index, fr.branch_index, fr.status, fr.worktree
             FROM fork_runs fr
             LEFT JOIN runs wr ON wr.id = fr.run_id
             WHERE fr.status IN (?1, ?2)
               AND (
                 wr.id IS NULL
                 OR wr.status NOT IN (?3, ?4, ?5)
                 OR fr.step_index != wr.step_index
               )
             ORDER BY fr.run_id ASC, fr.step_index ASC, fr.branch_index ASC",
        )?;
        let rows = stmt.query_map(
            params![
                ForkRunStatus::Pending as i32 as i64,
                ForkRunStatus::Running as i32 as i64,
                RunStatus::Pending.as_i32() as i64,
                RunStatus::Running.as_i32() as i64,
                RunStatus::Waiting.as_i32() as i64
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
                fork_run.run_id,
                fork_run.step_index as i64,
                fork_run.branch_index as i64,
                fork_run.status as i32 as i64,
                fork_run.worktree,
            ],
        )?;
        Ok(())
    }

    pub fn delete_fork_runs(&self, run_id: &LfdId, step_index: u32) -> StoreResult<u32> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let deleted = conn.execute(
            Self::sql(Query::DeleteForkRuns),
            params![run_id, step_index as i64],
        )?;
        Ok(deleted as u32)
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
                summary.agent,
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

    // The agent bus (`bus_messages`): `lf radio pub` publishes, every subscriber
    // polls forward from an id cursor. No process is in the path.

    /// Publish one frame and sweep whatever aged out of the window. The sweep
    /// rides the publish so the bus stays bounded with zero daemons: a bus
    /// nobody writes to needs no cleaning.
    pub fn publish_bus(
        &self,
        channel: &str,
        byline: &str,
        text: &str,
        at: i64,
    ) -> StoreResult<i64> {
        self.sweep_bus(at - BUS_WINDOW_SECS)?;
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO bus_messages (channel, byline, text, at) VALUES (?1, ?2, ?3, ?4)",
            params![channel, byline, text, at],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Drop every frame published before `cutoff` (unix seconds). Publishing
    /// sweeps, and so does every read: a bus that went quiet must still
    /// forget, or a lone expired report would sit there waiting to be
    /// delivered an hour late.
    pub fn sweep_bus(&self, cutoff: i64) -> StoreResult<usize> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.execute("DELETE FROM bus_messages WHERE at < ?1", params![cutoff])?)
    }

    /// Every surviving frame published after `cursor`, oldest first.
    pub fn read_bus_after(&self, cursor: i64) -> StoreResult<Vec<BusMessage>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, channel, byline, text, at FROM bus_messages
             WHERE id > ?1 ORDER BY id",
        )?;
        let rows = stmt.query_map(params![cursor], |row| {
            Ok(BusMessage {
                id: row.get(0)?,
                channel: row.get(1)?,
                byline: row.get(2)?,
                text: row.get(3)?,
                at: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    /// The high-water mark: the newest id ever published, `0` if none ever was.
    /// Read from `sqlite_sequence` rather than `MAX(id)` so it survives the
    /// sweep — a bus swept empty still remembers how far it got. Tuning in
    /// means starting here: a subscriber hears what is said while it listens.
    pub fn bus_head(&self) -> StoreResult<i64> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        bus_high_water(&conn)
    }

    /// The oldest id a subscriber can still read: the oldest surviving frame,
    /// or — on a bus swept empty — one past the high-water mark, because
    /// everything ever published is gone. A durable cursor below `floor - 1`
    /// means the sweeper reached frames this subscriber never read. `None`
    /// only when nothing was ever published, which nobody can have missed.
    pub fn bus_floor(&self) -> StoreResult<Option<i64>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let oldest: Option<i64> =
            conn.query_row("SELECT MIN(id) FROM bus_messages", [], |row| row.get(0))?;
        if oldest.is_some() {
            return Ok(oldest);
        }
        let high_water = bus_high_water(&conn)?;
        Ok((high_water > 0).then_some(high_water + 1))
    }

    pub fn bus_cursor(&self, subscriber: &str) -> StoreResult<Option<i64>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let cursor = conn
            .query_row(
                "SELECT cursor FROM bus_cursors WHERE subscriber = ?1",
                params![subscriber],
                |row| row.get(0),
            )
            .optional()?;
        Ok(cursor)
    }

    pub fn set_bus_cursor(&self, subscriber: &str, cursor: i64) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO bus_cursors (subscriber, cursor) VALUES (?1, ?2)
             ON CONFLICT(subscriber) DO UPDATE SET cursor = excluded.cursor",
            params![subscriber, cursor],
        )?;
        Ok(())
    }

    // Run ledger (`run_events`): the machine-grain, append-only record of
    // every run written directly by `lf`. Read by `lf runs` / `lf trace`.

    /// Cached line/token counts for a git blob. Content-addressed, so a hit is
    /// always correct and a miss only costs one tokenization.
    pub fn blob_tokens(&self, sha: &str) -> StoreResult<Option<(i64, i64, i64)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let row = conn
            .query_row(
                "SELECT lines, bytes, tokens FROM blob_tokens WHERE sha = ?1",
                params![sha],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        Ok(row)
    }

    pub fn put_blob_tokens(
        &self,
        sha: &str,
        lines: i64,
        bytes: i64,
        tokens: i64,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO blob_tokens (sha, lines, bytes, tokens) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(sha) DO NOTHING",
            params![sha, lines, bytes, tokens],
        )?;
        Ok(())
    }

    pub fn insert_run_event(&self, row: &RunEventRow) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO run_events (
                run_id, process_id, parent_process_id, seq, ts, repo, worktree, wave, node, event, command,
                flow, skill, step_index, error, input_tokens, output_tokens,
                cache_read_tokens, cost_usd, duration_secs, provider, model
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)",
            params![
                row.run_id,
                row.process_id,
                row.parent_process_id,
                row.seq,
                row.ts,
                row.repo,
                row.worktree,
                row.wave,
                row.node,
                row.event,
                row.command,
                row.flow,
                row.skill,
                row.step_index,
                row.error,
                row.input_tokens,
                row.output_tokens,
                row.cache_read_tokens,
                row.cost_usd,
                row.duration_secs,
                row.provider,
                row.model,
            ],
        )?;
        Ok(())
    }

    pub fn list_run_events_since(&self, since_unix: i64) -> StoreResult<Vec<RunEventRow>> {
        self.query_run_events(
            "SELECT run_id, process_id, parent_process_id, seq, ts, repo, worktree, wave, node, event, command,
                    flow, skill, step_index, error, input_tokens, output_tokens,
                    cache_read_tokens, cost_usd, duration_secs, provider, model
             FROM run_events WHERE ts >= ?1 ORDER BY ts, run_id, seq",
            params![since_unix],
        )
    }

    /// Events for one run; `run_id` may be a unique prefix.
    pub fn run_events_matching(&self, run_id: &str) -> StoreResult<Vec<RunEventRow>> {
        let prefix = format!("{}%", run_id.replace(['%', '_'], ""));
        self.query_run_events(
            "SELECT run_id, process_id, parent_process_id, seq, ts, repo, worktree, wave, node, event, command,
                    flow, skill, step_index, error, input_tokens, output_tokens,
                    cache_read_tokens, cost_usd, duration_secs, provider, model
             FROM run_events WHERE run_id LIKE ?1 ORDER BY ts, seq",
            params![prefix],
        )
    }

    fn query_run_events(
        &self,
        sql: &str,
        params: impl rusqlite::Params,
    ) -> StoreResult<Vec<RunEventRow>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map(params, |row| {
            Ok(RunEventRow {
                run_id: row.get(0)?,
                process_id: row.get(1)?,
                parent_process_id: row.get(2)?,
                seq: row.get(3)?,
                ts: row.get(4)?,
                repo: row.get(5)?,
                worktree: row.get(6)?,
                wave: row.get(7)?,
                node: row.get(8)?,
                event: row.get(9)?,
                command: row.get(10)?,
                flow: row.get(11)?,
                skill: row.get(12)?,
                step_index: row.get(13)?,
                error: row.get(14)?,
                input_tokens: row.get(15)?,
                output_tokens: row.get(16)?,
                cache_read_tokens: row.get(17)?,
                cost_usd: row.get(18)?,
                duration_secs: row.get(19)?,
                provider: row.get(20)?,
                model: row.get(21)?,
            })
        })?;
        let mut events = Vec::new();
        for row in rows {
            events.push(row?);
        }
        Ok(events)
    }

    pub fn insert_trace_capture(
        &self,
        launch: &AgentLaunchRow,
        turn: &AgentTurnRow,
        assets: &[ContextAssetRow],
        decisions: &[ContextDecisionRow],
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO agent_launches (
                id, run_id, process_id, started_at, ended_at, repo, worktree, wave, flow,
                skill, provider, model, surface, capture_status, incomplete_reason, outcome,
                artifact_dir, conversation_path, provider_events_path, provider_session_id,
                provider_session_path, context_gather_ms, context_render_ms, context_persist_ms,
                conversation_event_count, conversation_bytes
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26)",
            params![
                launch.id,
                launch.run_id,
                launch.process_id,
                launch.started_at,
                launch.ended_at,
                launch.repo,
                launch.worktree,
                launch.wave,
                launch.flow,
                launch.skill,
                launch.provider,
                launch.model,
                launch.surface,
                launch.capture_status,
                launch.incomplete_reason,
                launch.outcome,
                launch.artifact_dir,
                launch.conversation_path,
                launch.provider_events_path,
                launch.provider_session_id,
                launch.provider_session_path,
                launch.context_gather_ms,
                launch.context_render_ms,
                launch.context_persist_ms,
                launch.conversation_event_count,
                launch.conversation_bytes,
            ],
        )?;
        insert_agent_turn(&tx, turn)?;
        for row in assets {
            let asset = &row.asset;
            tx.execute(
                "INSERT INTO context_assets (
                    turn_id, position, channel, kind, scope, label, source_path, included_by,
                    content_sha256, byte_start, byte_end, bytes, isolated_tokens, attributed_tokens
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                params![
                    row.turn_id,
                    i64::from(asset.position),
                    asset.channel.as_str(),
                    asset.kind.as_str(),
                    asset.scope.as_str(),
                    asset.label,
                    asset.source_path,
                    asset.included_by,
                    asset.content_sha256,
                    asset.byte_start as i64,
                    asset.byte_end as i64,
                    asset.bytes as i64,
                    asset.isolated_tokens as i64,
                    asset.attributed_tokens as i64,
                ],
            )?;
        }
        for row in decisions {
            let decision = &row.decision;
            tx.execute(
                "INSERT INTO context_decisions (
                    turn_id, position, kind, label, source_path, decision, reason,
                    original_bytes, original_tokens, asset_position
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    row.turn_id,
                    i64::from(decision.position),
                    decision.kind,
                    decision.label,
                    decision.source_path,
                    decision.decision.as_str(),
                    decision.reason,
                    decision.original_bytes.map(|value| value as i64),
                    decision.original_tokens.map(|value| value as i64),
                    decision.asset_position.map(i64::from),
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn finish_trace_capture(
        &self,
        launch: &AgentLaunchRow,
        turn: &AgentTurnRow,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE agent_launches SET
                ended_at = ?2, capture_status = ?3, incomplete_reason = ?4, outcome = ?5,
                conversation_event_count = ?6, conversation_bytes = ?7,
                provider_session_id = ?8, provider_session_path = ?9
             WHERE id = ?1",
            params![
                launch.id,
                launch.ended_at,
                launch.capture_status,
                launch.incomplete_reason,
                launch.outcome,
                launch.conversation_event_count,
                launch.conversation_bytes,
                launch.provider_session_id,
                launch.provider_session_path,
            ],
        )?;
        tx.execute(
            "UPDATE agent_turns SET
                provider_turn_id = ?2, ended_at = ?3, status = ?4,
                provider_input_tokens = ?5, provider_output_tokens = ?6, reasoning_tokens = ?7,
                cache_read_tokens = ?8, cache_write_tokens = ?9, cost_usd = ?10,
                first_event_seq = ?11, last_event_seq = ?12
             WHERE id = ?1",
            params![
                turn.id,
                turn.provider_turn_id,
                turn.ended_at,
                turn.status,
                turn.provider_input_tokens,
                turn.provider_output_tokens,
                turn.reasoning_tokens,
                turn.cache_read_tokens,
                turn.cache_write_tokens,
                turn.cost_usd,
                turn.first_event_seq,
                turn.last_event_seq,
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn trace_capture_required_after(&self) -> StoreResult<i64> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row(
            "SELECT required_after FROM trace_capture_meta WHERE id = 1",
            [],
            |row| row.get(0),
        )?)
    }

    pub fn agent_launches_matching(&self, run_id: &str) -> StoreResult<Vec<AgentLaunchRow>> {
        let prefix = format!("{}%", run_id.replace(['%', '_'], ""));
        self.query_agent_launches(
            "SELECT id, run_id, process_id, started_at, ended_at, repo, worktree, wave, flow,
                    skill, provider, model, surface, capture_status, incomplete_reason, outcome,
                    artifact_dir, conversation_path, provider_events_path, provider_session_id,
                    provider_session_path, context_gather_ms, context_render_ms, context_persist_ms,
                    conversation_event_count, conversation_bytes
             FROM agent_launches WHERE run_id LIKE ?1 ORDER BY started_at, id",
            params![prefix],
        )
    }

    pub fn agent_launches_since(&self, since: i64) -> StoreResult<Vec<AgentLaunchRow>> {
        self.query_agent_launches(
            "SELECT id, run_id, process_id, started_at, ended_at, repo, worktree, wave, flow,
                    skill, provider, model, surface, capture_status, incomplete_reason, outcome,
                    artifact_dir, conversation_path, provider_events_path, provider_session_id,
                    provider_session_path, context_gather_ms, context_render_ms, context_persist_ms,
                    conversation_event_count, conversation_bytes
             FROM agent_launches WHERE started_at >= ?1 ORDER BY started_at, id",
            params![since],
        )
    }

    fn query_agent_launches(
        &self,
        sql: &str,
        params: impl rusqlite::Params,
    ) -> StoreResult<Vec<AgentLaunchRow>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map(params, |row| {
            Ok(AgentLaunchRow {
                id: row.get(0)?,
                run_id: row.get(1)?,
                process_id: row.get(2)?,
                started_at: row.get(3)?,
                ended_at: row.get(4)?,
                repo: row.get(5)?,
                worktree: row.get(6)?,
                wave: row.get(7)?,
                flow: row.get(8)?,
                skill: row.get(9)?,
                provider: row.get(10)?,
                model: row.get(11)?,
                surface: row.get(12)?,
                capture_status: row.get(13)?,
                incomplete_reason: row.get(14)?,
                outcome: row.get(15)?,
                artifact_dir: row.get(16)?,
                conversation_path: row.get(17)?,
                provider_events_path: row.get(18)?,
                provider_session_id: row.get(19)?,
                provider_session_path: row.get(20)?,
                context_gather_ms: row.get(21)?,
                context_render_ms: row.get(22)?,
                context_persist_ms: row.get(23)?,
                conversation_event_count: row.get(24)?,
                conversation_bytes: row.get(25)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub fn agent_turns_for_launches(
        &self,
        launch_ids: &[String],
    ) -> StoreResult<Vec<AgentTurnRow>> {
        if launch_ids.is_empty() {
            return Ok(Vec::new());
        }
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, launch_id, ordinal, provider_turn_id, started_at, ended_at, status,
                    input_op, context_coverage, tokenizer, system_prompt_path, task_prompt_path,
                    system_tokens, task_tokens, supplied_context_tokens, provider_input_tokens,
                    provider_output_tokens, reasoning_tokens, cache_read_tokens, cache_write_tokens,
                    cost_usd, first_event_seq, last_event_seq
             FROM agent_turns ORDER BY started_at, ordinal",
        )?;
        let rows = stmt.query_map([], map_agent_turn)?;
        let wanted: std::collections::HashSet<&str> =
            launch_ids.iter().map(String::as_str).collect();
        let mut turns = Vec::new();
        for row in rows {
            let turn = row?;
            if wanted.contains(turn.launch_id.as_str()) {
                turns.push(turn);
            }
        }
        Ok(turns)
    }

    pub fn context_assets_for_turns(
        &self,
        turn_ids: &[String],
    ) -> StoreResult<Vec<ContextAssetRow>> {
        if turn_ids.is_empty() {
            return Ok(Vec::new());
        }
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT turn_id, position, channel, kind, scope, label, source_path, included_by,
                    content_sha256, byte_start, byte_end, bytes, isolated_tokens, attributed_tokens
             FROM context_assets ORDER BY turn_id, position",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, i64>(9)?,
                row.get::<_, i64>(10)?,
                row.get::<_, i64>(11)?,
                row.get::<_, i64>(12)?,
                row.get::<_, i64>(13)?,
            ))
        })?;
        let wanted: std::collections::HashSet<&str> = turn_ids.iter().map(String::as_str).collect();
        let mut assets = Vec::new();
        for row in rows {
            let (
                turn_id,
                position,
                channel,
                kind,
                scope,
                label,
                source_path,
                included_by,
                content_sha256,
                byte_start,
                byte_end,
                bytes,
                isolated_tokens,
                attributed_tokens,
            ) = row?;
            if !wanted.contains(turn_id.as_str()) {
                continue;
            }
            assets.push(ContextAssetRow {
                turn_id,
                asset: ContextAsset {
                    position: position as u32,
                    channel: ContextChannel::parse(&channel)?,
                    kind: ContextAssetKind::parse(&kind)?,
                    scope: ContextScope::parse(&scope)?,
                    label,
                    source_path,
                    included_by,
                    content_sha256,
                    byte_start: byte_start as u64,
                    byte_end: byte_end as u64,
                    bytes: bytes as u64,
                    isolated_tokens: isolated_tokens as u64,
                    attributed_tokens: attributed_tokens as u64,
                },
            });
        }
        Ok(assets)
    }

    pub fn context_decisions_for_turns(
        &self,
        turn_ids: &[String],
    ) -> StoreResult<Vec<ContextDecisionRow>> {
        if turn_ids.is_empty() {
            return Ok(Vec::new());
        }
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT turn_id, position, kind, label, source_path, decision, reason,
                    original_bytes, original_tokens, asset_position
             FROM context_decisions ORDER BY turn_id, position",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, Option<i64>>(7)?,
                row.get::<_, Option<i64>>(8)?,
                row.get::<_, Option<i64>>(9)?,
            ))
        })?;
        let wanted: std::collections::HashSet<&str> = turn_ids.iter().map(String::as_str).collect();
        let mut decisions = Vec::new();
        for row in rows {
            let (
                turn_id,
                position,
                kind,
                label,
                source_path,
                decision,
                reason,
                original_bytes,
                original_tokens,
                asset_position,
            ) = row?;
            if !wanted.contains(turn_id.as_str()) {
                continue;
            }
            decisions.push(ContextDecisionRow {
                turn_id,
                decision: ContextDecision {
                    position: position as u32,
                    kind,
                    label,
                    source_path,
                    decision: ContextDecisionKind::parse(&decision)?,
                    reason,
                    original_bytes: original_bytes.map(|value| value as u64),
                    original_tokens: original_tokens.map(|value| value as u64),
                    asset_position: asset_position.map(|value| value as u32),
                },
            });
        }
        Ok(decisions)
    }
}

fn insert_agent_turn(tx: &rusqlite::Transaction<'_>, turn: &AgentTurnRow) -> StoreResult<()> {
    tx.execute(
        "INSERT INTO agent_turns (
            id, launch_id, ordinal, provider_turn_id, started_at, ended_at, status, input_op,
            context_coverage, tokenizer, system_prompt_path, task_prompt_path, system_tokens,
            task_tokens, supplied_context_tokens, provider_input_tokens, provider_output_tokens,
            reasoning_tokens, cache_read_tokens, cache_write_tokens, cost_usd, first_event_seq,
            last_event_seq
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
            ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23)",
        params![
            turn.id,
            turn.launch_id,
            turn.ordinal,
            turn.provider_turn_id,
            turn.started_at,
            turn.ended_at,
            turn.status,
            turn.input_op,
            turn.context_coverage,
            turn.tokenizer,
            turn.system_prompt_path,
            turn.task_prompt_path,
            turn.system_tokens,
            turn.task_tokens,
            turn.supplied_context_tokens,
            turn.provider_input_tokens,
            turn.provider_output_tokens,
            turn.reasoning_tokens,
            turn.cache_read_tokens,
            turn.cache_write_tokens,
            turn.cost_usd,
            turn.first_event_seq,
            turn.last_event_seq,
        ],
    )?;
    Ok(())
}

fn map_agent_turn(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentTurnRow> {
    Ok(AgentTurnRow {
        id: row.get(0)?,
        launch_id: row.get(1)?,
        ordinal: row.get(2)?,
        provider_turn_id: row.get(3)?,
        started_at: row.get(4)?,
        ended_at: row.get(5)?,
        status: row.get(6)?,
        input_op: row.get(7)?,
        context_coverage: row.get(8)?,
        tokenizer: row.get(9)?,
        system_prompt_path: row.get(10)?,
        task_prompt_path: row.get(11)?,
        system_tokens: row.get(12)?,
        task_tokens: row.get(13)?,
        supplied_context_tokens: row.get(14)?,
        provider_input_tokens: row.get(15)?,
        provider_output_tokens: row.get(16)?,
        reasoning_tokens: row.get(17)?,
        cache_read_tokens: row.get(18)?,
        cache_write_tokens: row.get(19)?,
        cost_usd: row.get(20)?,
        first_event_seq: row.get(21)?,
        last_event_seq: row.get(22)?,
    })
}
