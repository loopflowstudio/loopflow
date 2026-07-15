use std::path::Path;
use std::sync::{Arc, Mutex};

use rusqlite::{params, Connection, OptionalExtension, ToSql};

use crate::id::WaveId;
use crate::store::rows::{map_wave_row, now_unix};
use crate::store::token_crypto;
use crate::store::{BusMessage, PmSnapshotRow, RunEventRow, StoreError, StoreResult};
use crate::trace::{
    AgentLaunchRow, AgentTurnRow, ContextAsset, ContextAssetKind, ContextAssetRow, ContextChannel,
    ContextDecision, ContextDecisionKind, ContextDecisionRow, ContextScope,
};
use crate::wave::Wave;

mod child_sessions;

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
    pub fn new(path: &Path) -> StoreResult<Self> {
        let existing_database = std::fs::metadata(path).is_ok_and(|metadata| metadata.len() > 0);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| {
                StoreError::InvalidData(format!("failed to create db dir: {err}"))
            })?;
        }

        let mut conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL; PRAGMA busy_timeout = 5000; PRAGMA foreign_keys = ON;",
        )?;

        if existing_database && crate::build_info::provenance().is_release() {
            super::migrations::apply_sqlite_with_backup(&conn, path)?;
        } else {
            super::migrations::apply_sqlite(&conn)?;
        }
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
        let query = if repo.is_some() {
            "SELECT id, name, repo, created_at, parent_wave_id
             FROM waves WHERE repo = ?1 ORDER BY created_at DESC"
        } else {
            "SELECT id, name, repo, created_at, parent_wave_id
             FROM waves ORDER BY created_at DESC"
        };
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
        let created_at = wave
            .created_at()
            .map(|dt| dt.unix_timestamp())
            .unwrap_or_else(now_unix);

        conn.execute(
            "INSERT INTO waves (id, name, repo, created_at, parent_wave_id)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET
               name = excluded.name,
               repo = excluded.repo,
               created_at = excluded.created_at,
               parent_wave_id = excluded.parent_wave_id",
            params![
                wave.id(),
                wave.name(),
                wave.repo(),
                created_at,
                wave.parent_wave_id(),
            ],
        )?;
        Ok(())
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
        conn.query_row("SELECT 1", [], |_| Ok(()))?;
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

    pub fn list_waves(&self, repo: Option<&str>) -> StoreResult<Vec<Wave>> {
        self.read_waves(repo)
    }

    pub fn list_child_waves(&self, parent: &WaveId) -> StoreResult<Vec<Wave>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, name, repo, created_at, parent_wave_id
             FROM waves WHERE parent_wave_id = ?1 ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map(params![parent], |row| Ok(map_wave_row(row)))?;
        let mut waves = Vec::new();
        for wave in rows {
            waves.push(wave??);
        }
        Ok(waves)
    }

    pub fn get_wave(&self, wave_id: &WaveId) -> StoreResult<Option<Wave>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, name, repo, created_at, parent_wave_id FROM waves WHERE id = ?1",
        )?;
        let wave = stmt
            .query_row(params![wave_id], |row| Ok(map_wave_row(row)))
            .optional()?;
        wave.transpose()
    }

    pub fn get_wave_by_name(&self, name: &str) -> StoreResult<Option<Wave>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, name, repo, created_at, parent_wave_id FROM waves WHERE name = ?1",
        )?;
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

    pub fn delete_wave(&self, wave_id: &WaveId) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute("DELETE FROM waves WHERE id = ?1", params![wave_id])?;
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

    // Exec ledger (`run_events`): the machine-grain, append-only record of
    // every process written directly by `lf`. Read by `lf execs` / `lf trace`;
    // `lf runs` joins it to agent launches for process lineage.

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

    /// Events for one trace; the persisted `run_id` may be a unique prefix.
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

    /// Events identifying one exec by process-id prefix. The caller resolves
    /// its trace, then reads that trace whole.
    pub fn run_events_matching_exec(&self, exec_id: &str) -> StoreResult<Vec<RunEventRow>> {
        let prefix = format!("{}%", exec_id.replace(['%', '_'], ""));
        self.query_run_events(
            "SELECT run_id, process_id, parent_process_id, seq, ts, repo, worktree, wave, node, event, command,
                    flow, skill, step_index, error, input_tokens, output_tokens,
                    cache_read_tokens, cost_usd, duration_secs, provider, model
             FROM run_events WHERE process_id LIKE ?1 ORDER BY ts, seq",
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
                provider_session_path, conversation_event_count, conversation_bytes
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23)",
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
                launch.conversation_event_count,
                launch.conversation_bytes,
            ],
        )?;
        insert_agent_turn(&tx, turn)?;
        insert_context_rows(&tx, assets, decisions)?;
        tx.commit()?;
        Ok(())
    }

    pub fn insert_agent_turn_capture(
        &self,
        turn: &AgentTurnRow,
        assets: &[ContextAssetRow],
        decisions: &[ContextDecisionRow],
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        insert_agent_turn(&tx, turn)?;
        insert_context_rows(&tx, assets, decisions)?;
        tx.commit()?;
        Ok(())
    }

    pub fn finish_agent_turn_capture(&self, turn: &AgentTurnRow) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        update_agent_turn(&conn, turn)?;
        Ok(())
    }

    pub fn update_agent_launch_receipt(&self, launch: &AgentLaunchRow) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE agent_launches
             SET provider_session_id = ?2, provider_session_path = ?3
             WHERE id = ?1",
            params![
                launch.id,
                launch.provider_session_id,
                launch.provider_session_path,
            ],
        )?;
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
        update_agent_turn(&tx, turn)?;
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
        // Launch timestamps use ledger-second precision. rowid preserves the
        // append order when a fast flow starts several agents in one second.
        self.query_agent_launches(
            "SELECT id, run_id, process_id, started_at, ended_at, repo, worktree, wave, flow,
                    skill, provider, model, surface, capture_status, incomplete_reason, outcome,
                    artifact_dir, conversation_path, provider_events_path, provider_session_id,
                    provider_session_path, conversation_event_count, conversation_bytes
             FROM agent_launches WHERE run_id LIKE ?1 ORDER BY started_at, rowid",
            params![prefix],
        )
    }

    pub fn agent_launches_since(&self, since: i64) -> StoreResult<Vec<AgentLaunchRow>> {
        self.query_agent_launches(
            "SELECT id, run_id, process_id, started_at, ended_at, repo, worktree, wave, flow,
                    skill, provider, model, surface, capture_status, incomplete_reason, outcome,
                    artifact_dir, conversation_path, provider_events_path, provider_session_id,
                    provider_session_path, conversation_event_count, conversation_bytes
             FROM agent_launches WHERE started_at >= ?1 ORDER BY started_at, rowid",
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
                conversation_event_count: row.get(21)?,
                conversation_bytes: row.get(22)?,
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
        let mut turns = Vec::new();
        for launch_ids in launch_ids.chunks(500) {
            let placeholders = in_placeholders(launch_ids.len());
            let sql = format!(
                "SELECT id, launch_id, ordinal, provider_turn_id, started_at, ended_at, status,
                    input_op, context_coverage, tokenizer, system_prompt_path, task_prompt_path,
                    system_tokens, task_tokens, supplied_context_tokens, provider_input_tokens,
                    provider_output_tokens, reasoning_tokens, cache_read_tokens, cache_write_tokens,
                    cost_usd, context_gather_ms, context_render_ms, context_persist_ms,
                    first_event_seq, last_event_seq
             FROM agent_turns WHERE launch_id IN ({placeholders})
             ORDER BY started_at, rowid, ordinal"
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(rusqlite::params_from_iter(launch_ids), map_agent_turn)?;
            turns.extend(rows.collect::<Result<Vec<_>, _>>()?);
        }
        turns.sort_by_key(|turn| (turn.started_at, turn.ordinal));
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
        let mut assets = Vec::new();
        for turn_ids in turn_ids.chunks(500) {
            let placeholders = in_placeholders(turn_ids.len());
            let sql = format!(
                "SELECT turn_id, position, channel, kind, scope, label, source_path, included_by,
                    content_sha256, byte_start, byte_end, bytes, isolated_tokens, attributed_tokens
             FROM context_assets WHERE turn_id IN ({placeholders})
             ORDER BY turn_id, position"
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(rusqlite::params_from_iter(turn_ids), |row| {
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
        }
        assets.sort_by_key(|row| (row.turn_id.clone(), row.asset.position));
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
        let mut decisions = Vec::new();
        for turn_ids in turn_ids.chunks(500) {
            let placeholders = in_placeholders(turn_ids.len());
            let sql = format!(
                "SELECT turn_id, position, kind, scope, label, source_path, decision, reason,
                    original_bytes, original_tokens, asset_position
             FROM context_decisions WHERE turn_id IN ({placeholders})
             ORDER BY turn_id, position"
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(rusqlite::params_from_iter(turn_ids), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, Option<i64>>(8)?,
                    row.get::<_, Option<i64>>(9)?,
                    row.get::<_, Option<i64>>(10)?,
                ))
            })?;
            for row in rows {
                let (
                    turn_id,
                    position,
                    kind,
                    scope,
                    label,
                    source_path,
                    decision,
                    reason,
                    original_bytes,
                    original_tokens,
                    asset_position,
                ) = row?;
                decisions.push(ContextDecisionRow {
                    turn_id,
                    decision: ContextDecision {
                        position: position as u32,
                        kind: ContextAssetKind::parse(&kind)?,
                        scope: ContextScope::parse(&scope)?,
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
        }
        decisions.sort_by_key(|row| (row.turn_id.clone(), row.decision.position));
        Ok(decisions)
    }
}

/// `?, ?, ?` for a `WHERE col IN (...)` clause bound by position.
fn in_placeholders(count: usize) -> String {
    std::iter::repeat_n("?", count)
        .collect::<Vec<_>>()
        .join(", ")
}

fn insert_agent_turn(tx: &rusqlite::Transaction<'_>, turn: &AgentTurnRow) -> StoreResult<()> {
    tx.execute(
        "INSERT INTO agent_turns (
            id, launch_id, ordinal, provider_turn_id, started_at, ended_at, status, input_op,
            context_coverage, tokenizer, system_prompt_path, task_prompt_path, system_tokens,
            task_tokens, supplied_context_tokens, provider_input_tokens, provider_output_tokens,
            reasoning_tokens, cache_read_tokens, cache_write_tokens, cost_usd,
            context_gather_ms, context_render_ms, context_persist_ms, first_event_seq,
            last_event_seq
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
            ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26)",
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
            turn.context_gather_ms,
            turn.context_render_ms,
            turn.context_persist_ms,
            turn.first_event_seq,
            turn.last_event_seq,
        ],
    )?;
    Ok(())
}

fn insert_context_rows(
    tx: &rusqlite::Transaction<'_>,
    assets: &[ContextAssetRow],
    decisions: &[ContextDecisionRow],
) -> StoreResult<()> {
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
                turn_id, position, kind, scope, label, source_path, decision, reason,
                original_bytes, original_tokens, asset_position
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                row.turn_id,
                i64::from(decision.position),
                decision.kind.as_str(),
                decision.scope.as_str(),
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
    Ok(())
}

fn update_agent_turn(conn: &rusqlite::Connection, turn: &AgentTurnRow) -> StoreResult<()> {
    conn.execute(
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
        context_gather_ms: row.get(21)?,
        context_render_ms: row.get(22)?,
        context_persist_ms: row.get(23)?,
        first_event_seq: row.get(24)?,
        last_event_seq: row.get(25)?,
    })
}
