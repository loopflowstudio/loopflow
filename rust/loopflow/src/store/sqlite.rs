use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rusqlite::{params, Connection, OptionalExtension, ToSql, TransactionBehavior};

use crate::chat::types::TurnUsage;
use crate::id::WaveId;
use crate::profile::{
    AccessProfile, AccountAccessProfile, EmailAddress, ProfileId, ProviderRoute, RouteScope,
};
use crate::provider_auth::Provider;
use crate::store::rows::{map_wave_row, now_unix};
use crate::store::token_crypto;
use crate::store::{
    AccountLimitRow, AttributedTurnUsage, CredentialState, PmSnapshotRow, ProviderAccount,
    ProviderAccountId, ProviderAccountSelection, RoutingState, RunEventRow, StoreError,
    StoreResult, TurnUsageSample, WaveLocatorUpdate,
};
#[cfg(test)]
use crate::store::{
    PerformanceEvidenceSnapshot, TaskFirstProgressEvidenceRow, TaskPrPerformanceEvidenceRow,
};
use crate::trace::{
    AgentInvocationRow, AgentTurnRow, ContextAsset, ContextAssetKind, ContextAssetRow,
    ContextChannel, ContextDecision, ContextDecisionKind, ContextDecisionRow, ContextScope,
};
use crate::wave::{Wave, WaveLocator};

mod children;
mod ci_incidents;
mod durable;
mod metrics;
mod provider_deliveries;

/// A fleet can legitimately queue longer than SQLite's common five-second
/// default while every process opens and records its first receipt. Durable
/// writes wait for that bounded local contention instead of dropping evidence.
pub(crate) const SQLITE_WRITE_BUSY_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone)]
pub struct SqliteStore {
    conn: Arc<Mutex<Connection>>,
}

pub(crate) fn read_nonterminal_task_worktrees(path: &Path) -> StoreResult<Vec<PathBuf>> {
    let conn = Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.execute_batch("PRAGMA query_only = ON; PRAGMA busy_timeout = 5000;")?;
    let mut statement = conn.prepare(
        "SELECT t.worktree FROM tasks t
         JOIN epochs e ON e.id=(
             SELECT latest.id FROM epochs latest
             WHERE latest.task_id=t.id ORDER BY latest.number DESC LIMIT 1
         )
         WHERE e.state='open'",
    )?;
    let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
    rows.map(|row| row.map(PathBuf::from).map_err(StoreError::from))
        .collect()
}

/// Whether an AgentInvocation still owns mutation authority for its writer.
///
/// Process liveness is deliberately absent from this proof. A provider process
/// may outlive its Run or Ask, but it cannot keep mutation authority after the
/// durable owner settles.
pub(crate) fn writer_invocation_is_authoritative(
    path: &Path,
    invocation_id: &str,
) -> StoreResult<bool> {
    let conn = Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.execute_batch("PRAGMA query_only = ON; PRAGMA busy_timeout = 5000;")?;
    let invocation = conn
        .query_row(
            "SELECT i.ended_at, i.supervising_run_id, i.answer_ask_id,
                    EXISTS (
                        SELECT 1 FROM agent_turns t
                        WHERE t.invocation_id=i.id AND t.status='running'
                    )
             FROM agent_invocations i WHERE i.id=?1",
            [invocation_id],
            |row| {
                Ok((
                    row.get::<_, Option<i64>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, bool>(3)?,
                ))
            },
        )
        .optional()?;
    let Some((ended_at, run_id, ask_id, turn_active)) = invocation else {
        return Ok(false);
    };
    if ended_at.is_some() || !turn_active {
        return Ok(false);
    }
    let Some(run_id) = run_id else {
        return Ok(false);
    };
    let run_active = conn
        .query_row(
            "SELECT state='active' FROM runs WHERE id=?1",
            [&run_id],
            |row| row.get::<_, bool>(0),
        )
        .optional()?
        .unwrap_or(false);
    if !run_active {
        return Ok(false);
    }
    let Some(ask_id) = ask_id else {
        return Ok(true);
    };
    Ok(conn
        .query_row(
            "SELECT state='claimed' AND active_invocation_id=?2
             FROM ask_exchanges WHERE id=?1",
            params![ask_id, invocation_id],
            |row| row.get::<_, bool>(0),
        )
        .optional()?
        .unwrap_or(false))
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

fn read_provider_account(row: &rusqlite::Row) -> rusqlite::Result<StoreResult<ProviderAccount>> {
    let provider = row.get(0)?;
    let account_id = row.get::<_, String>(1)?;
    let home = row
        .get::<_, Option<String>>(2)?
        .map(std::path::PathBuf::from);
    let login_email = row.get::<_, Option<String>>(3)?;
    let credential_state = row.get::<_, String>(4)?;
    let routing_state = row.get::<_, String>(5)?;
    let plan = row.get(6)?;
    let paid_through = row.get::<_, Option<i32>>(7)?;
    let utilization_percent = row.get(8)?;
    let cooldown_until = row.get(9)?;
    let cooldown_reason = row.get(10)?;
    let last_selected_at = row.get(11)?;
    let created_at = row.get(12)?;
    let updated_at = row.get(13)?;
    Ok((|| {
        let account_id = ProviderAccountId::parse(&account_id).map_err(StoreError::InvalidData)?;
        let login_email = login_email
            .map(|value| EmailAddress::parse(&value))
            .transpose()
            .map_err(StoreError::InvalidData)?;
        let credential_state =
            CredentialState::from_db(&credential_state).map_err(StoreError::InvalidData)?;
        let routing_state =
            RoutingState::from_db(&routing_state).map_err(StoreError::InvalidData)?;
        let paid_through = paid_through
            .map(time::Date::from_julian_day)
            .transpose()
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        Ok(ProviderAccount {
            provider,
            account_id,
            home,
            login_email,
            credential_state,
            routing_state,
            plan,
            paid_through,
            utilization_percent,
            cooldown_until,
            cooldown_reason,
            last_selected_at,
            created_at,
            updated_at,
        })
    })())
}

fn read_account_limit_row(row: &rusqlite::Row) -> rusqlite::Result<StoreResult<AccountLimitRow>> {
    let account_id = row.get::<_, String>(1)?;
    let account_id = match ProviderAccountId::parse(&account_id) {
        Ok(account_id) => account_id,
        Err(error) => return Ok(Err(StoreError::InvalidData(error))),
    };
    Ok(Ok(AccountLimitRow {
        provider: row.get(0)?,
        account_id,
        window: row.get(2)?,
        used_percent: row.get(3)?,
        resets_at: row.get(4)?,
        plan: row.get(5)?,
        observed_at: row.get(6)?,
        source: row.get(7)?,
    }))
}

fn read_access_profile(row: &rusqlite::Row) -> rusqlite::Result<StoreResult<AccessProfile>> {
    let profile_id = row.get::<_, String>(0)?;
    let chrome_directory = row.get(1)?;
    let expected_login = row.get::<_, String>(2)?;
    let created_at = row.get(3)?;
    let updated_at = row.get(4)?;
    Ok(ProfileId::parse(&profile_id)
        .map_err(StoreError::InvalidData)
        .and_then(|id| {
            EmailAddress::parse(&expected_login)
                .map_err(StoreError::InvalidData)
                .map(|expected_login| AccessProfile {
                    id,
                    chrome_directory,
                    expected_login,
                    created_at,
                    updated_at,
                })
        }))
}

fn read_account_access_profile(
    row: &rusqlite::Row,
) -> rusqlite::Result<StoreResult<AccountAccessProfile>> {
    let provider = row.get::<_, String>(0)?;
    let account_id = row.get::<_, String>(1)?;
    let position = row.get::<_, i64>(2)? as usize;
    let profile_id = row.get::<_, String>(3)?;
    Ok(provider
        .parse::<Provider>()
        .map_err(|error| StoreError::InvalidData(error.to_string()))
        .and_then(|provider| {
            ProviderAccountId::parse(&account_id)
                .map_err(StoreError::InvalidData)
                .map(|account_id| (provider, account_id))
        })
        .and_then(|(provider, account_id)| {
            ProfileId::parse(&profile_id)
                .map_err(StoreError::InvalidData)
                .map(|profile_id| AccountAccessProfile {
                    provider,
                    account_id,
                    position,
                    profile_id,
                })
        }))
}

fn read_provider_route_account(row: &rusqlite::Row) -> rusqlite::Result<ProviderAccountId> {
    ProviderAccountId::parse(&row.get::<_, String>(0)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, error.into())
    })
}

fn to_sqlite_conversion_error(error: impl std::fmt::Display) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            error.to_string(),
        )),
    )
}

impl SqliteStore {
    /// Open the store for ordinary use. This never advances the shared release
    /// frontier: against `~/.lf/loopflow.db` it reads and validates but leaves
    /// the migration frontier where the installed `lf` left it. Advancing the
    /// shared frontier is the promotion boundary's job — see
    /// [`Self::open_as_promotion_boundary`].
    pub fn new(path: &Path) -> StoreResult<Self> {
        Self::open(path, super::FrontierAdvance::Forbidden)
    }

    /// Open the shared store as `lf install promote` — the single authorized
    /// owner of the migration frontier. Applies any pending migration under the
    /// caller's exclusive promotion lock and drained live-body fence.
    pub(crate) fn open_as_promotion_boundary(path: &Path) -> StoreResult<Self> {
        Self::open(path, super::FrontierAdvance::Authorized)
    }

    fn open(path: &Path, advance: super::FrontierAdvance) -> StoreResult<Self> {
        Self::open_with(
            path,
            crate::build_info::migration_authority(),
            &super::machine_home_dir(),
            advance,
        )
    }

    /// Open resolving the migration decision against an explicit authority and
    /// machine home rather than this build's compiled-in values. Production opens
    /// pass the real ones through [`Self::open`]; the same-module shared-frontier
    /// regressions pass a temp home and a chosen authority so they drive the
    /// published and promotion-boundary branches a validation-only test build
    /// cannot reach through the compiled-in authority.
    fn open_with(
        path: &Path,
        authority: crate::build_info::MigrationAuthority,
        home: &Path,
        advance: super::FrontierAdvance,
    ) -> StoreResult<Self> {
        let existing_database = std::fs::metadata(path).is_ok_and(|metadata| metadata.len() > 0);

        // Resolve the frontier authority before touching the filesystem. An
        // ordinary open of a shared store it may not initialize refuses here,
        // before create_dir_all/Connection::open would leave an empty
        // ~/.lf/loopflow.db behind — a file whose mere existence a liveness or
        // bootstrap check could misread as "the shared store is initialized".
        let may_apply_migrations = super::may_apply_migrations(path, authority, home, advance)
            .map_err(|error| {
                StoreError::InvalidData(format!("resolve migration authority: {error}"))
            })?;
        if !may_apply_migrations && !existing_database {
            return Err(StoreError::InvalidData(format!(
                "shared store {} is not initialized and an ordinary lf may not create it; \
                 install a published release with `uv run python scripts/install.py refresh`",
                path.display()
            )));
        }

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| {
                StoreError::InvalidData(format!("failed to create db dir: {err}"))
            })?;
        }

        let mut conn = Connection::open(path)?;
        // Install the handler before journal-mode negotiation: that pragma can
        // itself meet another process opening the same WAL database.
        conn.busy_timeout(SQLITE_WRITE_BUSY_TIMEOUT)?;
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")?;

        if !may_apply_migrations {
            // Validate the applied history first (preserving divergent/incompatible
            // and store-ahead errors), then refuse if this binary knows a migration
            // the store has not applied. An ordinary open must not hand back a store
            // whose schema is older than this binary's code, which may query the
            // columns that pending migration adds.
            super::migrations::validate_sqlite(&conn)?;
            if let Some(pending) = super::migrations::pending_shared_migration(&conn)? {
                return Err(StoreError::InvalidData(format!(
                    "shared store {} is at an older frontier than this lf (pending {pending}); \
                     an ordinary lf must not advance it — install a published release with \
                     `uv run python scripts/install.py refresh`",
                    path.display()
                )));
            }
        } else if existing_database {
            super::migrations::apply_sqlite_with_backup(&conn, path)?;
        } else {
            super::migrations::apply_sqlite(&conn)?;
        }
        validate_run_events_schema(&conn)?;
        if may_apply_migrations {
            migrate_plaintext_provider_tokens(&mut conn)?;
        }

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Open only the stable run ledger surface without schema or token writes.
    /// Observability commands use this when a source build may be older than
    /// the machine's release-owned database.
    pub(crate) fn open_run_ledger_read_only(path: &Path) -> StoreResult<Self> {
        let conn = Connection::open_with_flags(
            path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        conn.execute_batch("PRAGMA query_only = ON; PRAGMA busy_timeout = 5000;")?;
        validate_run_events_schema(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Run several ledger queries against one SQLite read snapshot.
    ///
    /// The store must not be cloned into the closure: each query briefly takes
    /// the same connection lock while the connection-level transaction stays
    /// open. Observability callers create a private read-only store for this
    /// operation, so no unrelated reader can join the transaction.
    pub(crate) fn read_run_ledger_snapshot<T>(
        &self,
        read: impl FnOnce(&Self) -> StoreResult<T>,
    ) -> StoreResult<T> {
        {
            let conn = self.conn.lock().expect("store mutex poisoned");
            conn.execute_batch("BEGIN DEFERRED TRANSACTION")?;
        }
        let result = read(self);
        let finish = {
            let conn = self.conn.lock().expect("store mutex poisoned");
            if result.is_ok() {
                conn.execute_batch("COMMIT")
            } else {
                conn.execute_batch("ROLLBACK")
            }
        };
        match result {
            Ok(value) => {
                finish?;
                Ok(value)
            }
            Err(error) => Err(error),
        }
    }

    #[cfg(test)]
    pub(crate) fn apply_migration_for_test(&self, name: &str) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        if crate::store::migrations::migration_is_applied_for_test(&conn, name)? {
            return Ok(());
        }
        conn.execute_batch(&crate::store::migrations::migration_sql_for_test(name))?;
        Ok(())
    }

    pub fn put_pm_snapshot(&self, snapshot: &PmSnapshotRow) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO pm_snapshots (wave_id, provider, initiative, synced_at, payload)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(wave_id) DO UPDATE SET
               provider = excluded.provider,
               initiative = excluded.initiative,
               synced_at = excluded.synced_at,
               payload = excluded.payload",
            params![
                snapshot.wave_id,
                snapshot.provider,
                snapshot.initiative,
                snapshot.synced_at,
                snapshot.payload
            ],
        )?;
        Ok(())
    }

    pub fn pm_snapshot(&self, wave_id: &WaveId) -> StoreResult<Option<PmSnapshotRow>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            "SELECT wave_id, provider, initiative, synced_at, payload
             FROM pm_snapshots WHERE wave_id = ?1",
            params![wave_id],
            |row| {
                Ok(PmSnapshotRow {
                    wave_id: row.get(0)?,
                    provider: row.get(1)?,
                    initiative: row.get(2)?,
                    synced_at: row.get(3)?,
                    payload: row.get(4)?,
                })
            },
        )
        .optional()
        .map_err(StoreError::from)
    }

    fn read_waves(&self, repo: Option<&str>) -> StoreResult<Vec<Wave>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let query = if repo.is_some() {
            "SELECT id, name, repo, created_at, parent_wave_id, promoted_at,
                    retired_at, superseded_by_wave_id, retirement_reason
             FROM waves WHERE repo = ?1 AND retired_at IS NULL ORDER BY created_at DESC"
        } else {
            "SELECT id, name, repo, created_at, parent_wave_id, promoted_at,
                    retired_at, superseded_by_wave_id, retirement_reason
             FROM waves WHERE retired_at IS NULL ORDER BY created_at DESC"
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
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let created_at = wave
            .created_at()
            .map(|dt| dt.unix_timestamp())
            .unwrap_or_else(now_unix);

        tx.execute(
            "INSERT INTO waves (
                 id, name, repo, created_at, parent_wave_id, promoted_at,
                 retired_at, superseded_by_wave_id, retirement_reason
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(id) DO UPDATE SET
               parent_wave_id = COALESCE(waves.parent_wave_id, excluded.parent_wave_id),
               promoted_at = COALESCE(waves.promoted_at, excluded.promoted_at)",
            params![
                wave.id(),
                wave.name(),
                wave.repo(),
                created_at,
                wave.parent_wave_id(),
                wave.promoted_at().map(|at| at.unix_timestamp()),
                wave.retired_at().map(|at| at.unix_timestamp()),
                wave.superseded_by_wave_id(),
                wave.retirement_reason(),
            ],
        )?;
        durable::create_wave_spine(&tx, wave.id(), wave.name(), wave.repo(), created_at)?;
        tx.commit()?;
        Ok(())
    }
}

fn validate_run_events_schema(conn: &Connection) -> StoreResult<()> {
    conn.prepare(
        "SELECT run_id, process_id, parent_process_id, seq, ts, repo, worktree,
                wave, node, event, command, flow, skill, step_index, error
         FROM run_events LIMIT 0",
    )?;
    Ok(())
}

impl SqliteStore {
    pub fn health_check(&self) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row("SELECT 1", [], |_| Ok(()))?;
        super::migrations::validate_persisted_json(&conn)
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

    // -- Provider accounts -----------------------------------------------------

    pub fn upsert_provider_account(&self, account: &ProviderAccount) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO provider_accounts (
                provider, account_id, home, login_email, credential_state,
                routing_state, plan, paid_through, utilization_percent,
                cooldown_until, cooldown_reason, last_selected_at, created_at,
                updated_at
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                ?14
             )
             ON CONFLICT(provider, account_id) DO UPDATE SET
                home = excluded.home,
                login_email = excluded.login_email,
                credential_state = excluded.credential_state,
                routing_state = excluded.routing_state,
                plan = excluded.plan,
                paid_through = excluded.paid_through,
                utilization_percent = excluded.utilization_percent,
                cooldown_until = excluded.cooldown_until,
                cooldown_reason = excluded.cooldown_reason,
                last_selected_at = excluded.last_selected_at,
                updated_at = excluded.updated_at",
            params![
                account.provider,
                account.account_id.as_str(),
                account
                    .home
                    .as_ref()
                    .map(|path| path.to_string_lossy().to_string()),
                account.login_email.as_ref().map(EmailAddress::as_str),
                account.credential_state.as_str(),
                account.routing_state.as_str(),
                account.plan,
                account.paid_through.map(time::Date::to_julian_day),
                account.utilization_percent,
                account.cooldown_until,
                account.cooldown_reason,
                account.last_selected_at,
                account.created_at,
                account.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn get_provider_account(
        &self,
        provider: &str,
        account_id: &ProviderAccountId,
    ) -> StoreResult<Option<ProviderAccount>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT provider, account_id, home, login_email, credential_state,
                    routing_state, plan, paid_through, utilization_percent,
                    cooldown_until, cooldown_reason, last_selected_at,
                    created_at, updated_at
             FROM provider_accounts
             WHERE provider = ?1 AND account_id = ?2",
        )?;
        statement
            .query_row(
                params![provider, account_id.as_str()],
                read_provider_account,
            )
            .optional()?
            .transpose()
    }

    pub fn list_provider_accounts(
        &self,
        provider: Option<&str>,
    ) -> StoreResult<Vec<ProviderAccount>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let sql = match provider {
            Some(_) => {
                "SELECT provider, account_id, home, login_email, credential_state,
                        routing_state, plan, paid_through, utilization_percent,
                        cooldown_until, cooldown_reason, last_selected_at,
                        created_at, updated_at
                 FROM provider_accounts
                 WHERE provider = ?1
                 ORDER BY provider, account_id"
            }
            None => {
                "SELECT provider, account_id, home, login_email, credential_state,
                        routing_state, plan, paid_through, utilization_percent,
                        cooldown_until, cooldown_reason, last_selected_at,
                        created_at, updated_at
                 FROM provider_accounts
                 ORDER BY provider, account_id"
            }
        };
        let mut statement = conn.prepare(sql)?;
        let mut accounts = Vec::new();
        match provider {
            Some(provider) => {
                let rows = statement.query_map([provider], read_provider_account)?;
                for row in rows {
                    accounts.push(row??);
                }
            }
            None => {
                let rows = statement.query_map([], read_provider_account)?;
                for row in rows {
                    accounts.push(row??);
                }
            }
        }
        Ok(accounts)
    }

    pub fn update_provider_account_lifecycle(&self, account: &ProviderAccount) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let changed = conn.execute(
            "UPDATE provider_accounts
             SET login_email = ?3,
                 routing_state = ?4,
                 plan = ?5,
                 paid_through = ?6,
                 updated_at = ?7
             WHERE provider = ?1 AND account_id = ?2",
            params![
                account.provider,
                account.account_id.as_str(),
                account.login_email.as_ref().map(EmailAddress::as_str),
                account.routing_state.as_str(),
                account.plan,
                account.paid_through.map(time::Date::to_julian_day),
                account.updated_at,
            ],
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    pub fn reset_provider_account_health(
        &self,
        provider: &str,
        account_id: &ProviderAccountId,
    ) -> StoreResult<()> {
        self.record_provider_account_health(provider, account_id, None, None, None)
    }

    pub fn record_provider_account_credential_invalidated(
        &self,
        provider: &str,
        account_id: &ProviderAccountId,
        reason: &str,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let changed = conn.execute(
            "UPDATE provider_accounts
             SET credential_state = 'missing',
                 cooldown_until = NULL,
                 cooldown_reason = ?3,
                 updated_at = ?4
             WHERE provider = ?1 AND account_id = ?2",
            params![
                provider,
                account_id.as_str(),
                reason,
                time::OffsetDateTime::now_utc().unix_timestamp(),
            ],
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    pub fn record_provider_account_health(
        &self,
        provider: &str,
        account_id: &ProviderAccountId,
        utilization_percent: Option<u8>,
        cooldown_until: Option<i64>,
        cooldown_reason: Option<&str>,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let changed = conn.execute(
            "UPDATE provider_accounts
             SET utilization_percent = ?3,
                 cooldown_until = ?4,
                 cooldown_reason = ?5,
                 updated_at = ?6
             WHERE provider = ?1 AND account_id = ?2",
            params![
                provider,
                account_id.as_str(),
                utilization_percent,
                cooldown_until,
                cooldown_reason,
                now_unix(),
            ],
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    /// Record observed subscription window state for one account, replacing
    /// each window's previous observation.
    pub fn upsert_provider_account_limits(
        &self,
        provider: &str,
        account_id: &ProviderAccountId,
        windows: &[crate::store::AccountLimitWindow],
        source: &str,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let now = now_unix();
        for window in windows {
            conn.execute(
                "INSERT INTO provider_account_limits
                     (provider, account_id, window, used_percent, resets_at, plan, observed_at, source)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(provider, account_id, window) DO UPDATE SET
                     used_percent = excluded.used_percent,
                     resets_at = excluded.resets_at,
                     plan = excluded.plan,
                     observed_at = excluded.observed_at,
                     source = excluded.source",
                params![
                    provider,
                    account_id.as_str(),
                    window.window,
                    window.used_percent,
                    window.resets_at,
                    window.plan,
                    now,
                    source,
                ],
            )?;
        }
        Ok(())
    }

    pub fn provider_account_limits(
        &self,
        provider: Option<&str>,
    ) -> StoreResult<Vec<crate::store::AccountLimitRow>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT provider, account_id, window, used_percent, resets_at, plan, observed_at, source
             FROM provider_account_limits
             WHERE ?1 IS NULL OR provider = ?1
             ORDER BY provider, account_id, window",
        )?;
        let rows = statement.query_map([provider], read_account_limit_row)?;
        let mut limits = Vec::new();
        for row in rows {
            limits.push(row??);
        }
        Ok(limits)
    }

    // -- Access profiles and provider routes ----------------------------------

    pub fn upsert_access_profile(&self, profile: &AccessProfile) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO access_profiles (
                profile_id, chrome_directory, expected_login, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(profile_id) DO UPDATE SET
                chrome_directory = excluded.chrome_directory,
                expected_login = excluded.expected_login,
                updated_at = excluded.updated_at",
            params![
                profile.id.as_str(),
                profile.chrome_directory,
                profile.expected_login.as_str(),
                profile.created_at,
                profile.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn get_access_profile(&self, profile_id: &ProfileId) -> StoreResult<Option<AccessProfile>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            "SELECT profile_id, chrome_directory, expected_login, created_at, updated_at
             FROM access_profiles WHERE profile_id = ?1",
            [profile_id.as_str()],
            read_access_profile,
        )
        .optional()?
        .transpose()
    }

    pub fn list_access_profiles(&self) -> StoreResult<Vec<AccessProfile>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT profile_id, chrome_directory, expected_login, created_at, updated_at
             FROM access_profiles ORDER BY profile_id",
        )?;
        let rows = statement.query_map([], read_access_profile)?;
        rows.map(|row| row?).collect()
    }

    pub fn set_account_access_profiles(
        &self,
        provider: Provider,
        account_id: &ProviderAccountId,
        profile_ids: &[ProfileId],
    ) -> StoreResult<()> {
        let unique = profile_ids.iter().collect::<std::collections::HashSet<_>>();
        if unique.len() != profile_ids.len() {
            return Err(StoreError::InvalidData(
                "account access profiles must be unique".to_string(),
            ));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute(
            "DELETE FROM account_access_profiles WHERE provider = ?1 AND account_id = ?2",
            params![provider.as_str(), account_id.as_str()],
        )?;
        for (position, profile_id) in profile_ids.iter().enumerate() {
            transaction.execute(
                "INSERT INTO account_access_profiles (
                    provider, account_id, position, profile_id
                 ) VALUES (?1, ?2, ?3, ?4)",
                params![
                    provider.as_str(),
                    account_id.as_str(),
                    position as i64,
                    profile_id.as_str(),
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn list_account_access_profiles(
        &self,
        provider: Option<Provider>,
        account_id: Option<&ProviderAccountId>,
    ) -> StoreResult<Vec<AccountAccessProfile>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let provider = provider.map(|value| value.as_str());
        let account_id = account_id.map(ProviderAccountId::as_str);
        let mut statement = conn.prepare(
            "SELECT provider, account_id, position, profile_id
             FROM account_access_profiles
             WHERE (?1 IS NULL OR provider = ?1)
               AND (?2 IS NULL OR account_id = ?2)
             ORDER BY provider, account_id, position",
        )?;
        let rows =
            statement.query_map(params![provider, account_id], read_account_access_profile)?;
        rows.map(|row| row?).collect()
    }

    pub fn set_provider_route(&self, route: &ProviderRoute) -> StoreResult<()> {
        if route.accounts.is_empty() {
            return Err(StoreError::InvalidData(
                "provider route needs at least one account".to_string(),
            ));
        }
        let unique = route
            .accounts
            .iter()
            .collect::<std::collections::HashSet<_>>();
        if unique.len() != route.accounts.len() {
            return Err(StoreError::InvalidData(
                "provider route accounts must be unique".to_string(),
            ));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute(
            "DELETE FROM provider_routes
             WHERE scope = ?1 AND scope_id = ?2 AND provider = ?3",
            params![
                route.scope.kind(),
                route.scope.id(),
                route.provider.as_str()
            ],
        )?;
        for (position, account_id) in route.accounts.iter().enumerate() {
            transaction.execute(
                "INSERT INTO provider_routes (
                    scope, scope_id, provider, position, account_id, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    route.scope.kind(),
                    route.scope.id(),
                    route.provider.as_str(),
                    position as i64,
                    account_id.as_str(),
                    route.created_at,
                    route.updated_at,
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn provider_route(
        &self,
        scope: &RouteScope,
        provider: Provider,
    ) -> StoreResult<Option<ProviderRoute>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT account_id, created_at, updated_at
             FROM provider_routes
             WHERE scope = ?1 AND scope_id = ?2 AND provider = ?3
             ORDER BY position",
        )?;
        let rows = statement.query_map(
            params![scope.kind(), scope.id(), provider.as_str()],
            |row| {
                Ok((
                    read_provider_route_account(row)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )?;
        let mut accounts = Vec::new();
        let mut created_at = 0;
        let mut updated_at = 0;
        for row in rows {
            let (account_id, row_created_at, row_updated_at) = row?;
            accounts.push(account_id);
            created_at = if created_at == 0 {
                row_created_at
            } else {
                created_at.min(row_created_at)
            };
            updated_at = updated_at.max(row_updated_at);
        }
        Ok((!accounts.is_empty()).then(|| ProviderRoute {
            scope: scope.clone(),
            provider,
            accounts,
            created_at,
            updated_at,
        }))
    }

    pub fn pin_provider_session_route(
        &self,
        provider: Provider,
        provider_session_id: &str,
        account_id: &ProviderAccountId,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO provider_session_accounts (
                provider, provider_session_id, account_id, created_at
             ) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(provider, provider_session_id) DO UPDATE SET
                account_id = excluded.account_id,
                created_at = excluded.created_at",
            params![
                provider.as_str(),
                provider_session_id,
                account_id.as_str(),
                now_unix(),
            ],
        )?;
        Ok(())
    }

    pub fn provider_session_account(
        &self,
        provider: Provider,
        provider_session_id: &str,
    ) -> StoreResult<Option<ProviderAccountId>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            "SELECT account_id FROM provider_session_accounts
             WHERE provider = ?1 AND provider_session_id = ?2",
            params![provider.as_str(), provider_session_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .as_deref()
        .map(ProviderAccountId::parse)
        .transpose()
        .map_err(StoreError::InvalidData)
    }

    pub fn select_provider_account(
        &self,
        provider: Provider,
        candidates: &[ProviderAccountId],
        provider_session_id: Option<&str>,
    ) -> StoreResult<Option<ProviderAccountSelection>> {
        if candidates.is_empty() {
            return Ok(None);
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let now = now_unix();
        let today = time::OffsetDateTime::now_utc().date();
        let newest_selection = transaction.query_row(
            "SELECT COALESCE(MAX(last_selected_at), 0)
             FROM provider_accounts WHERE provider = ?1",
            [provider.as_str()],
            |row| row.get::<_, i64>(0),
        )?;
        let selection_time = now.max(newest_selection + 1);
        let requested = match provider_session_id {
            Some(session_id) => transaction
                .query_row(
                    "SELECT account_id FROM provider_session_accounts
                     WHERE provider = ?1 AND provider_session_id = ?2",
                    params![provider.as_str(), session_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()?,
            None => None,
        };
        let limits = {
            let mut statement = transaction.prepare(
                "SELECT provider, account_id, window, used_percent, resets_at, plan, observed_at, source
                 FROM provider_account_limits
                 WHERE provider = ?1
                 ORDER BY account_id, window",
            )?;
            let rows = statement.query_map([provider.as_str()], read_account_limit_row)?;
            let mut limits = Vec::new();
            for row in rows {
                limits.push(row??);
            }
            limits
        };
        let mut account_statement = transaction.prepare(
            "SELECT provider, account_id, home, login_email, credential_state,
                    routing_state, plan, paid_through, utilization_percent,
                    cooldown_until, cooldown_reason, last_selected_at,
                    created_at, updated_at
             FROM provider_accounts
             WHERE provider = ?1 AND account_id = ?2",
        )?;
        let mut available = Vec::new();
        for account_id in candidates {
            let account = account_statement
                .query_row(
                    params![provider.as_str(), account_id.as_str()],
                    read_provider_account,
                )
                .optional()?
                .transpose()?;
            if let Some(account) = account.filter(|account| {
                account.eligible_for_automatic_routing(today)
                    && account.cooldown_until.is_none_or(|until| until <= now)
            }) {
                available.push(account);
            }
        }
        drop(account_statement);

        let resumed = requested.as_ref().and_then(|account_id| {
            available
                .iter()
                .position(|account| account.account_id.as_str() == account_id)
        });
        let (mut account, resume_requested_session) = match resumed {
            Some(index) => (available.remove(index), true),
            None => {
                crate::provider_account::order_accounts_by_strain(&mut available, &limits, now);
                match available.into_iter().next() {
                    Some(selection) => (selection, false),
                    None => {
                        transaction.commit()?;
                        return Ok(None);
                    }
                }
            }
        };
        transaction.execute(
            "UPDATE provider_accounts
             SET last_selected_at = ?3, updated_at = ?3
             WHERE provider = ?1 AND account_id = ?2",
            params![
                provider.as_str(),
                account.account_id.as_str(),
                selection_time
            ],
        )?;
        transaction.commit()?;
        account.last_selected_at = Some(selection_time);
        account.updated_at = selection_time;
        Ok(Some(ProviderAccountSelection {
            account,
            resume_requested_session,
        }))
    }

    pub fn list_waves(&self, repo: Option<&str>) -> StoreResult<Vec<Wave>> {
        self.read_waves(repo)
    }

    pub fn list_child_waves(&self, parent: &WaveId) -> StoreResult<Vec<Wave>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, name, repo, created_at, parent_wave_id, promoted_at,
                    retired_at, superseded_by_wave_id, retirement_reason
             FROM waves
             WHERE parent_wave_id = ?1 AND retired_at IS NULL
             ORDER BY created_at ASC",
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
            "SELECT id, name, repo, created_at, parent_wave_id, promoted_at,
                    retired_at, superseded_by_wave_id, retirement_reason
             FROM waves WHERE id = ?1",
        )?;
        let wave = stmt
            .query_row(params![wave_id], |row| Ok(map_wave_row(row)))
            .optional()?;
        wave.transpose()
    }

    pub fn get_wave_at(&self, locator: &WaveLocator) -> StoreResult<Option<Wave>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, name, repo, created_at, parent_wave_id, promoted_at,
                    retired_at, superseded_by_wave_id, retirement_reason
             FROM waves
             WHERE repo = ?1 AND name = ?2 AND retired_at IS NULL",
        )?;
        let wave = stmt
            .query_row(params![locator.repo().to_string(), locator.slug()], |row| {
                Ok(map_wave_row(row))
            })
            .optional()?;
        wave.transpose()
    }

    pub(crate) fn repair_wave_repo(
        &self,
        wave_id: &WaveId,
        expected_repo: &str,
        target_repo: &str,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = tx
            .query_row(
                "SELECT repo, name FROM waves WHERE id = ?1",
                params![wave_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?
            .ok_or_else(|| StoreError::InvalidData(format!("Wave {wave_id} is not registered")))?;
        if current.0 == target_repo {
            tx.commit()?;
            return Ok(());
        }
        if current.0 != expected_repo {
            return Err(StoreError::InvalidData(format!(
                "Wave {wave_id} repository changed from {expected_repo} while its canonical path was being repaired"
            )));
        }
        let collision = tx
            .query_row(
                "SELECT id FROM waves
                 WHERE repo = ?1 AND name = ?2 AND id != ?3 AND retired_at IS NULL",
                params![target_repo, current.1, wave_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        if let Some(collision) = collision {
            return Err(StoreError::InvalidData(format!(
                "cannot repair Wave {wave_id} repository to {target_repo}: locator belongs to Wave {collision}"
            )));
        }
        tx.execute(
            "UPDATE waves SET repo = ?2 WHERE id = ?1 AND repo = ?3",
            params![wave_id, target_repo, expected_repo],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn find_waves_by_slug(&self, slug: &str) -> StoreResult<Vec<Wave>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, name, repo, created_at, parent_wave_id, promoted_at,
                    retired_at, superseded_by_wave_id, retirement_reason
             FROM waves
             WHERE name = ?1 AND retired_at IS NULL
             ORDER BY repo",
        )?;
        let rows = stmt.query_map(params![slug], |row| Ok(map_wave_row(row)))?;
        let mut waves = Vec::new();
        for wave in rows {
            waves.push(wave??);
        }
        Ok(waves)
    }

    pub fn create_wave(&self, wave: &Wave) -> StoreResult<()> {
        self.upsert_wave(wave)
    }

    pub fn update_wave(&self, wave: &Wave) -> StoreResult<()> {
        self.upsert_wave(wave)
    }

    pub(crate) fn relocate_waves(&self, updates: &[WaveLocatorUpdate]) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        for update in updates {
            let current = tx
                .query_row(
                    "SELECT repo, name FROM waves WHERE id = ?1 AND retired_at IS NULL",
                    params![update.wave_id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .optional()?
                .ok_or_else(|| {
                    StoreError::InvalidData(format!("Wave {} is not registered", update.wave_id))
                })?;
            if current != (update.expected_repo.clone(), update.expected_slug.clone()) {
                return Err(StoreError::InvalidData(format!(
                    "Wave {} moved from {}/{} while relocation was staged",
                    update.wave_id, update.expected_repo, update.expected_slug
                )));
            }

            let collision = tx
                .query_row(
                    "SELECT id FROM waves
                     WHERE repo = ?1 AND name = ?2 AND id != ?3
                       AND retired_at IS NULL",
                    params![
                        update.target.repo().to_string(),
                        update.target.slug(),
                        update.wave_id
                    ],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            if collision.as_deref()
                != update
                    .retire_collision
                    .as_ref()
                    .map(crate::id::WaveId::as_str)
            {
                return Err(StoreError::InvalidData(format!(
                    "target {}/{} collision changed while relocation was staged",
                    update.target.repo(),
                    update.target.slug()
                )));
            }
            if let Some(collision) = &update.retire_collision {
                let blockers = Self::wave_retirement_blockers_in(&tx, collision)?;
                if !blockers.is_empty() {
                    return Err(StoreError::InvalidData(format!(
                        "cannot retire destination Wave {collision}: {}",
                        blockers.join(", ")
                    )));
                }
            }

            let active_run = tx
                .query_row(
                    "SELECT r.id
                     FROM runs r
                     JOIN epochs e ON e.id = r.epoch_id
                     LEFT JOIN projects ep ON ep.id = e.project_id
                     LEFT JOIN tasks et ON et.id = e.task_id
                     LEFT JOIN projects tp ON tp.id = et.project_id
                     WHERE r.state != 'ended'
                       AND (e.wave_id = ?1 OR ep.wave_id = ?1 OR tp.wave_id = ?1)
                     LIMIT 1",
                    params![update.wave_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            if let Some(run_id) = active_run {
                return Err(StoreError::InvalidData(format!(
                    "cannot relocate Wave {} while Run {run_id} is active",
                    update.wave_id
                )));
            }
        }

        for update in updates {
            if let Some(collision) = &update.retire_collision {
                let retired_at = now_unix();
                tx.execute(
                    "UPDATE epochs
                     SET state = 'abandoned', terminal_at = ?2
                     WHERE wave_id = ?1 AND state = 'open'",
                    params![collision, retired_at],
                )?;
                tx.execute(
                    "UPDATE work_placements SET enabled = 0 WHERE wave_id = ?1",
                    params![collision],
                )?;
                tx.execute(
                    "UPDATE waves
                     SET retired_at = ?2,
                         superseded_by_wave_id = ?3,
                         retirement_reason = ?4
                     WHERE id = ?1 AND retired_at IS NULL",
                    params![
                        collision,
                        retired_at,
                        update.wave_id,
                        "registration-only destination shadow retired during relocation"
                    ],
                )?;
            }
            tx.execute(
                "UPDATE waves SET repo = ?2, name = ?3 WHERE id = ?1",
                params![
                    update.wave_id,
                    update.target.repo().to_string(),
                    update.target.slug()
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn wave_retirement_blockers(&self, wave_id: &WaveId) -> StoreResult<Vec<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Self::wave_retirement_blockers_in(&conn, wave_id)
    }

    fn wave_retirement_blockers_in(
        conn: &Connection,
        wave_id: &WaveId,
    ) -> StoreResult<Vec<String>> {
        let mut blockers = Vec::new();
        let epochs = conn
            .prepare(
                "SELECT id, number, state, current_rev
                 FROM epochs WHERE wave_id = ?1 ORDER BY number",
            )?
            .query_map(params![wave_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        if epochs.len() != 1 {
            blockers.push(format!("{} Wave Epochs", epochs.len()));
        }
        if let Some((epoch_id, number, state, revision)) = epochs.first() {
            if *number != 1 || state != "open" {
                blockers.push(format!("Epoch {number} is {state}"));
            }
            if *revision != 0 {
                blockers.push(format!("Truth revision is {revision}"));
            }
            let steers: i64 = conn.query_row(
                "SELECT COUNT(*) FROM steers WHERE epoch_id = ?1",
                params![epoch_id],
                |row| row.get(0),
            )?;
            if steers > 0 {
                blockers.push(format!("{steers} authored Steers"));
            }
            let truths: i64 = conn.query_row(
                "SELECT COUNT(*) FROM work_truth WHERE epoch_id = ?1",
                params![epoch_id],
                |row| row.get(0),
            )?;
            if truths != 1 {
                blockers.push(format!("{truths} Truth revisions"));
            }
            let runs: i64 = conn.query_row(
                "SELECT COUNT(*) FROM runs WHERE epoch_id = ?1",
                params![epoch_id],
                |row| row.get(0),
            )?;
            if runs > 0 {
                blockers.push(format!("{runs} Runs"));
            }
        }
        let projects: i64 = conn.query_row(
            "SELECT COUNT(*) FROM projects WHERE wave_id = ?1",
            params![wave_id],
            |row| row.get(0),
        )?;
        if projects > 0 {
            blockers.push(format!("{projects} Projects"));
        }
        let tasks: i64 = conn.query_row(
            "SELECT COUNT(*)
             FROM tasks JOIN projects ON projects.id = tasks.project_id
             WHERE projects.wave_id = ?1",
            params![wave_id],
            |row| row.get(0),
        )?;
        if tasks > 0 {
            blockers.push(format!("{tasks} Tasks"));
        }
        let children: i64 = conn.query_row(
            "SELECT COUNT(*) FROM waves
             WHERE parent_wave_id = ?1 AND retired_at IS NULL",
            params![wave_id],
            |row| row.get(0),
        )?;
        if children > 0 {
            blockers.push(format!("{children} child Waves"));
        }
        let snapshots: i64 = conn.query_row(
            "SELECT COUNT(*) FROM pm_snapshots WHERE wave_id = ?1",
            params![wave_id],
            |row| row.get(0),
        )?;
        if snapshots > 0 {
            blockers.push("PM snapshot".to_string());
        }
        let promoted = conn
            .query_row(
                "SELECT promoted_at FROM waves WHERE id = ?1",
                params![wave_id],
                |row| row.get::<_, Option<i64>>(0),
            )
            .optional()?
            .flatten();
        if promoted.is_some() {
            blockers.push("promotion receipt".to_string());
        }
        Ok(blockers)
    }

    pub fn delete_wave(&self, wave_id: &WaveId) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute("DELETE FROM epochs WHERE wave_id = ?1", params![wave_id])?;
        tx.execute("DELETE FROM waves WHERE id = ?1", params![wave_id])?;
        tx.commit()?;
        Ok(())
    }

    // Exec ledger (`run_events`): the machine-grain, append-only record of
    // every process written directly by `lf`. Read by `lf execs` / `lf trace`;
    // `lf runs` joins it to agent invocations for process lineage.

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
                flow, skill, step_index, error
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
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
            ],
        )?;
        Ok(())
    }

    /// Every provider-measured Turn's latest usage since `since_unix`,
    /// attributed by the invocation that ran it.
    ///
    /// Turns with no provider report at all are dropped: they carry no spend to
    /// aggregate, and keeping them would let a reader mistake silence for zero. Any
    /// one measurement is enough to keep the turn — a report of cache reads
    /// alone is still something the provider measured.
    pub(crate) fn attributed_turn_usage_since(
        &self,
        since_unix: i64,
    ) -> StoreResult<Vec<AttributedTurnUsage>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT t.id, l.id, l.process_id, l.repo, l.wave, l.flow, l.skill,
                    l.provider, COALESCE(u.model, l.model), u.observed_at, u.input_tokens,
                    u.total_input_tokens, u.peak_input_tokens, u.context_window_tokens,
                    u.output_tokens, u.reasoning_tokens, u.cache_read_tokens,
                    u.cache_write_tokens, u.cost_usd
             FROM turn_usage_samples u
             JOIN agent_turns t ON t.id = u.turn_id
             JOIN agent_invocations l ON l.id = t.invocation_id
             WHERE u.observed_at = (
                    SELECT MAX(latest.observed_at)
                    FROM turn_usage_samples latest
                    WHERE latest.turn_id = u.turn_id
                )
               AND u.observed_at >= ?1
             ORDER BY u.observed_at, l.process_id, t.ordinal",
        )?;
        let rows = stmt.query_map(params![since_unix], |row| {
            let model: Option<String> = row.get(8)?;
            let usage = map_turn_usage(row, 10, model)?.ok_or_else(|| {
                rusqlite::Error::InvalidColumnType(
                    10,
                    "turn_usage_samples".to_string(),
                    rusqlite::types::Type::Null,
                )
            })?;
            Ok(AttributedTurnUsage {
                turn_id: row.get(0)?,
                invocation_id: row.get(1)?,
                exec_id: row.get(2)?,
                repo: row.get(3)?,
                wave: row.get(4)?,
                flow: row.get(5)?,
                skill: row.get(6)?,
                provider: row.get(7)?,
                at: row.get(9)?,
                usage,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(StoreError::from)
    }

    /// Record one cumulative provider usage checkpoint. Providers may update
    /// a checkpoint within the same second; the last value wins. Across
    /// checkpoints token counters cannot decrease, and a final receipt cannot
    /// become provisional again.
    pub fn record_turn_usage_sample(&self, sample: &TurnUsageSample) -> StoreResult<()> {
        if !sample.usage.is_reported() {
            return Err(StoreError::InvalidData(
                "provider emitted an empty usage checkpoint".to_string(),
            ));
        }
        if let (Some(reasoning), Some(output)) =
            (sample.usage.reasoning_tokens, sample.usage.output_tokens)
        {
            if reasoning > output {
                return Err(StoreError::InvalidData(
                    "reasoning tokens exceed inclusive output tokens".to_string(),
                ));
            }
        }

        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let previous = latest_turn_usage_sample_in(&transaction, &sample.turn_id)?;
        if let Some(previous) = previous.as_ref() {
            validate_usage_progress(previous, sample)?;
        }
        let usage = &sample.usage;
        transaction.execute(
            "INSERT INTO turn_usage_samples (
                turn_id, observed_at, final_receipt, input_tokens, total_input_tokens,
                peak_input_tokens, context_window_tokens, output_tokens, reasoning_tokens,
                cache_read_tokens, cache_write_tokens, model, cost_usd
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
             ON CONFLICT(turn_id, observed_at) DO UPDATE SET
                final_receipt=excluded.final_receipt,
                input_tokens=excluded.input_tokens,
                total_input_tokens=excluded.total_input_tokens,
                peak_input_tokens=excluded.peak_input_tokens,
                context_window_tokens=excluded.context_window_tokens,
                output_tokens=excluded.output_tokens,
                reasoning_tokens=excluded.reasoning_tokens,
                cache_read_tokens=excluded.cache_read_tokens,
                cache_write_tokens=excluded.cache_write_tokens,
                model=excluded.model,
                cost_usd=excluded.cost_usd",
            params![
                sample.turn_id,
                sample.observed_at,
                sample.final_receipt,
                to_sql_i64(usage.input_tokens, "input_tokens")?,
                to_sql_i64(usage.total_input_tokens, "total_input_tokens")?,
                to_sql_i64(usage.peak_input_tokens, "peak_input_tokens")?,
                to_sql_i64(usage.context_window_tokens, "context_window_tokens")?,
                to_sql_i64(usage.output_tokens, "output_tokens")?,
                to_sql_i64(usage.reasoning_tokens, "reasoning_tokens")?,
                to_sql_i64(usage.cache_read_tokens, "cache_read_tokens")?,
                to_sql_i64(usage.cache_write_tokens, "cache_write_tokens")?,
                usage.model,
                usage.cost_usd,
            ],
        )?;
        if sample.final_receipt {
            transaction.execute(
                "DELETE FROM turn_usage_samples AS stale
                 WHERE stale.final_receipt = 0
                   AND stale.observed_at < ?1
                   AND EXISTS (
                       SELECT 1
                       FROM turn_usage_samples AS newer
                       WHERE newer.turn_id = stale.turn_id
                         AND newer.observed_at > stale.observed_at
                   )",
                [sample.observed_at - crate::store::TURN_USAGE_LIVE_RETENTION_SECONDS],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// Usage checkpoints at or after `since_unix`, plus the last checkpoint
    /// before the boundary for every continuing Turn. The preceding row is a
    /// baseline only; it prevents a window from claiming earlier output.
    pub fn turn_usage_samples_since(&self, since_unix: i64) -> StoreResult<Vec<TurnUsageSample>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT turn_id, observed_at, final_receipt, input_tokens,
                    total_input_tokens, peak_input_tokens, context_window_tokens,
                    output_tokens, reasoning_tokens, cache_read_tokens,
                    cache_write_tokens, model, cost_usd
             FROM turn_usage_samples sample
             WHERE observed_at >= ?1
                OR observed_at = (
                    SELECT MAX(baseline.observed_at)
                    FROM turn_usage_samples baseline
                    WHERE baseline.turn_id = sample.turn_id
                      AND baseline.observed_at < ?1
                )
             ORDER BY turn_id, observed_at",
        )?;
        let rows = statement.query_map([since_unix], map_turn_usage_sample)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(StoreError::from)
    }

    pub fn list_run_events_since(&self, since_unix: i64) -> StoreResult<Vec<RunEventRow>> {
        self.query_run_events(
            "SELECT run_id, process_id, parent_process_id, seq, ts, repo, worktree, wave, node, event, command,
                    flow, skill, step_index, error
             FROM run_events WHERE ts >= ?1 ORDER BY ts, run_id, seq",
            params![since_unix],
        )
    }

    /// Whether this ledger holds any row for `process_id`. A run start asks
    /// before honoring an inherited parent: a parent this ledger never
    /// recorded cannot be pointed at, only inherited from.
    pub fn process_is_recorded(&self, process_id: &str) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare("SELECT 1 FROM run_events WHERE process_id = ?1 LIMIT 1")?;
        Ok(stmt.exists(params![process_id])?)
    }

    /// Events for one trace; the persisted `run_id` may be a unique prefix.
    pub fn run_events_matching(&self, run_id: &str) -> StoreResult<Vec<RunEventRow>> {
        let prefix = format!("{}%", run_id.replace(['%', '_'], ""));
        self.query_run_events(
            "SELECT run_id, process_id, parent_process_id, seq, ts, repo, worktree, wave, node, event, command,
                    flow, skill, step_index, error
             FROM run_events WHERE run_id LIKE ?1 ORDER BY ts, seq",
            params![prefix],
        )
    }

    /// Events identifying one exec by process-id prefix. The caller resolves
    /// its trace, then reads that trace whole.
    pub fn run_events_matching_exec(&self, exec_id: &str) -> StoreResult<Vec<RunEventRow>> {
        let (operator, value) = exact_or_prefix(exec_id);
        self.query_run_events(
            &format!(
                "SELECT run_id, process_id, parent_process_id, seq, ts, repo, worktree, wave, node, event, command,
                    flow, skill, step_index, error
             FROM run_events WHERE process_id {operator} ?1 ORDER BY ts, seq"
            ),
            params![value],
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
        invocation: &AgentInvocationRow,
        turn: &AgentTurnRow,
        assets: &[ContextAssetRow],
        decisions: &[ContextDecisionRow],
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (supervising_run_id, account_id, resume_token) = invocation
            .supervision
            .as_ref()
            .map(|supervision| {
                (
                    Some(supervision.supervising_run_id.as_str()),
                    supervision.account_id.as_deref(),
                    supervision.resume_token.as_deref(),
                )
            })
            .unwrap_or((None, None, None));
        let registered = supervising_run_id.is_some()
            && tx.query_row(
                "SELECT EXISTS(
                        SELECT 1 FROM agent_invocations
                        WHERE id=?1 AND supervising_run_id=?2
                     )",
                params![invocation.id, supervising_run_id],
                |row| row.get::<_, bool>(0),
            )?;
        if registered {
            tx.execute(
                "UPDATE agent_invocations SET
                    run_id=?2, process_id=?3, repo=?4, worktree=?5, wave=?6,
                    flow=?7, skill=?8, project=?9, task=?10, provider=?11,
                    model=?12, surface=?13, capture_status=?14,
                    incomplete_reason=?15, outcome=?16, artifact_dir=?17,
                    conversation_path=?18, provider_events_path=?19,
                    provider_session_id=?20, provider_session_path=?21,
                    conversation_event_count=?22, conversation_bytes=?23
                 WHERE id=?1 AND supervising_run_id=?24",
                params![
                    invocation.id,
                    invocation.run_id,
                    invocation.process_id,
                    invocation.repo,
                    invocation.worktree,
                    invocation.wave,
                    invocation.flow,
                    invocation.skill,
                    invocation.project,
                    invocation.task,
                    invocation.provider,
                    invocation.model,
                    invocation.surface,
                    invocation.capture_status,
                    invocation.incomplete_reason,
                    invocation.outcome,
                    invocation.artifact_dir,
                    invocation.conversation_path,
                    invocation.provider_events_path,
                    invocation.provider_session_id,
                    invocation.provider_session_path,
                    invocation.conversation_event_count,
                    invocation.conversation_bytes,
                    supervising_run_id,
                ],
            )?;
        } else {
            tx.execute(
                "INSERT INTO agent_invocations (
                    id, run_id, process_id, started_at, ended_at, repo, worktree, wave, flow,
                    skill, project, task, provider, model, surface, capture_status,
                    incomplete_reason, outcome, artifact_dir, conversation_path,
                    provider_events_path, provider_session_id, provider_session_path,
                    conversation_event_count, conversation_bytes, supervising_run_id,
                    account_id, resume_token, answer_ask_id
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                    ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25,
                    ?26, ?27, ?28, ?29)",
                params![
                    invocation.id,
                    invocation.run_id,
                    invocation.process_id,
                    invocation.started_at,
                    invocation.ended_at,
                    invocation.repo,
                    invocation.worktree,
                    invocation.wave,
                    invocation.flow,
                    invocation.skill,
                    invocation.project,
                    invocation.task,
                    invocation.provider,
                    invocation.model,
                    invocation.surface,
                    invocation.capture_status,
                    invocation.incomplete_reason,
                    invocation.outcome,
                    invocation.artifact_dir,
                    invocation.conversation_path,
                    invocation.provider_events_path,
                    invocation.provider_session_id,
                    invocation.provider_session_path,
                    invocation.conversation_event_count,
                    invocation.conversation_bytes,
                    supervising_run_id,
                    account_id,
                    resume_token,
                    invocation.answer_ask_id,
                ],
            )?;
        }
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
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        insert_agent_turn(&tx, turn)?;
        insert_context_rows(&tx, assets, decisions)?;
        tx.commit()?;
        Ok(())
    }

    pub fn finish_agent_turn_capture(&self, turn: &AgentTurnRow) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        update_agent_turn(&tx, turn)?;
        tx.commit()?;
        Ok(())
    }

    pub fn update_agent_invocation_receipt(
        &self,
        invocation: &AgentInvocationRow,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE agent_invocations
             SET provider_session_id = ?2, provider_session_path = ?3,
                 resume_token=CASE WHEN supervising_run_id IS NULL THEN resume_token ELSE ?2 END
             WHERE id = ?1",
            params![
                invocation.id,
                invocation.provider_session_id,
                invocation.provider_session_path,
            ],
        )?;
        Ok(())
    }

    pub fn finish_trace_capture(
        &self,
        invocation: &AgentInvocationRow,
        turn: &AgentTurnRow,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "UPDATE agent_invocations SET
                ended_at = ?2, capture_status = ?3, incomplete_reason = ?4, outcome = ?5,
                conversation_event_count = ?6, conversation_bytes = ?7,
                provider_session_id = ?8, provider_session_path = ?9,
                handback_state=CASE
                    WHEN supervising_run_id IS NULL THEN handback_state
                    WHEN ?5='completed' THEN 'succeeded'
                    WHEN ?5='interrupted' THEN 'interrupted'
                    ELSE 'failed'
                END,
                resume_token=CASE WHEN supervising_run_id IS NULL THEN resume_token ELSE ?8 END
             WHERE id = ?1",
            params![
                invocation.id,
                invocation.ended_at,
                invocation.capture_status,
                invocation.incomplete_reason,
                invocation.outcome,
                invocation.conversation_event_count,
                invocation.conversation_bytes,
                invocation.provider_session_id,
                invocation.provider_session_path,
            ],
        )?;
        update_agent_turn(&tx, turn)?;
        tx.commit()?;
        Ok(())
    }

    /// Record that an invocation's referenced conversation artifact is known absent.
    /// If capture loss also exposed an unclosed owner, close the invocation and its
    /// running Turns in the same transaction.
    pub fn prune_invocation_capture(
        &self,
        invocation_id: &str,
        incomplete_reason: &str,
        ended_at_fallback: i64,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let updated = tx.execute(
            "UPDATE agent_invocations
             SET capture_status = 'pruned', incomplete_reason = ?2,
                 ended_at = COALESCE(ended_at, ?3),
                 outcome = CASE WHEN outcome = 'running' THEN 'interrupted' ELSE outcome END
             WHERE id = ?1 AND capture_status != 'pruned'",
            params![invocation_id, incomplete_reason, ended_at_fallback],
        )?;
        if updated != 1 {
            return Err(StoreError::InvalidData(format!(
                "capture {invocation_id} is already pruned or missing"
            )));
        }
        tx.execute(
            "UPDATE agent_turns
             SET status = 'interrupted', ended_at = COALESCE(ended_at, ?2)
             WHERE invocation_id = ?1 AND status = 'running'",
            params![invocation_id, ended_at_fallback],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Close an intact capture whose owner ended before capture finalization.
    pub fn interrupt_invocation_capture(
        &self,
        invocation_id: &str,
        incomplete_reason: &str,
        ended_at_fallback: i64,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let updated = tx.execute(
            "UPDATE agent_invocations
             SET capture_status = 'interrupted', incomplete_reason = ?2,
                 ended_at = COALESCE(ended_at, ?3), outcome = 'interrupted'
             WHERE id = ?1 AND capture_status = 'capturing'",
            params![invocation_id, incomplete_reason, ended_at_fallback],
        )?;
        if updated != 1 {
            return Err(StoreError::InvalidData(format!(
                "capture {invocation_id} is no longer capturing"
            )));
        }
        tx.execute(
            "UPDATE agent_turns
             SET status = 'interrupted', ended_at = COALESCE(ended_at, ?2)
             WHERE invocation_id = ?1 AND status = 'running'",
            params![invocation_id, ended_at_fallback],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Acknowledge an aged intact capture write loss without discarding its
    /// original `incomplete_reason` or artifact references.
    pub fn lose_invocation_capture(
        &self,
        invocation_id: &str,
        ended_at_fallback: i64,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let updated = tx.execute(
            "UPDATE agent_invocations
             SET capture_status = 'lost', ended_at = COALESCE(ended_at, ?2),
                 outcome = CASE WHEN outcome = 'running' THEN 'interrupted' ELSE outcome END
             WHERE id = ?1 AND capture_status = 'partial'",
            params![invocation_id, ended_at_fallback],
        )?;
        if updated != 1 {
            return Err(StoreError::InvalidData(format!(
                "capture {invocation_id} is no longer partial"
            )));
        }
        tx.execute(
            "UPDATE agent_turns
             SET status = 'interrupted', ended_at = COALESCE(ended_at, ?2)
             WHERE invocation_id = ?1 AND status = 'running'",
            params![invocation_id, ended_at_fallback],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn agent_invocations_matching(&self, run_id: &str) -> StoreResult<Vec<AgentInvocationRow>> {
        let (operator, value) = exact_or_prefix(run_id);
        // Invocation timestamps use ledger-second precision. rowid preserves the
        // append order when a fast flow starts several agents in one second.
        self.query_agent_invocations(
            &format!(
                "SELECT id, run_id, process_id, started_at, ended_at, repo, worktree, wave, flow,
                    skill, project, task, provider, model, surface, capture_status,
                    incomplete_reason, outcome, artifact_dir, conversation_path,
                    provider_events_path, provider_session_id, provider_session_path,
                    conversation_event_count, conversation_bytes, supervising_run_id,
                    account_id, resume_token, answer_ask_id
             FROM agent_invocations WHERE run_id {operator} ?1 ORDER BY started_at, rowid"
            ),
            params![value],
        )
    }

    pub fn agent_invocations_since(&self, since: i64) -> StoreResult<Vec<AgentInvocationRow>> {
        self.query_agent_invocations(
            "SELECT id, run_id, process_id, started_at, ended_at, repo, worktree, wave, flow,
                    skill, project, task, provider, model, surface, capture_status,
                    incomplete_reason, outcome, artifact_dir, conversation_path,
                    provider_events_path, provider_session_id, provider_session_path,
                    conversation_event_count, conversation_bytes, supervising_run_id,
                    account_id, resume_token, answer_ask_id
             FROM agent_invocations WHERE started_at >= ?1 ORDER BY started_at, rowid",
            params![since],
        )
    }

    /// Invocations whose lifecycle reaches `since`, including an invocation
    /// that started earlier and is still open. Windowed usage readers need the
    /// latter because its provider receipts can arrive inside the window.
    pub fn agent_invocations_overlapping_since(
        &self,
        since: i64,
    ) -> StoreResult<Vec<AgentInvocationRow>> {
        self.query_agent_invocations(
            "SELECT id, run_id, process_id, started_at, ended_at, repo, worktree, wave, flow,
                    skill, project, task, provider, model, surface, capture_status,
                    incomplete_reason, outcome, artifact_dir, conversation_path,
                    provider_events_path, provider_session_id, provider_session_path,
                    conversation_event_count, conversation_bytes, supervising_run_id,
                    account_id, resume_token, answer_ask_id
             FROM agent_invocations
             WHERE ended_at IS NULL OR ended_at >= ?1
             ORDER BY started_at, rowid",
            params![since],
        )
    }

    pub fn agent_invocations_for_wave_since(
        &self,
        wave_id: &WaveId,
        since: i64,
    ) -> StoreResult<Vec<AgentInvocationRow>> {
        self.query_agent_invocations(
            "SELECT ai.id, ai.run_id, ai.process_id, ai.started_at, ai.ended_at,
                    ai.repo, ai.worktree, ai.wave, ai.flow, ai.skill, ai.project,
                    ai.task, ai.provider, ai.model, ai.surface, ai.capture_status,
                    ai.incomplete_reason, ai.outcome, ai.artifact_dir,
                    ai.conversation_path, ai.provider_events_path,
                    ai.provider_session_id, ai.provider_session_path,
                    ai.conversation_event_count, ai.conversation_bytes,
                    ai.supervising_run_id, ai.account_id, ai.resume_token,
                    ai.answer_ask_id
             FROM agent_invocations ai
             JOIN runs r ON r.id = ai.supervising_run_id
             JOIN epochs e ON e.id = r.epoch_id
             LEFT JOIN projects ep ON ep.id = e.project_id
             LEFT JOIN tasks et ON et.id = e.task_id
             LEFT JOIN projects tp ON tp.id = et.project_id
             WHERE ai.started_at >= ?2
               AND (e.wave_id = ?1 OR ep.wave_id = ?1 OR tp.wave_id = ?1)
             ORDER BY ai.started_at, ai.rowid",
            params![wave_id, since],
        )
    }

    /// Invocations with a start or finish fact at or after `since`.
    pub fn agent_invocations_with_activity_since(
        &self,
        since: i64,
    ) -> StoreResult<Vec<AgentInvocationRow>> {
        self.query_agent_invocations(
            "SELECT id, run_id, process_id, started_at, ended_at, repo, worktree, wave, flow,
                    skill, project, task, provider, model, surface, capture_status,
                    incomplete_reason, outcome, artifact_dir, conversation_path,
                    provider_events_path, provider_session_id, provider_session_path,
                    conversation_event_count, conversation_bytes, supervising_run_id,
                    account_id, resume_token, answer_ask_id
             FROM agent_invocations
             WHERE started_at >= ?1 OR ended_at >= ?1
             ORDER BY started_at, rowid",
            params![since],
        )
    }

    /// Invocations owning a terminal Turn at or after `since`.
    pub fn agent_invocations_with_turns_ended_since(
        &self,
        since: i64,
    ) -> StoreResult<Vec<AgentInvocationRow>> {
        self.query_agent_invocations(
            "SELECT id, run_id, process_id, started_at, ended_at, repo, worktree, wave, flow,
                    skill, project, task, provider, model, surface, capture_status,
                    incomplete_reason, outcome, artifact_dir, conversation_path,
                    provider_events_path, provider_session_id, provider_session_path,
                    conversation_event_count, conversation_bytes, supervising_run_id,
                    account_id, resume_token, answer_ask_id
             FROM agent_invocations
             WHERE EXISTS (
                 SELECT 1 FROM agent_turns
                 WHERE agent_turns.invocation_id = agent_invocations.id
                   AND agent_turns.ended_at >= ?1
             )
             ORDER BY started_at, rowid",
            params![since],
        )
    }

    fn query_agent_invocations(
        &self,
        sql: &str,
        params: impl rusqlite::Params,
    ) -> StoreResult<Vec<AgentInvocationRow>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map(params, |row| {
            Ok(AgentInvocationRow {
                id: row.get(0)?,
                run_id: row.get(1)?,
                answer_ask_id: row.get(28)?,
                process_id: row.get(2)?,
                started_at: row.get(3)?,
                ended_at: row.get(4)?,
                repo: row.get(5)?,
                worktree: row.get(6)?,
                wave: row.get(7)?,
                flow: row.get(8)?,
                skill: row.get(9)?,
                project: row.get(10)?,
                task: row.get(11)?,
                provider: row.get(12)?,
                model: row.get(13)?,
                surface: row.get(14)?,
                capture_status: row.get(15)?,
                incomplete_reason: row.get(16)?,
                outcome: row.get(17)?,
                artifact_dir: row.get(18)?,
                conversation_path: row.get(19)?,
                provider_events_path: row.get(20)?,
                provider_session_id: row.get(21)?,
                provider_session_path: row.get(22)?,
                conversation_event_count: row.get(23)?,
                conversation_bytes: row.get(24)?,
                supervision: row
                    .get::<_, Option<String>>(25)?
                    .map(|run_id| {
                        Ok::<_, rusqlite::Error>(crate::trace::SupervisedInvocation {
                            invocation_id: crate::durable::AgentInvocationId::parse(
                                &row.get::<_, String>(0)?,
                            )
                            .map_err(to_sqlite_conversion_error)?,
                            supervising_run_id: crate::durable::RunId::parse(&run_id)
                                .map_err(to_sqlite_conversion_error)?,
                            account_id: row.get(26)?,
                            resume_token: row.get(27)?,
                        })
                    })
                    .transpose()?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub fn agent_turns_for_invocations(
        &self,
        invocation_ids: &[String],
    ) -> StoreResult<Vec<AgentTurnRow>> {
        if invocation_ids.is_empty() {
            return Ok(Vec::new());
        }
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut turns = Vec::new();
        for invocation_ids in invocation_ids.chunks(500) {
            let placeholders = in_placeholders(invocation_ids.len());
            let sql = format!(
                "SELECT t.id, t.invocation_id, t.ordinal, t.provider_turn_id, t.started_at,
                    t.ended_at, t.status, t.input_op, t.context_coverage, t.tokenizer,
                    t.system_prompt_path, t.task_prompt_path, t.system_tokens, t.task_tokens,
                    t.supplied_context_tokens, u.input_tokens, u.total_input_tokens,
                    u.peak_input_tokens, u.context_window_tokens, u.output_tokens,
                    u.reasoning_tokens, u.cache_read_tokens, u.cache_write_tokens, u.cost_usd,
                    t.context_gather_ms, t.context_render_ms, t.context_persist_ms,
                    t.first_event_seq, t.last_event_seq, t.root_output, t.epoch_id, t.basis_rev
             FROM agent_turns t
             LEFT JOIN turn_usage_samples u ON u.turn_id=t.id AND u.observed_at=(
                SELECT MAX(latest.observed_at) FROM turn_usage_samples latest
                WHERE latest.turn_id=t.id
             )
             WHERE t.invocation_id IN ({placeholders})
             ORDER BY t.started_at, t.rowid, t.ordinal"
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows =
                stmt.query_map(rusqlite::params_from_iter(invocation_ids), map_agent_turn)?;
            turns.extend(rows.collect::<Result<Vec<_>, _>>()?);
        }
        turns.sort_by_key(|turn| (turn.started_at, turn.ordinal));
        Ok(turns)
    }

    #[cfg(test)]
    pub(crate) fn performance_evidence_since(
        &self,
        since: i64,
    ) -> StoreResult<Option<PerformanceEvidenceSnapshot>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let schema_available: bool = conn.query_row(
            "SELECT
                 EXISTS(SELECT 1 FROM sqlite_schema
                        WHERE type='table' AND name='performance_evidence_authority')
                 AND EXISTS(SELECT 1 FROM pragma_table_info('runs')
                            WHERE name='first_material_at')
                 AND EXISTS(SELECT 1 FROM pragma_table_info('task_prs')
                            WHERE name='merged_at')
                 AND EXISTS(SELECT 1 FROM pragma_table_info('task_prs')
                            WHERE name='merge_tracking_complete')
                 AND EXISTS(SELECT 1 FROM pragma_table_info('task_prs')
                            WHERE name='repair_tracking_complete')
                 AND EXISTS(SELECT 1 FROM sqlite_schema
                            WHERE type='table' AND name='task_pr_repair_incidents')",
            [],
            |row| row.get(0),
        )?;
        if !schema_available {
            return Ok(None);
        }

        let authority_started_at = conn.query_row(
            "SELECT started_at FROM performance_evidence_authority
             WHERE singleton=1",
            [],
            |row| row.get(0),
        )?;

        let mut run_statement = conn.prepare(
            "SELECT COALESCE(r.cwd, t.worktree), r.started_at, r.ended_at,
                    r.first_material_at
             FROM runs r
             JOIN epochs e ON e.id=r.epoch_id
             JOIN tasks t ON t.id=e.task_id
             WHERE e.task_id IS NOT NULL
               AND r.started_at IS NOT NULL
               AND r.ended_at >= ?1
             ORDER BY r.ended_at, r.id",
        )?;
        let task_runs = run_statement
            .query_map([since], |row| {
                Ok(TaskFirstProgressEvidenceRow {
                    worktree: row.get(0)?,
                    started_at: row.get(1)?,
                    ended_at: row.get(2)?,
                    first_material_at: row.get(3)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut pr_statement = conn.prepare(
            "SELECT t.worktree, pr.merge_requested_at, pr.merged_at,
                    pr.merge_tracking_complete, pr.repair_tracking_complete,
                    pr.github_observation,
                    EXISTS(SELECT 1 FROM task_pr_repair_incidents incident
                           WHERE incident.task_pr_id=pr.id
                             AND incident.kind='avoidable_rebase_agent'),
                    EXISTS(SELECT 1 FROM task_pr_repair_incidents incident
                           WHERE incident.task_pr_id=pr.id
                             AND incident.kind='manual_git_repair')
             FROM task_prs pr
             JOIN tasks t ON t.id=pr.task_id
             WHERE pr.merge_requested_at IS NOT NULL
               AND pr.merge_commit IS NOT NULL
               AND (
                   pr.merged_at >= ?1
                   OR (pr.merge_tracking_complete = 1 AND pr.merged_at IS NULL)
               )
             ORDER BY COALESCE(pr.merged_at, pr.merge_requested_at), pr.id",
        )?;
        let task_prs = pr_statement
            .query_map([since], |row| {
                let observation = row
                    .get::<_, Option<String>>(5)?
                    .map(|json| serde_json::from_str::<crate::task::GithubObservation>(&json))
                    .transpose()
                    .map_err(to_sqlite_conversion_error)?;
                Ok(TaskPrPerformanceEvidenceRow {
                    worktree: row.get(0)?,
                    requested_at: row.get(1)?,
                    merged_at: row.get(2)?,
                    merge_tracking_complete: row.get(3)?,
                    repair_tracking_complete: row.get(4)?,
                    merge_observation_complete: observation.is_some_and(|observation| {
                        matches!(
                            observation.result,
                            crate::task::GithubObservationResult::Fresh
                        )
                    }),
                    avoidable_rebase_agent: row.get(6)?,
                    manual_git_repair: row.get(7)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(Some(PerformanceEvidenceSnapshot {
            authority_started_at,
            task_runs,
            task_prs,
        }))
    }

    pub fn asks_for_turns(&self, turn_ids: &[String]) -> StoreResult<Vec<crate::durable::Ask>> {
        if turn_ids.is_empty() {
            return Ok(Vec::new());
        }
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut asks = Vec::new();
        for turn_ids in turn_ids.chunks(500) {
            let placeholders = in_placeholders(turn_ids.len());
            let sql = format!(
                "SELECT id FROM ask_exchanges WHERE origin_turn_id IN ({placeholders})
                 ORDER BY asked_at, rowid"
            );
            let mut statement = conn.prepare(&sql)?;
            let ids = statement
                .query_map(rusqlite::params_from_iter(turn_ids), |row| {
                    row.get::<_, String>(0)
                })?
                .collect::<Result<Vec<_>, _>>()?;
            for id in ids {
                let id = crate::durable::AskId::parse(&id)
                    .map_err(|error| StoreError::InvalidData(error.to_string()))?;
                asks.push(durable::ask_by_id_in(&conn, &id)?);
            }
        }
        asks.sort_by_key(|ask| ask.asked_at);
        Ok(asks)
    }

    /// One agent turn by its UUID.
    pub fn agent_turn(&self, id: &str) -> StoreResult<Option<AgentTurnRow>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT t.id, t.invocation_id, t.ordinal, t.provider_turn_id, t.started_at,
                    t.ended_at, t.status, t.input_op, t.context_coverage, t.tokenizer,
                    t.system_prompt_path, t.task_prompt_path, t.system_tokens, t.task_tokens,
                    t.supplied_context_tokens, u.input_tokens, u.total_input_tokens,
                    u.peak_input_tokens, u.context_window_tokens, u.output_tokens,
                    u.reasoning_tokens, u.cache_read_tokens, u.cache_write_tokens, u.cost_usd,
                    t.context_gather_ms, t.context_render_ms, t.context_persist_ms,
                    t.first_event_seq, t.last_event_seq, t.root_output, t.epoch_id, t.basis_rev
             FROM agent_turns t
             LEFT JOIN turn_usage_samples u ON u.turn_id=t.id AND u.observed_at=(
                SELECT MAX(latest.observed_at) FROM turn_usage_samples latest
                WHERE latest.turn_id=t.id
             )
             WHERE t.id=?1",
        )?;
        let row = stmt.query_row(params![id], map_agent_turn).optional()?;
        Ok(row)
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

fn exact_or_prefix(value: &str) -> (&'static str, String) {
    if uuid::Uuid::parse_str(value).is_ok() {
        ("=", value.to_string())
    } else {
        ("LIKE", format!("{}%", value.replace(['%', '_'], "")))
    }
}

fn to_sql_i64(value: Option<u64>, field: &str) -> StoreResult<Option<i64>> {
    value
        .map(|value| {
            i64::try_from(value).map_err(|_| {
                StoreError::InvalidData(format!("{field} exceeds SQLite integer range"))
            })
        })
        .transpose()
}

fn map_turn_usage_sample(row: &rusqlite::Row<'_>) -> rusqlite::Result<TurnUsageSample> {
    Ok(TurnUsageSample {
        turn_id: row.get(0)?,
        observed_at: row.get(1)?,
        final_receipt: row.get(2)?,
        usage: TurnUsage {
            input_tokens: read_u64(row, 3)?,
            total_input_tokens: read_u64(row, 4)?,
            peak_input_tokens: read_u64(row, 5)?,
            context_window_tokens: read_u64(row, 6)?,
            output_tokens: read_u64(row, 7)?,
            reasoning_tokens: read_u64(row, 8)?,
            cache_read_tokens: read_u64(row, 9)?,
            cache_write_tokens: read_u64(row, 10)?,
            model: row.get(11)?,
            cost_usd: row.get(12)?,
        },
    })
}

fn read_u64(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<Option<u64>> {
    row.get::<_, Option<i64>>(index)?.map_or(Ok(None), |value| {
        u64::try_from(value).map(Some).map_err(|_| {
            rusqlite::Error::FromSqlConversionFailure(
                index,
                rusqlite::types::Type::Integer,
                std::io::Error::new(std::io::ErrorKind::InvalidData, "negative token count").into(),
            )
        })
    })
}

fn latest_turn_usage_sample_in(
    conn: &rusqlite::Connection,
    turn_id: &str,
) -> StoreResult<Option<TurnUsageSample>> {
    conn.query_row(
        "SELECT turn_id, observed_at, final_receipt, input_tokens,
                total_input_tokens, peak_input_tokens, context_window_tokens,
                output_tokens, reasoning_tokens, cache_read_tokens,
                cache_write_tokens, model, cost_usd
         FROM turn_usage_samples
         WHERE turn_id=?1
         ORDER BY observed_at DESC
         LIMIT 1",
        [turn_id],
        map_turn_usage_sample,
    )
    .optional()
    .map_err(StoreError::from)
}

fn validate_usage_progress(
    previous: &TurnUsageSample,
    sample: &TurnUsageSample,
) -> StoreResult<()> {
    if previous.final_receipt && !sample.final_receipt {
        return Err(StoreError::InvalidData(
            "final usage receipt cannot become provisional".to_string(),
        ));
    }
    let counters = [
        (
            "input_tokens",
            previous.usage.input_tokens,
            sample.usage.input_tokens,
        ),
        (
            "total_input_tokens",
            previous.usage.total_input_tokens,
            sample.usage.total_input_tokens,
        ),
        (
            "output_tokens",
            previous.usage.output_tokens,
            sample.usage.output_tokens,
        ),
        (
            "reasoning_tokens",
            previous.usage.reasoning_tokens,
            sample.usage.reasoning_tokens,
        ),
        (
            "cache_read_tokens",
            previous.usage.cache_read_tokens,
            sample.usage.cache_read_tokens,
        ),
        (
            "cache_write_tokens",
            previous.usage.cache_write_tokens,
            sample.usage.cache_write_tokens,
        ),
    ];
    for (field, before, after) in counters {
        if matches!((before, after), (Some(before), Some(after)) if after < before) {
            return Err(StoreError::InvalidData(format!(
                "{field} decreased within one Turn"
            )));
        }
    }
    Ok(())
}

fn insert_agent_turn(tx: &rusqlite::Transaction<'_>, turn: &AgentTurnRow) -> StoreResult<()> {
    tx.execute(
        "INSERT INTO agent_turns (
            id, invocation_id, ordinal, provider_turn_id, started_at, ended_at, status, input_op,
            context_coverage, tokenizer, system_prompt_path, task_prompt_path, system_tokens,
            task_tokens, supplied_context_tokens, context_gather_ms, context_render_ms,
            context_persist_ms, first_event_seq, last_event_seq, root_output, epoch_id, basis_rev
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
            ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23)",
        params![
            turn.id,
            turn.invocation_id,
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
            turn.context_gather_ms,
            turn.context_render_ms,
            turn.context_persist_ms,
            turn.first_event_seq,
            turn.last_event_seq,
            turn.root_output,
            turn.basis.as_ref().map(|basis| basis.epoch_id.as_str()),
            turn.basis.as_ref().map(|basis| basis.revision as i64),
        ],
    )?;
    if let Some(basis) = &turn.basis {
        durable::insert_seed_sends_for_turn(tx, &turn.id, basis)?;
    }
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
            first_event_seq = ?5, last_event_seq = ?6, root_output = ?7
         WHERE id = ?1",
        params![
            turn.id,
            turn.provider_turn_id,
            turn.ended_at,
            turn.status,
            turn.first_event_seq,
            turn.last_event_seq,
            turn.root_output,
        ],
    )?;
    Ok(())
}

fn map_agent_turn(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentTurnRow> {
    Ok(AgentTurnRow {
        id: row.get(0)?,
        invocation_id: row.get(1)?,
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
        usage: map_turn_usage(row, 15, None)?,
        context_gather_ms: row.get(24)?,
        context_render_ms: row.get(25)?,
        context_persist_ms: row.get(26)?,
        first_event_seq: row.get(27)?,
        last_event_seq: row.get(28)?,
        root_output: row.get(29)?,
        basis: match (
            row.get::<_, Option<String>>(30)?,
            row.get::<_, Option<i64>>(31)?,
        ) {
            (Some(epoch_id), Some(revision)) => Some(crate::durable::Basis {
                epoch_id: crate::durable::EpochId::parse(&epoch_id).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        30,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?,
                revision: revision as u64,
            }),
            (None, None) => None,
            _ => {
                return Err(rusqlite::Error::InvalidColumnType(
                    30,
                    "epoch_id/basis_rev".to_string(),
                    rusqlite::types::Type::Null,
                ))
            }
        },
    })
}

fn map_turn_usage(
    row: &rusqlite::Row<'_>,
    start: usize,
    model: Option<String>,
) -> rusqlite::Result<Option<TurnUsage>> {
    let usage = TurnUsage {
        input_tokens: read_u64(row, start)?,
        total_input_tokens: read_u64(row, start + 1)?,
        peak_input_tokens: read_u64(row, start + 2)?,
        context_window_tokens: read_u64(row, start + 3)?,
        output_tokens: read_u64(row, start + 4)?,
        reasoning_tokens: read_u64(row, start + 5)?,
        cache_read_tokens: read_u64(row, start + 6)?,
        cache_write_tokens: read_u64(row, start + 7)?,
        model,
        cost_usd: row.get(start + 8)?,
    };
    Ok(usage.is_reported().then_some(usage))
}

#[cfg(test)]
mod contention_tests {
    use super::{SqliteStore, SQLITE_WRITE_BUSY_TIMEOUT};
    use crate::chat::types::TurnUsage;
    use crate::store::{StoreResult, TurnUsageSample};
    use crate::trace::{AgentInvocationRow, AgentTurnRow};
    use std::path::Path;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, Instant};

    static WRITE_CONTENDED: AtomicBool = AtomicBool::new(false);

    fn signal_busy(_: i32) -> bool {
        WRITE_CONTENDED.store(true, Ordering::Release);
        true
    }

    fn while_wal_writer_holds_lock<T>(
        store: &SqliteStore,
        path: &Path,
        operation: &str,
        run: impl FnOnce() -> StoreResult<T>,
    ) -> T {
        WRITE_CONTENDED.store(false, Ordering::Release);
        store
            .conn
            .lock()
            .expect("store mutex poisoned")
            .busy_handler(Some(signal_busy))
            .unwrap();

        let path = path.to_path_buf();
        let writer_run_id = format!("holder-{operation}");
        let (locked_tx, locked_rx) = mpsc::sync_channel(0);
        let writer = thread::spawn(move || {
            let mut connection = rusqlite::Connection::open(path).unwrap();
            let transaction = connection
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .unwrap();
            transaction
                .execute(
                    "INSERT INTO run_events (run_id, process_id, seq, ts, node, event)
                     VALUES (?1, 'holder', 0, 1, 'flow', 'started')",
                    [writer_run_id],
                )
                .unwrap();
            locked_tx.send(()).unwrap();
            let deadline = Instant::now() + Duration::from_secs(1);
            while !WRITE_CONTENDED.load(Ordering::Acquire) && Instant::now() < deadline {
                thread::yield_now();
            }
            transaction.commit().unwrap();
            WRITE_CONTENDED.load(Ordering::Acquire)
        });

        locked_rx.recv().unwrap();
        let result = run();
        let contended = writer.join().unwrap();
        let connection = store.conn.lock().expect("store mutex poisoned");
        connection.busy_handler(None).unwrap();
        connection.busy_timeout(SQLITE_WRITE_BUSY_TIMEOUT).unwrap();
        assert!(
            contended,
            "{operation} bypassed the busy handler instead of acquiring write authority first"
        );
        result.unwrap()
    }

    fn invocation() -> AgentInvocationRow {
        AgentInvocationRow {
            id: "invocation_contention".to_string(),
            run_id: "run-contention".to_string(),
            answer_ask_id: None,
            process_id: "exec_contention".to_string(),
            started_at: 1,
            ended_at: None,
            repo: "/repo".to_string(),
            worktree: "/repo/task".to_string(),
            wave: Some("product".to_string()),
            flow: Some("task".to_string()),
            skill: Some("ship-5whys".to_string()),
            project: Some("loopflow-api".to_string()),
            task: Some("LOO-238".to_string()),
            provider: "codex".to_string(),
            model: Some("gpt-5".to_string()),
            surface: "headless".to_string(),
            capture_status: "capturing".to_string(),
            incomplete_reason: None,
            outcome: "running".to_string(),
            artifact_dir: "run/exec/invocation".to_string(),
            conversation_path: "run/exec/invocation/conversation.jsonl".to_string(),
            provider_events_path: None,
            provider_session_id: None,
            provider_session_path: None,
            conversation_event_count: 1,
            conversation_bytes: 10,
            supervision: None,
        }
    }

    fn turn(id: &str, ordinal: i64, status: &str) -> AgentTurnRow {
        AgentTurnRow {
            id: id.to_string(),
            invocation_id: "invocation_contention".to_string(),
            ordinal,
            provider_turn_id: None,
            started_at: ordinal,
            ended_at: None,
            status: status.to_string(),
            input_op: if ordinal == 1 { "initial" } else { "message" }.to_string(),
            context_coverage: "assembled".to_string(),
            tokenizer: "cl100k_base".to_string(),
            system_prompt_path: None,
            task_prompt_path: format!("run/exec/invocation/turns/{ordinal:04}-task.md"),
            system_tokens: 0,
            task_tokens: 2,
            supplied_context_tokens: 2,
            usage: None,
            context_gather_ms: 1,
            context_render_ms: 1,
            context_persist_ms: 1,
            first_event_seq: Some(ordinal - 1),
            last_event_seq: None,
            root_output: None,
            basis: None,
        }
    }

    #[test]
    fn multi_turn_capture_waits_for_concurrent_wal_writers() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("loopflow.db");
        let store = SqliteStore::new(&path).expect("materialize the schema");
        let mut invocation = invocation();
        let mut first_turn = turn("turn_contention_1", 1, "running");

        while_wal_writer_holds_lock(&store, &path, "capture start", || {
            store.insert_trace_capture(&invocation, &first_turn, &[], &[])
        });
        while_wal_writer_holds_lock(&store, &path, "usage checkpoint", || {
            store.record_turn_usage_sample(&TurnUsageSample {
                turn_id: first_turn.id.clone(),
                observed_at: 2,
                final_receipt: false,
                usage: TurnUsage {
                    input_tokens: Some(10),
                    output_tokens: Some(1),
                    ..Default::default()
                },
            })
        });

        first_turn.ended_at = Some(3);
        first_turn.status = "partial".to_string();
        let second_turn = turn("turn_contention_2", 2, "running");
        while_wal_writer_holds_lock(&store, &path, "turn transition", || {
            store.finish_agent_turn_capture(&first_turn)?;
            store.insert_agent_turn_capture(&second_turn, &[], &[])
        });

        invocation.ended_at = Some(4);
        invocation.capture_status = "complete".to_string();
        invocation.outcome = "completed".to_string();
        let mut second_turn = second_turn;
        second_turn.ended_at = Some(4);
        second_turn.status = "completed".to_string();
        while_wal_writer_holds_lock(&store, &path, "capture finish", || {
            store.finish_trace_capture(&invocation, &second_turn)
        });

        let invocations = store
            .agent_invocations_matching(&invocation.run_id)
            .unwrap();
        assert_eq!(invocations.len(), 1);
        assert_eq!(invocations[0].capture_status, "complete");
        assert_eq!(invocations[0].outcome, "completed");
        let turns = store
            .agent_turns_for_invocations(&[invocation.id.clone()])
            .unwrap();
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].status, "partial");
        assert_eq!(turns[1].status, "completed");
        assert_eq!(
            turns[0].usage.as_ref().and_then(|usage| usage.input_tokens),
            Some(10)
        );
    }
}

#[cfg(test)]
mod frontier_tests {
    use super::{writer_invocation_is_authoritative, SqliteStore};
    use crate::build_info::MigrationAuthority::{self, Published, ValidationOnly};
    use crate::store::migrations::{
        apply_all_but_head, latest_applied_version_sqlite, latest_known_version,
        prior_known_version,
    };
    use crate::store::FrontierAdvance::{self, Authorized, Forbidden};
    use std::path::{Path, PathBuf};

    /// The machine home whose `.lf/loopflow.db` `may_apply_migrations` treats as
    /// the shared release store. The regressions inject it so they never touch a
    /// developer's real `~/.lf`.
    struct SharedHome {
        _dir: tempfile::TempDir,
        home: PathBuf,
    }

    impl SharedHome {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let home = dir.path().to_path_buf();
            Self { _dir: dir, home }
        }

        fn shared_db(&self) -> PathBuf {
            self.home.join(".lf/loopflow.db")
        }
    }

    fn open(
        path: &Path,
        authority: MigrationAuthority,
        home: &Path,
        advance: FrontierAdvance,
    ) -> crate::store::StoreResult<SqliteStore> {
        SqliteStore::open_with(path, authority, home, advance)
    }

    fn frontier(path: &Path) -> Option<String> {
        let conn = rusqlite::Connection::open(path).unwrap();
        latest_applied_version_sqlite(&conn).unwrap()
    }

    #[test]
    fn writer_authority_follows_the_exact_run_and_ask_not_the_process() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("authority.db");
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE runs (id TEXT PRIMARY KEY, state TEXT NOT NULL);
                 CREATE TABLE agent_invocations (
                    id TEXT PRIMARY KEY,
                    ended_at INTEGER,
                    supervising_run_id TEXT,
                    answer_ask_id TEXT
                 );
                 CREATE TABLE agent_turns (
                    id TEXT PRIMARY KEY,
                    invocation_id TEXT NOT NULL,
                    status TEXT NOT NULL
                 );
                 CREATE TABLE ask_exchanges (
                    id TEXT PRIMARY KEY,
                    state TEXT NOT NULL,
                    active_invocation_id TEXT
                 );
                 INSERT INTO runs VALUES ('run_task', 'active');
                 INSERT INTO agent_invocations
                    VALUES ('invocation_task', NULL, 'run_task', NULL);
                 INSERT INTO agent_turns
                    VALUES ('turn_task', 'invocation_task', 'running');
                 INSERT INTO runs VALUES ('run_parent', 'active');
                 INSERT INTO agent_invocations
                    VALUES ('invocation_ask', NULL, 'run_parent', 'ask_one');
                 INSERT INTO agent_turns
                    VALUES ('turn_ask', 'invocation_ask', 'running');
                 INSERT INTO ask_exchanges
                    VALUES ('ask_one', 'claimed', 'invocation_ask');",
            )
            .unwrap();
        drop(connection);

        assert!(writer_invocation_is_authoritative(&path, "invocation_task").unwrap());
        assert!(writer_invocation_is_authoritative(&path, "invocation_ask").unwrap());

        let connection = rusqlite::Connection::open(&path).unwrap();
        connection
            .execute(
                "UPDATE agent_turns SET status='completed' WHERE id='turn_task'",
                [],
            )
            .unwrap();
        connection
            .execute(
                "UPDATE agent_turns SET status='completed' WHERE id='turn_ask'",
                [],
            )
            .unwrap();
        drop(connection);

        assert!(!writer_invocation_is_authoritative(&path, "invocation_task").unwrap());
        assert!(!writer_invocation_is_authoritative(&path, "invocation_ask").unwrap());

        let connection = rusqlite::Connection::open(&path).unwrap();
        connection
            .execute(
                "UPDATE ask_exchanges
                 SET state='resolved', active_invocation_id=NULL
                 WHERE id='ask_one'",
                [],
            )
            .unwrap();
        drop(connection);

        assert!(!writer_invocation_is_authoritative(&path, "invocation_task").unwrap());
        assert!(!writer_invocation_is_authoritative(&path, "invocation_ask").unwrap());
        assert!(!writer_invocation_is_authoritative(&path, "invocation_missing").unwrap());
    }

    fn seed_shared_store_at_prior_head(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let conn = rusqlite::Connection::open(path).unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON").unwrap();
        apply_all_but_head(&conn).unwrap();
    }

    fn seed_completed_trace(path: &Path) {
        let conn = rusqlite::Connection::open(path).unwrap();
        conn.execute_batch(
            "INSERT INTO run_events
                (run_id, process_id, seq, ts, node, event)
             VALUES
                ('trace-before-promotion', 'process-before-promotion', 0, 100, 'run', 'started'),
                ('trace-before-promotion', 'process-before-promotion', 1, 101, 'run', 'completed')",
        )
        .unwrap();
    }

    fn trace_events(path: &Path) -> Vec<String> {
        SqliteStore::open_run_ledger_read_only(path)
            .unwrap()
            .list_run_events_since(0)
            .unwrap()
            .into_iter()
            .map(|event| event.event)
            .collect()
    }

    /// (a) An ordinary open of an absent shared store must refuse actionably and
    /// leave no file behind. Sabotage guard: an `open_with` that creates the
    /// SQLite file before deciding authority (or that lets Forbidden bootstrap)
    /// would create the path and this fails.
    #[test]
    fn an_ordinary_open_never_creates_or_initializes_an_absent_shared_store() {
        let shared = SharedHome::new();
        let path = shared.shared_db();

        let error = open(&path, Published, &shared.home, Forbidden)
            .expect_err("an ordinary open must not initialize the shared store");
        assert!(
            error.to_string().contains("scripts/install.py refresh"),
            "the refusal must name the authorized boundary: {error}"
        );
        assert!(
            !path.exists(),
            "an ordinary open must not create the shared store file"
        );
    }

    /// (b) An ordinary open of an existing shared store the binary is ahead of
    /// must refuse actionably without advancing — it must not hand N+1 code a
    /// store still at the N schema — while the old N reader keeps recognizing it.
    /// Sabotage guards: a Forbidden open that applied the pending head, or that
    /// returned a usable store instead of erroring, fails this test.
    #[test]
    fn an_ordinary_open_ahead_of_the_shared_frontier_refuses_without_advancing() {
        let shared = SharedHome::new();
        let path = shared.shared_db();
        seed_shared_store_at_prior_head(&path);
        let installed_frontier = frontier(&path).unwrap();
        assert_eq!(installed_frontier, prior_known_version());
        assert_ne!(installed_frontier, latest_known_version());

        // The candidate is one migration ahead; its ordinary open refuses rather
        // than reuse a schema older than its own code.
        let error = open(&path, Published, &shared.home, Forbidden)
            .expect_err("an ordinary open ahead of the frontier must refuse");
        assert!(
            error.to_string().contains("scripts/install.py refresh"),
            "the refusal must name the authorized boundary: {error}"
        );
        assert!(
            error.to_string().contains(&installed_frontier)
                || error.to_string().contains(&latest_known_version()),
            "the refusal names the pending frontier: {error}"
        );
        assert_eq!(
            frontier(&path).as_deref(),
            Some(installed_frontier.as_str()),
            "a refused open must not advance the shared frontier"
        );

        // The old reader — a build whose head is the store's frontier — still
        // recognizes the untouched store as exactly its own frontier.
        let conn = rusqlite::Connection::open(&path).unwrap();
        assert!(
            crate::store::migrations::old_reader_recognizes(&conn),
            "the old installed reader must still recognize the untouched store"
        );
    }

    /// (c) The promotion boundary owns both first initialization and advancement.
    #[test]
    fn the_promotion_boundary_initializes_and_advances_the_shared_store() {
        let shared = SharedHome::new();
        let path = shared.shared_db();

        // Initialization from absent.
        open(&path, Published, &shared.home, Authorized).expect("boundary initializes");
        assert_eq!(
            frontier(&path).as_deref(),
            Some(latest_known_version().as_str())
        );

        // Once the boundary has initialized it, an ordinary open validates the
        // now-existing store read-only and leaves the frontier at the head.
        open(&path, Published, &shared.home, Forbidden)
            .expect("an ordinary open validates the initialized store");
        assert_eq!(
            frontier(&path).as_deref(),
            Some(latest_known_version().as_str())
        );

        // Advancement from a prior-head store.
        let advanced = SharedHome::new();
        let advanced_path = advanced.shared_db();
        seed_shared_store_at_prior_head(&advanced_path);
        assert_eq!(
            frontier(&advanced_path).as_deref(),
            Some(prior_known_version().as_str())
        );
        open(&advanced_path, Published, &advanced.home, Authorized).expect("boundary advances");
        assert_eq!(
            frontier(&advanced_path).as_deref(),
            Some(latest_known_version().as_str())
        );
    }

    /// The 2026-07-17 incident shape, exercised as two binary generations: a
    /// branch candidate knows one migration the installed release does not.
    /// Ordinary candidate use must leave both the shared frontier and existing
    /// trace status untouched; explicit promotion advances once, after which
    /// both current opens and the stable ledger reader retain the trace.
    #[test]
    fn branch_candidate_cannot_advance_shared_store_or_damage_trace_status_outside_promotion() {
        let shared = SharedHome::new();
        let path = shared.shared_db();
        seed_shared_store_at_prior_head(&path);
        seed_completed_trace(&path);
        let installed_frontier = prior_known_version();
        assert_eq!(
            frontier(&path).as_deref(),
            Some(installed_frontier.as_str())
        );
        assert_eq!(trace_events(&path), vec!["started", "completed"]);

        open(&path, Published, &shared.home, Forbidden)
            .expect_err("ordinary branch candidate must not promote its draft migration");
        assert_eq!(
            frontier(&path).as_deref(),
            Some(installed_frontier.as_str())
        );
        assert_eq!(trace_events(&path), vec!["started", "completed"]);
        let installed = rusqlite::Connection::open(&path).unwrap();
        assert!(
            crate::store::migrations::old_reader_recognizes(&installed),
            "the installed release must still recognize the candidate's untouched store"
        );
        drop(installed);

        open(&path, Published, &shared.home, Authorized)
            .expect("explicit promotion advances the shared frontier");
        let promoted_frontier = latest_known_version();
        assert_eq!(frontier(&path).as_deref(), Some(promoted_frontier.as_str()));
        assert_eq!(trace_events(&path), vec!["started", "completed"]);

        open(&path, Published, &shared.home, Authorized)
            .expect("repeating promotion at the same frontier is a no-op");
        assert_eq!(frontier(&path).as_deref(), Some(promoted_frontier.as_str()));
        assert_eq!(trace_events(&path), vec!["started", "completed"]);
        open(&path, Published, &shared.home, Forbidden)
            .expect("ordinary current binary opens after promotion");
    }

    /// A validation-only build never advances the shared store even at the
    /// nominal boundary, and a private/isolated DB stays freely initializable —
    /// the isolated dev escape the directive preserves.
    #[test]
    fn validation_only_is_walled_from_the_shared_store_but_not_private_ones() {
        let shared = SharedHome::new();
        let path = shared.shared_db();
        open(&path, ValidationOnly, &shared.home, Authorized)
            .expect_err("a validation-only build must never initialize the shared store");
        assert!(!path.exists());

        // A private path (not ~/.lf/loopflow.db) initializes regardless of
        // authority or boundary.
        let private = shared.home.join(".lf-dev/branch/loopflow.db");
        open(&private, ValidationOnly, &shared.home, Forbidden).expect("private DB initializes");
        assert_eq!(
            frontier(&private).as_deref(),
            Some(latest_known_version().as_str())
        );
    }
}
