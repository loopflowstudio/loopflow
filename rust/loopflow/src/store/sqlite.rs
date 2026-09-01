use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rusqlite::{params, Connection, OptionalExtension, ToSql, TransactionBehavior};

use crate::id::WaveId;
use crate::profile::{
    AccessProfile, AccountAccessProfile, EmailAddress, ProfileId, ProviderRoute, RouteScope,
};
use crate::provider_auth::Provider;
use crate::store::rows::{map_wave_row, now_unix};
use crate::store::token_crypto;
use crate::store::{
    AccountLimitRow, CredentialState, PmSnapshotRow, ProviderAccount, ProviderAccountId,
    ProviderAccountSelection, RoutingState, RunEventRow, StoreError, StoreResult,
    WaveLocatorUpdate,
};
use crate::work::wave::{Wave, WaveLocator};

mod children;
mod ci_incidents;
mod controller;
mod durable;
mod metrics;
mod pr_landings;
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
    let mut statement = conn.prepare("SELECT worktree FROM tasks WHERE work_state='ready'")?;
    let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
    rows.map(|row| row.map(PathBuf::from).map_err(StoreError::from))
        .collect()
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
    /// owner of the migration frontier. Applies pending migrations under the
    /// caller's exclusive promotion lock.
    pub(crate) fn open_as_promotion_boundary(path: &Path) -> StoreResult<Self> {
        Self::open(path, super::FrontierAdvance::Authorized)
    }

    /// Advance one disposable installed-development store through this build's
    /// canonical migrations and exact embedded draft tail.
    pub(crate) fn open_as_local_promotion_boundary(path: &Path) -> StoreResult<Self> {
        super::guard_development_database(
            path,
            crate::build_info::provenance(),
            &super::machine_home_dir(),
        )
        .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                StoreError::InvalidData(format!("failed to create db dir: {error}"))
            })?;
        }
        let conn = Connection::open(path)?;
        conn.busy_timeout(SQLITE_WRITE_BUSY_TIMEOUT)?;
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")?;
        super::migrations::apply_installed_development_sqlite(
            &conn,
            crate::build_info::migration_draft_manifest(),
        )?;
        validate_run_events_schema(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Open a hermetic, fully-migrated store at `path`: the base canonical
    /// migrations plus this build's exact embedded draft manifest, reading **no**
    /// process- or machine-global state — no `LF_HOME`, no install selection, no
    /// shared `~/.lf` identity, no frontier authority. Tests use this so their
    /// schema is deterministic under parallel execution; the production
    /// [`Self::open`] path resolves real install/frontier authority and is what
    /// races when tests mutate ambient env concurrently.
    pub(crate) fn open_ephemeral(path: &Path) -> StoreResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                StoreError::InvalidData(format!("failed to create db dir: {error}"))
            })?;
        }
        let conn = Connection::open(path)?;
        conn.busy_timeout(SQLITE_WRITE_BUSY_TIMEOUT)?;
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")?;
        super::migrations::apply_installed_development_sqlite(
            &conn,
            crate::build_info::migration_draft_manifest(),
        )?;
        validate_run_events_schema(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
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
        let installed_selection = crate::machine_install::selection_for_current_executable()
            .map_err(|error| {
                StoreError::InvalidData(format!("resolve machine install selection: {error}"))
            })?;
        let installed_development = match installed_selection {
            Some(selection)
                if selection.source == crate::machine_install::InstallSource::Development =>
            {
                super::same_database_file(path, &selection.store).map_err(|error| {
                    StoreError::InvalidData(format!(
                        "resolve installed development store identity: {error}"
                    ))
                })?
            }
            _ => false,
        };
        // Resolve the frontier authority before touching the filesystem. An
        // ordinary open of a shared store it may not initialize refuses here,
        // before create_dir_all/Connection::open would leave an empty
        // ~/.lf/loopflow.db behind — a file whose mere existence a liveness or
        // bootstrap check could misread as "the shared store is initialized".
        let may_apply_migrations = super::may_apply_migrations(path, authority, home, advance)
            .map_err(|error| {
                StoreError::InvalidData(format!("resolve migration authority: {error}"))
            })?;
        let shared_database = super::same_database_file(path, &home.join(".lf/loopflow.db"))
            .map_err(|error| {
                StoreError::InvalidData(format!("resolve shared store identity: {error}"))
            })?;
        let initializes_private_development = !installed_development
            && !shared_database
            && may_apply_migrations
            && !crate::build_info::migration_draft_manifest().is_empty()
            && crate::build_info::provenance() == crate::build_info::BuildProvenance::Development;
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

        if installed_development {
            super::migrations::validate_installed_development_sqlite(
                &conn,
                crate::build_info::migration_draft_manifest(),
            )?;
        } else if initializes_private_development {
            super::migrations::apply_installed_development_sqlite(
                &conn,
                crate::build_info::migration_draft_manifest(),
            )?;
        } else if !may_apply_migrations {
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
        durable::create_wave_work(&tx, wave.id(), created_at)?;
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
        }

        for update in updates {
            if let Some(collision) = &update.retire_collision {
                let retired_at = now_unix();
                tx.execute(
                    "UPDATE work_placements SET enabled = 0 WHERE wave_id = ?1",
                    params![collision],
                )?;
                tx.execute(
                    "UPDATE waves
                     SET retired_at = ?2,
                         superseded_by_wave_id = ?3,
                         retirement_reason = ?4,
                         work_state = 'abandoned',
                         work_terminal_at = ?2
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
        tx.execute("DELETE FROM waves WHERE id = ?1", params![wave_id])?;
        tx.commit()?;
        Ok(())
    }

    // Exec ledger (`run_events`): the machine-grain, append-only record of
    // every process written directly by `lf`.

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
}

fn exact_or_prefix(value: &str) -> (&'static str, String) {
    if uuid::Uuid::parse_str(value).is_ok() {
        ("=", value.to_string())
    } else {
        ("LIKE", format!("{}%", value.replace(['%', '_'], "")))
    }
}
#[cfg(test)]
mod frontier_tests {
    use super::SqliteStore;
    use crate::build_info::MigrationAuthority::{self, Published, ValidationOnly};
    use crate::durable::{WorkRef, WorkStatus};
    use crate::id::WaveId;
    use crate::store::migrations::{
        apply_all_but_head, latest_applied_version_sqlite, latest_known_version,
        prior_known_version,
    };
    use crate::store::FrontierAdvance::{self, Authorized, Forbidden};
    use crate::work::wave::Wave;
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

    #[test]
    fn private_development_store_opens_with_the_current_work_schema() {
        let shared = SharedHome::new();
        let path = shared.home.join("private/loopflow.db");
        let store = open(&path, ValidationOnly, &shared.home, Forbidden)
            .expect("private development store opens at the embedded draft frontier");
        let wave = Wave::new(
            WaveId::new(),
            "private-development".to_string(),
            "/repo".to_string(),
        );

        store.create_wave(&wave).unwrap();

        assert_eq!(
            store
                .work_status(&WorkRef::Wave(wave.id().clone()))
                .unwrap(),
            WorkStatus::Ready
        );
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
