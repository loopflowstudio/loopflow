use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::profile::{AccessProfile, EmailAddress, ProfileId};
use crate::provider::Provider;
use crate::rows::{map_wave_row, now_unix};
use crate::token_crypto;
use crate::work::wave::{Wave, WaveLocator};
use crate::{
    CredentialState, ProviderAccount, ProviderAccountId, ProviderTokenReplacement, StoreError,
    StoreResult, WaveLocatorUpdate,
};
use loopflow_model::durable::{ProjectId, TaskId, WorkRef};
use loopflow_model::id::{AgentSessionId, WaveId};

mod admission;
pub mod children;
pub mod ci_incidents;
pub mod command_answers;
pub mod creation;
pub mod desktop_shells;
pub mod durable;
mod github_people;
mod graph_inventory;
pub mod linear_events;
mod linear_issue_state;
mod subscription_events;
mod task_started;

mod linear_team_checks;
mod machine_records;
pub mod plan_carriers;
pub mod planning_order;
mod planning_sync;
pub mod planning_write;
mod pr_landings;
mod processes;
pub use processes::{PreviousAgentDisposition, SessionTransfer};
mod program_status;
#[cfg(test)]
mod project_content_tests;
pub mod provider_transitions;
mod rentals;
pub mod repositories;
pub mod retention;
mod revisions;
pub mod scope_defaults;
pub mod selections;
mod session_events;
pub mod session_recovery;
pub mod sessions;
mod turns;
pub use sessions::SessionCaller;
pub mod task_ownership;

mod steers;
mod task_comments;
pub use task_comments::TaskCommentThread;
mod task_content;
mod task_details;
pub mod task_moves;
mod task_retirement;
mod transport;
pub use task_retirement::{TaskRetirement, ViewerHold, WORKSPACE_GRACE};
mod task_state;
pub mod task_summaries;
pub mod task_work;
pub mod tracker_actions;
pub mod wave_definitions;
mod wave_lifecycle;

pub use children::ClosedTaskPage;
pub use graph_inventory::{entry_of, AgentLimitRecovery, GraphExecutionFacts, UnrunGraph};
pub use revisions::{Revisions, StoreRevisions};
pub use session_events::{AnsweredTurn, CurrentConversation, TurnSpan};
pub use task_details::TaskDetailFacts;
pub use task_work::{Auto, AutoState, OpenWork};

/// A fleet can legitimately queue longer than SQLite's common five-second
/// default while every process opens and records its first receipt. Durable
/// writes wait for that bounded local contention instead of dropping evidence.
pub const SQLITE_WRITE_BUSY_TIMEOUT: Duration = Duration::from_secs(15);

/// Bytes of write-ahead log kept on disk after a checkpoint resets it.
const WAL_SIZE_LIMIT_BYTES: i64 = 64 * 1024 * 1024;

/// The first rollback-to-WAL transition can return BUSY immediately even with
/// a busy handler: two readers cannot both upgrade their journal lock. Reuse
/// migration exclusion only for that transition; ordinary WAL opens stay reads.
/// Why an ordinary open refuses a shared store this build is ahead of, and the fix.
fn unapplied_message(path: &Path, pending: &str) -> String {
    if super::migration_runner::unmerged_migrations().contains(&pending) {
        format!(
            "shared store {} does not have this build's unmerged migration {pending}; \
             run this build with a private LF_HOME",
            path.display()
        )
    } else {
        format!(
            "shared store {} does not have migration {pending} yet; run `lf install` to \
             finish installing this build",
            path.display()
        )
    }
}

fn configure_write_connection(
    conn: &mut Connection,
    path: &Path,
    busy: Duration,
) -> StoreResult<()> {
    conn.busy_timeout(busy)?;
    // A deferred transaction that reads, then writes after another process
    // committed, fails at once with SQLITE_BUSY_SNAPSHOT: the busy handler
    // never runs. Taking the write lock at BEGIN makes it wait instead.
    conn.set_transaction_behavior(TransactionBehavior::Immediate);
    let mode: String = conn.pragma_query_value(None, "journal_mode", |row| row.get(0))?;
    if mode != "wal" {
        let _lock = super::migration_runner::migration_lock(path)?;
        let mode: String = conn.pragma_query_value(None, "journal_mode", |row| row.get(0))?;
        if mode != "wal" {
            conn.pragma_update(None, "journal_mode", "WAL")?;
        }
    }
    conn.pragma_update(None, "foreign_keys", "ON")?;
    super::write_holds::watch_write_holds(conn)?;
    // A WAL file never shrinks by itself: one migration or burst leaves its
    // high-water mark on disk forever. Truncate it whenever a checkpoint resets it.
    conn.pragma_update(None, "journal_size_limit", WAL_SIZE_LIMIT_BYTES)?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct SqliteStore {
    #[cfg(feature = "test-support")]
    pub conn: Arc<Mutex<Connection>>,
    #[cfg(not(feature = "test-support"))]
    pub(crate) conn: Arc<Mutex<Connection>>,
    #[cfg(feature = "test-support")]
    pub token_key: token_crypto::TokenKeySource,
    #[cfg(not(feature = "test-support"))]
    pub(crate) token_key: token_crypto::TokenKeySource,
    /// This connection's writer of `history.db`, opened at its first write.
    #[cfg(feature = "test-support")]
    pub history: Arc<crate::history::HistoryFile>,
    #[cfg(not(feature = "test-support"))]
    pub(crate) history: Arc<crate::history::HistoryFile>,
}

fn waves_in(conn: &Connection, repo: Option<&str>) -> StoreResult<Vec<Wave>> {
    let query = if repo.is_some() {
        "SELECT id, name, repo, created_at, parent_wave_id, promoted_at,
                    retired_at, superseded_by_wave_id, retirement_reason, slug
             FROM wave_addresses WHERE repo = ?1 AND retired_at IS NULL ORDER BY created_at DESC"
    } else {
        "SELECT id, name, repo, created_at, parent_wave_id, promoted_at,
                    retired_at, superseded_by_wave_id, retirement_reason, slug
             FROM wave_addresses WHERE retired_at IS NULL ORDER BY created_at DESC"
    };
    let mut stmt = conn.prepare(query)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(repo), |row| {
        Ok(map_wave_row(row))
    })?;
    rows.map(|wave| wave?).collect()
}

impl SqliteStore {
    /// The database backing this connection; in-memory connections have no path.
    pub fn path(&self) -> Option<PathBuf> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.path()
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
    }

    /// Refresh `sqlite_stat1` for tables whose size changed enough to move a
    /// plan, in the store and in `history.db`. Bounded by `analysis_limit`,
    /// it holds the write lock under a second on a store of 770 MB. The
    /// Machine server runs it at start and about once a day; nothing else
    /// does. A history that cannot be measured is a gap.
    pub fn refresh_statistics(&self) -> StoreResult<()> {
        const OPTIMIZE: &str = "PRAGMA analysis_limit=400; PRAGMA main.optimize=0x10002;";
        self.conn
            .lock()
            .expect("store mutex poisoned")
            .execute_batch(OPTIMIZE)?;
        self.history
            .write("statistics", |tx| Ok(tx.execute_batch(OPTIMIZE)?));
        Ok(())
    }

    pub fn home_dir(&self) -> StoreResult<PathBuf> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        home_dir_in(&conn)
    }

    /// A store over a fixture's own connection, with its history attached as
    /// an opened store's is.
    #[cfg(any(test, feature = "test-support"))]
    pub fn from_fixture_connection(
        conn: Connection,
        token_key: token_crypto::TokenKeySource,
    ) -> Self {
        let path = conn
            .path()
            .filter(|path| !path.is_empty())
            .map(PathBuf::from);
        match &path {
            Some(path) => crate::history::attach(&conn, path),
            None => crate::history::attach_empty(&conn),
        }
        Self::with_history(conn, token_key, path.as_deref())
    }

    /// A store over `conn`, whose history lives beside `path` and is already
    /// attached.
    fn with_history(
        conn: Connection,
        token_key: token_crypto::TokenKeySource,
        path: Option<&Path>,
    ) -> Self {
        Self {
            conn: Arc::new(Mutex::new(conn)),
            token_key,
            history: crate::history::HistoryFile::beside(path),
        }
    }

    /// Run `write` on this store's `history.db`, never under the store's write
    /// lock. A failure is a recording gap for `what`, and `None`.
    pub(crate) fn write_history<T>(
        &self,
        what: &str,
        write: impl FnOnce(&rusqlite::Transaction<'_>) -> StoreResult<T>,
    ) -> Option<T> {
        self.history.write(what, write)
    }

    /// Run `sql` on this store's history file, with the store attached as
    /// `store`, for fixtures that seed history and turn rows directly.
    #[cfg(any(test, feature = "test-support"))]
    pub fn execute_history(&self, sql: &str, params: impl rusqlite::Params) -> StoreResult<usize> {
        self.write_history("fixture", |tx| Ok(tx.execute(sql, params)?))
            .ok_or_else(|| StoreError::InvalidData("fixture history write failed".into()))
    }

    /// Append `rows` to Session history in one transaction, after the state
    /// they describe has committed; a failure is a recording gap.
    pub(crate) fn append_history(&self, what: &str, rows: &[crate::history::Row]) {
        if rows.is_empty() {
            return;
        }
        self.write_history(what, |tx| {
            for row in rows {
                crate::history::append_in(tx, row)?;
            }
            Ok(())
        });
    }
}

fn home_dir_in(conn: &Connection) -> StoreResult<PathBuf> {
    conn.path()
        .and_then(|path| Path::new(path).parent())
        .map(Path::to_path_buf)
        .ok_or_else(|| StoreError::InvalidData("store has no owning Machine path".into()))
}

/// A Task's checkout on this Machine, usable without project metadata.
#[derive(Debug, Clone)]
pub struct TaskCheckout {
    pub task_id: TaskId,
    pub issue_identifier: String,
    pub worktree: PathBuf,
}

/// Stable identity fields for observation, independent of execution schema.
#[derive(Debug)]
pub struct WorkIdentity {
    pub work: WorkRef,
    pub parent: Option<WorkRef>,
    pub subject: String,
    pub external_id: Option<String>,
}

pub fn read_nonterminal_task_worktrees(path: &Path) -> StoreResult<Vec<PathBuf>> {
    Ok(read_task_worktree_ownership(path)?
        .into_iter()
        .filter(|owner| owner.unfinished)
        .map(|owner| owner.path)
        .collect())
}

#[derive(Debug)]
pub struct TaskWorktreeOwnership {
    pub task: String,
    pub path: PathBuf,
    pub completed: bool,
    pub unfinished: bool,
}

/// Copy only into a fresh fixture; reopening must retain its writes and schema.
#[cfg(any(test, feature = "test-support"))]
fn copy_test_store(path: &Path) -> StoreResult<()> {
    crate::test_database::copy(path)
        .map_err(|error| StoreError::InvalidData(format!("copy test store: {error}")))
}

/// Open a registry read-only without migrating it.
fn open_unmigrated(path: &Path) -> StoreResult<Connection> {
    let conn = Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
            | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX
            | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )?;
    crate::instrumentation::observe_sqlite(&conn);
    conn.execute_batch("PRAGMA query_only = ON; PRAGMA busy_timeout = 5000;")?;
    Ok(conn)
}

/// SQL for an open Task and a Done Task over alias `t`, for this registry's schema.
fn task_state_sql(conn: &Connection) -> StoreResult<(String, &'static str)> {
    // This guard reads the installed release without migrating it. Its completion
    // owner may still be the Workflow, rather than the candidate's planning field.
    let planning_completion: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('tasks') WHERE name='planning_completed')",
        [],
        |row| row.get(0),
    )?;
    let open = if planning_completion {
        durable::task_open_sql("t")
    } else {
        "t.abandoned_at IS NULL AND NOT EXISTS(SELECT 1 FROM task_workflows wf \
         WHERE wf.task_id=t.id AND wf.node='end' AND wf.edge IS NULL)"
            .to_owned()
    };
    let done = if planning_completion {
        "t.abandoned_at IS NULL AND t.planning_completed IS 1"
    } else {
        "t.abandoned_at IS NULL AND EXISTS(SELECT 1 FROM task_workflows wf \
         WHERE wf.task_id=t.id AND wf.node='end' AND wf.edge IS NULL)"
    };
    Ok((open, done))
}

/// Every branch a Task names, on the Task or any of its PRs, and whether
/// that Task is still open.
pub fn read_task_branches(path: &Path) -> StoreResult<Vec<(String, bool)>> {
    let conn = open_unmigrated(path)?;
    let (open, _) = task_state_sql(&conn)?;
    let checkouts: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='task_checkouts')",
        [],
        |row| row.get(0),
    )?;
    // A registry from before `task_checkouts` keeps the branch on the Task
    // (removed by Task 538340).
    let checkout_branches = if checkouts {
        "SELECT task_id, branch FROM task_checkouts"
    } else {
        "SELECT id AS task_id, branch FROM tasks"
    };
    let mut statement = conn.prepare(&format!(
        "SELECT b.branch,({open}) FROM tasks t JOIN \
         ({checkout_branches} UNION SELECT task_id, branch FROM task_prs) b \
         ON b.task_id=t.id"
    ))?;
    let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
    rows.map(|row| row.map_err(StoreError::from)).collect()
}

pub fn read_task_worktree_ownership(path: &Path) -> StoreResult<Vec<TaskWorktreeOwnership>> {
    let conn = open_unmigrated(path)?;
    let (open, done) = task_state_sql(&conn)?;
    // Old registries still protect unfinished work, but cannot grant deletion
    // until they can establish this machine's positive checkout ownership.
    let (checkouts, placed, local_flag, local_route): (bool, bool, bool, bool) = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='task_checkouts'), \
         EXISTS(SELECT 1 FROM pragma_table_info('tasks') WHERE name='checkout_machine_id'), \
         EXISTS(SELECT 1 FROM pragma_table_info('machines') WHERE name='is_local'), \
         EXISTS(SELECT 1 FROM pragma_table_info('machines') WHERE name='route')",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )?;
    let query = if checkouts {
        format!(
            "SELECT t.id,c.worktree,({done}),({open}) FROM task_checkouts c \
             JOIN tasks t ON t.id=c.task_id"
        )
    } else {
        // A registry from before `task_checkouts` (removed by Task 538340).
        let local = if placed && local_flag {
            "COALESCE(t.checkout_machine_id=(SELECT id FROM machines WHERE is_local=1),0)"
        } else if placed && local_route {
            "COALESCE(t.checkout_machine_id=(SELECT id FROM machines WHERE route='local'),0)"
        } else {
            "0"
        };
        format!(
            "SELECT t.id,t.worktree,({done}) AND ({local}),({open}) FROM tasks t \
             WHERE t.worktree IS NOT NULL AND t.worktree != ''"
        )
    };
    let mut statement = conn.prepare(&query)?;
    let rows = statement.query_map([], |row| {
        Ok(TaskWorktreeOwnership {
            task: row.get(0)?,
            path: PathBuf::from(row.get::<_, String>(1)?),
            completed: row.get(2)?,
            unfinished: row.get(3)?,
        })
    })?;
    rows.map(|row| row.map_err(StoreError::from)).collect()
}

fn migrate_plaintext_provider_tokens(
    conn: &mut Connection,
    token_key: token_crypto::TokenKeySource,
) -> StoreResult<()> {
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
        let encrypted_access =
            token_crypto::encrypt_token(&access_token, token_key).map_err(|error| {
                StoreError::InvalidData(format!(
                    "failed to encrypt existing access token for provider '{provider}': {error}"
                ))
            })?;
        let encrypted_refresh = token_crypto::encrypt_optional(refresh_token.as_deref(), token_key)
            .map_err(|error| {
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

fn decrypt_token_row(
    row: TokenRow,
    token_key: token_crypto::TokenKeySource,
) -> StoreResult<super::ProviderToken> {
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
    let access_token = token_crypto::decrypt_if_needed(&access_token, encrypted, token_key)
        .map_err(|error| {
            StoreError::InvalidData(format!(
                "failed to decrypt access token for provider '{provider}': {error}"
            ))
        })?;
    let refresh_token = refresh_token
        .as_deref()
        .map(|token| token_crypto::decrypt_if_needed(token, encrypted, token_key))
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

const PROVIDER_ACCOUNT_COLUMNS: &str = "provider, account_id, home, credential_state,
    utilization_percent, cooldown_until, cooldown_reason, last_selected_at,
    created_at, updated_at, observed_email, observed_subject, observed_plan, observed_credential_digest";

fn read_provider_account(row: &rusqlite::Row) -> StoreResult<ProviderAccount> {
    Ok(ProviderAccount {
        provider: row.get(0)?,
        account_id: ProviderAccountId::parse(&row.get::<_, String>(1)?)
            .map_err(StoreError::InvalidData)?,
        home: row.get::<_, Option<String>>(2)?.map(PathBuf::from),
        credential_state: CredentialState::from_db(&row.get::<_, String>(3)?)
            .map_err(StoreError::InvalidData)?,
        utilization_percent: row.get(4)?,
        cooldown_until: row.get(5)?,
        cooldown_reason: row.get(6)?,
        last_selected_at: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
        observed_email: row.get(10)?,
        observed_subject: row.get(11)?,
        observed_plan: row.get(12)?,
        observed_credential_digest: row.get(13)?,
        // Historical labels are applied from one configuration read per operation.
        login_email: None,
    })
}

fn read_access_profile(row: &rusqlite::Row) -> rusqlite::Result<StoreResult<AccessProfile>> {
    let profile_id = row.get::<_, String>(0)?;
    let chrome_directory = row.get(1)?;
    let expected_login = row.get::<_, Option<String>>(2)?;
    let created_at = row.get(3)?;
    let updated_at = row.get(4)?;
    Ok(ProfileId::parse(&profile_id)
        .map_err(StoreError::InvalidData)
        .and_then(|id| {
            expected_login
                .as_deref()
                .map(EmailAddress::parse)
                .transpose()
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

impl SqliteStore {
    #[cfg(any(test, feature = "test-support"))]
    pub fn assert_no_historical_runs(&self) {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='runs'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            count, 0,
            "current execution must not recreate the retired Run table"
        );
    }

    /// Revalidate a connection after a child executable may have upgraded it.
    pub fn validate_current_schema(
        &self,
        build: &super::migration_runner::MigrationBuild,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        super::migration_runner::validate_sqlite_schema(build, &conn)
    }

    /// Open a hermetic, fully-migrated store at `path`: this build's whole
    /// migration catalog, reading **no**
    /// process- or machine-global state — no `LF_HOME`, no install selection, no
    /// shared `~/.lf` identity, no frontier authority. Tests use this so their
    /// schema is deterministic under parallel execution; the application
    /// opener resolves real install/frontier authority and is what
    /// races when tests mutate ambient env concurrently.
    pub fn open_ephemeral_prepared(
        path: &Path,
        build: &super::migration_runner::MigrationBuild,
        observers: &super::migration_runner::MigrationObservers<'_>,
        token_key: token_crypto::TokenKeySource,
    ) -> StoreResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                StoreError::InvalidData(format!("failed to create db dir: {error}"))
            })?;
        }
        #[cfg(any(test, feature = "test-support"))]
        copy_test_store(path)?;
        let mut conn = Connection::open(path)?;
        crate::instrumentation::observe_sqlite(&conn);
        configure_write_connection(&mut conn, path, SQLITE_WRITE_BUSY_TIMEOUT)?;
        super::migration_runner::apply_sqlite(build, &conn, observers)?;
        crate::history::attach(&conn, path);
        Ok(Self::with_history(conn, token_key, Some(path)))
    }

    /// Open with the application's selected frontier authority and upgrade observations.
    pub fn open_prepared(
        path: &Path,
        shared_database: bool,
        may_apply_migrations: bool,
        busy: Duration,
        build: &super::migration_runner::MigrationBuild,
        observers: &super::migration_runner::MigrationObservers<'_>,
        token_key: token_crypto::TokenKeySource,
    ) -> StoreResult<Self> {
        let existing_database = std::fs::metadata(path).is_ok_and(|metadata| metadata.len() > 0);
        if !may_apply_migrations && !existing_database {
            return Err(StoreError::InvalidData(format!(
                "shared store {} is not initialized and an ordinary lf may not create it; \
                 install lf with `lf self install`",
                path.display()
            )));
        }

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| {
                StoreError::InvalidData(format!("failed to create db dir: {err}"))
            })?;
        }

        #[cfg(test)]
        if !existing_database {
            copy_test_store(path)?;
        }
        let mut conn = Connection::open(path)?;
        crate::instrumentation::observe_sqlite(&conn);
        // Install the handler before journal-mode negotiation: that pragma can
        // itself meet another process opening the same WAL database.
        configure_write_connection(&mut conn, path, busy)?;

        if shared_database && !may_apply_migrations {
            // Validate the applied history first (preserving divergent/incompatible
            // and store-ahead errors), then refuse if this binary knows a migration
            // the store has not applied. An ordinary open must not hand back a store
            // whose schema is older than this binary's code, which may query the
            // columns that pending migration adds.
            super::migration_runner::validate_sqlite_schema(build, &conn)?;
            if let Some(pending) = super::migration_runner::pending_shared_migration(build, &conn)?
            {
                return Err(StoreError::InvalidData(unapplied_message(path, &pending)));
            }
        } else if existing_database {
            super::migration_runner::apply_sqlite_with_backup(build, &conn, path, observers)?;
            if !shared_database {
                // A private home holds exactly this build's schema; one changed
                // outside lf is never repaired.
                super::migration_runner::validate_sqlite_schema(build, &conn).map_err(|error| {
                    StoreError::InvalidData(format!(
                        "{error}\nDatabase: {}\nA private LF_HOME is disposable: start a fresh one; \
                         lf will not repair it.",
                        path.display()
                    ))
                })?;
            }
        } else {
            super::migration_runner::apply_sqlite(build, &conn, observers)?;
        }
        if may_apply_migrations {
            migrate_plaintext_provider_tokens(&mut conn, token_key)?;
        }
        crate::history::attach(&conn, path);
        Ok(Self::with_history(conn, token_key, Some(path)))
    }

    /// Open without schema or token writes: readers use this when a source
    /// build may be older or newer than the Machine's database.
    pub fn open_read_only(
        path: &Path,
        token_key: token_crypto::TokenKeySource,
    ) -> StoreResult<Self> {
        let conn = Connection::open_with_flags(
            path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
                | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX
                | rusqlite::OpenFlags::SQLITE_OPEN_URI,
        )?;
        crate::instrumentation::observe_sqlite(&conn);
        conn.execute_batch("PRAGMA query_only = ON; PRAGMA busy_timeout = 5000;")?;
        crate::history::attach(&conn, path);
        Ok(Self::with_history(conn, token_key, Some(path)))
    }

    pub fn work_identities(&self) -> StoreResult<Vec<WorkIdentity>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let conn = conn.savepoint()?;
        let mut statement = conn.prepare(
            "SELECT 0 AS kind, id, NULL AS parent, slug, NULL AS external_id FROM wave_addresses
             UNION ALL
             SELECT 1, id, wave_id, project_slug, external_project_id FROM projects
             UNION ALL
             SELECT 2, id, project_id, issue_identifier, external_issue_id FROM tasks
             ORDER BY kind",
        )?;
        let rows = statement.query_map([], |row| {
            let id: String = row.get(1)?;
            let (work, parent) = match row.get::<_, u8>(0)? {
                0 => (WorkRef::Wave(row.get(1)?), None),
                1 => (
                    WorkRef::Project(ProjectId::from_raw(id)),
                    Some(WorkRef::Wave(row.get(2)?)),
                ),
                2 => (
                    WorkRef::Task(TaskId::from_raw(id)),
                    row.get::<_, Option<String>>(2)?
                        .map(|id| WorkRef::Project(ProjectId::from_raw(id))),
                ),
                _ => unreachable!("identity query selects only Wave, Project, and Task"),
            };
            Ok(WorkIdentity {
                work,
                parent,
                subject: row.get(3)?,
                external_id: row.get(4)?,
            })
        })?;
        let mut identities = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        for identity in &mut identities {
            if let WorkRef::Task(id) = &identity.work {
                identity.subject =
                    children::short_task_id(&conn, id)?.ok_or(StoreError::NotFound)?;
            }
        }
        Ok(identities)
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn apply_migration_for_test(&self, name: &str) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        if crate::migration_runner::migration_is_applied_for_test(&conn, name)? {
            return Ok(());
        }
        conn.execute_batch(&crate::migration_runner::migration_sql_for_test(name))?;
        Ok(())
    }

    pub fn upsert_wave_prepared(
        &self,
        wave: &Wave,
        registration: Option<(
            &repositories::RepositoryRegistration,
            &loopflow_model::durable::RepositoryId,
        )>,
    ) -> StoreResult<bool> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some((inputs, id)) = registration {
            if !repositories::register_in(&tx, wave.repo(), inputs, id, false)? {
                return Ok(false);
            }
        }
        let created_at = wave
            .created_at()
            .map(|dt| dt.unix_timestamp())
            .unwrap_or_else(now_unix);

        validate_wave_parent(
            &tx,
            wave.id(),
            wave.name(),
            wave.repo(),
            wave.parent_wave_id(),
        )?;
        let parent: Option<Option<String>> = tx
            .query_row(
                "SELECT parent_wave_id FROM waves WHERE id=?1",
                [wave.id()],
                |r| r.get(0),
            )
            .optional()?;
        let saved = parent.is_some();
        let edits = if saved {
            vec![crate::work::edit::WorkValue::WaveParent(
                parent
                    .flatten()
                    .or_else(|| wave.parent_wave_id().map(ToString::to_string)),
            )]
        } else {
            vec![
                crate::work::edit::WorkValue::WaveName(wave.name().into()),
                crate::work::edit::WorkValue::WaveParent(
                    wave.parent_wave_id().map(ToString::to_string),
                ),
            ]
        };
        planning_write::create(
            &tx,
            wave.repo(),
            &crate::work::edit::WorkEdit::for_target(wave.id().as_str(), &edits),
            crate::sqlite::planning_write::WriteOrigin::Local,
        )?;
        if !saved {
            tx.execute("UPDATE waves SET created_at=?2,retired_at=?3,superseded_by_wave_id=?4,retirement_reason=?5 WHERE id=?1",params![wave.id(),created_at,wave.retired_at().map(|at|at.unix_timestamp()),wave.superseded_by_wave_id(),wave.retirement_reason()])?;
        }
        tx.execute(
            "UPDATE waves SET promoted_at=COALESCE(promoted_at,?2) WHERE id=?1",
            params![wave.id(), wave.promoted_at().map(|at| at.unix_timestamp())],
        )?;
        tx.commit()?;
        Ok(true)
    }
}

fn validate_wave_parent(
    conn: &Connection,
    id: &WaveId,
    name: &str,
    repo: &str,
    parent: Option<&WaveId>,
) -> StoreResult<()> {
    if name.is_empty() || name.contains(['/', '\\']) || matches!(name, "." | "..") {
        return Err(StoreError::InvalidData(format!(
            "Wave name must be one segment: {name:?}"
        )));
    }
    if let Some(parent) = parent {
        let parent_repo: String = conn.query_row(
            "SELECT repo FROM waves WHERE id=?1 AND retired_at IS NULL",
            params![parent],
            |row| row.get(0),
        )?;
        if parent_repo != repo {
            return Err(StoreError::InvalidData(
                "Wave directory parent belongs to another repository".into(),
            ));
        }
        let cycle: bool = conn.query_row(
            "WITH RECURSIVE ancestors(id, parent_wave_id) AS (
                SELECT id, parent_wave_id FROM waves WHERE id = ?1
                UNION
                SELECT w.id, w.parent_wave_id FROM waves w
                JOIN ancestors a ON w.id = a.parent_wave_id
             ) SELECT EXISTS(SELECT 1 FROM ancestors WHERE id = ?2)",
            params![parent, id],
            |row| row.get(0),
        )?;
        if cycle {
            return Err(StoreError::InvalidData(
                "Wave parent would create a cycle".into(),
            ));
        }
    }
    Ok(())
}

impl SqliteStore {
    pub fn health_check(&self) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row("SELECT 1", [], |_| Ok(()))?;
        super::migration_runner::validate_persisted_json(&conn)
    }

    pub fn schema_version(
        &self,
        build: &super::migration_runner::MigrationBuild,
    ) -> StoreResult<String> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        super::migration_runner::latest_version_sqlite(build, &conn)
    }

    pub fn applied_schema_version(&self) -> StoreResult<Option<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        super::migration_runner::latest_applied_version_sqlite(&conn)
    }

    // -- Provider tokens -------------------------------------------------------

    pub fn provider_auth_snapshot(
        &self,
        provider: Provider,
    ) -> StoreResult<Option<crate::provider::ProviderAuthSnapshot>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            "SELECT login, expires_at, credential_type FROM provider_tokens WHERE provider = ?1",
            [provider.as_str()],
            |row| {
                let login: Option<String> = row.get(0)?;
                let expires_at: Option<i64> = row.get(1)?;
                let credential_type: String = row.get(2)?;
                Ok((login, expires_at, credential_type))
            },
        )
        .optional()?
        .map(|(login, expires_at, credential_type)| {
            Ok(crate::provider::ProviderAuthSnapshot {
                provider,
                status: if expires_at.is_some_and(|expiry| expiry <= now_unix()) {
                    crate::provider::AuthStatus::Expired
                } else {
                    crate::provider::AuthStatus::Active { login }
                },
                expires_at,
                next_refresh_at: None,
                credential_type: Some(super::CredentialType::from_db(&credential_type)),
            })
        })
        .transpose()
    }

    pub fn get_provider_token(&self, provider: &str) -> StoreResult<Option<super::ProviderToken>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT provider, access_token, refresh_token, oauth_client_id, expires_at, login, updated_at, credential_type, encrypted
             FROM provider_tokens WHERE provider = ?1",
        )?;
        let row = stmt
            .query_row(params![provider], read_token_row)
            .optional()?;

        row.map(|row| decrypt_token_row(row, self.token_key))
            .transpose()
    }

    pub fn upsert_provider_token(&self, token: &super::ProviderToken) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let encrypted_access = token_crypto::encrypt_token(&token.access_token, self.token_key)
            .map_err(|error| {
                StoreError::InvalidData(format!(
                    "failed to encrypt access token for provider '{}': {error}",
                    token.provider
                ))
            })?;
        let encrypted_refresh =
            token_crypto::encrypt_optional(token.refresh_token.as_deref(), self.token_key)
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

    pub fn replace_provider_token(
        &self,
        expected: &super::ProviderToken,
        replacement: &super::ProviderToken,
        deadline: std::time::Instant,
    ) -> StoreResult<super::ProviderTokenReplacement> {
        let expired = || StoreError::InvalidData("credential replacement deadline elapsed".into());
        let access = token_crypto::encrypt_token(&replacement.access_token, self.token_key)
            .map_err(|_| StoreError::InvalidData("credential encryption failed".into()))?;
        let refresh =
            token_crypto::encrypt_optional(replacement.refresh_token.as_deref(), self.token_key)
                .map_err(|_| StoreError::InvalidData("credential encryption failed".into()))?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let previous: u32 = conn.pragma_query_value(None, "busy_timeout", |row| row.get(0))?;
        let remaining = deadline
            .checked_duration_since(std::time::Instant::now())
            .ok_or_else(expired)?;
        conn.busy_timeout(remaining)?;
        let result = (|| {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            if std::time::Instant::now() >= deadline {
                return Err(expired());
            }
            let row = tx.query_row(
                "SELECT provider, access_token, refresh_token, oauth_client_id, expires_at, login, updated_at, credential_type, encrypted
                 FROM provider_tokens WHERE provider = ?1",
                params![expected.provider], read_token_row,
            ).optional()?.map(|row| decrypt_token_row(row, self.token_key)).transpose()?;
            let Some(current) = row else {
                return Ok(ProviderTokenReplacement::Missing);
            };
            if current != *expected {
                return Ok(ProviderTokenReplacement::Changed(current));
            }
            if replacement
                .expires_at
                .is_some_and(|expiry| expiry <= now_unix())
            {
                return Err(StoreError::InvalidData(
                    "refreshed credential expired before persistence".into(),
                ));
            }
            tx.execute(
                "UPDATE provider_tokens SET access_token=?2, refresh_token=?3, oauth_client_id=?4,
                 expires_at=?5, login=?6, updated_at=?7, credential_type=?8, encrypted=1 WHERE provider=?1",
                params![expected.provider, access, refresh, replacement.oauth_client_id,
                    replacement.expires_at, replacement.login, replacement.updated_at,
                    replacement.credential_type.as_str()],
            )?;
            tx.commit()?;
            Ok(ProviderTokenReplacement::Replaced)
        })();
        conn.busy_timeout(Duration::from_millis(u64::from(previous)))?;
        result
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
            tokens.push(decrypt_token_row(row?, self.token_key)?);
        }
        Ok(tokens)
    }

    // -- Provider accounts -----------------------------------------------------

    pub fn upsert_provider_account_row(&self, account: &ProviderAccount) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute("INSERT INTO provider_accounts(provider,account_id,home,credential_state,utilization_percent,cooldown_until,cooldown_reason,last_selected_at,created_at,updated_at,observed_email,observed_subject,observed_plan,observed_credential_digest)
            VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)
            ON CONFLICT(provider,account_id) DO UPDATE SET capacity_revision=provider_accounts.capacity_revision+1,home=excluded.home,credential_state=excluded.credential_state,utilization_percent=excluded.utilization_percent,cooldown_until=excluded.cooldown_until,cooldown_reason=excluded.cooldown_reason,last_selected_at=excluded.last_selected_at,updated_at=excluded.updated_at,observed_email=excluded.observed_email,observed_subject=excluded.observed_subject,observed_plan=excluded.observed_plan,observed_credential_digest=excluded.observed_credential_digest",
            params![account.provider,account.account_id.as_str(),account.home.as_ref().map(|path| path.to_string_lossy().to_string()),account.credential_state.as_str(),account.utilization_percent,account.cooldown_until,account.cooldown_reason,account.last_selected_at,account.created_at,account.updated_at,account.observed_email,account.observed_subject,account.observed_plan,account.observed_credential_digest])?;
        Ok(())
    }

    pub fn provider_account_row(
        &self,
        provider: &str,
        account_id: &ProviderAccountId,
    ) -> StoreResult<Option<ProviderAccount>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            &format!("SELECT {PROVIDER_ACCOUNT_COLUMNS} FROM provider_accounts WHERE provider = ?1 AND account_id = ?2"),
        )?;
        statement
            .query_row(params![provider, account_id.as_str()], |row| {
                Ok(read_provider_account(row))
            })
            .optional()?
            .transpose()
    }

    pub fn provider_account_rows(
        &self,
        provider: Option<&str>,
    ) -> StoreResult<Vec<ProviderAccount>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(&format!(
            "SELECT {PROVIDER_ACCOUNT_COLUMNS} FROM provider_accounts
             WHERE (?1 IS NULL OR provider = ?1) ORDER BY provider, account_id"
        ))?;
        let accounts = statement.query_map([provider], |row| Ok(read_provider_account(row)))?;
        accounts.map(|row| row?).collect()
    }

    // -- Session moves --------------------------------------------------------

    /// Leave a move for a busy Session; its next Stop hook applies it.
    pub fn record_session_move(&self, session: &str, agent: &str) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO session_moves (session_id, agent, requested_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(session_id) DO UPDATE SET
                agent = excluded.agent,
                requested_at = excluded.requested_at",
            params![session, agent, now_unix()],
        )?;
        Ok(())
    }

    /// The agent a move left for `session` names, if one waits.
    pub fn pending_session_move(&self, session: &str) -> StoreResult<Option<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT agent FROM session_moves WHERE session_id = ?1",
                [session],
                |row| row.get::<_, String>(0),
            )
            .optional()?)
    }

    /// Clear an applied move. A move re-requested for another agent meanwhile stays.
    pub fn clear_session_move(&self, session: &str, agent: &str) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "DELETE FROM session_moves WHERE session_id = ?1 AND agent = ?2",
            [session, agent],
        )?;
        Ok(())
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
                profile.expected_login.as_ref().map(EmailAddress::as_str),
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

    /// Transcript adoption changes its location, never its historical account.
    pub fn record_native_conversation_home(
        &self,
        provider: Provider,
        thread: &AgentSessionId,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE provider_session_accounts SET isolated = 0 WHERE provider = ?1 AND provider_session_id = ?2",
            params![provider.as_str(), thread],
        )?;
        Ok(())
    }

    /// The account a launch last ran this conversation on.
    pub fn provider_session_account(
        &self,
        provider: Provider,
        agent_session: &AgentSessionId,
    ) -> StoreResult<Option<ProviderAccountId>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            "SELECT account_id FROM provider_session_accounts
             WHERE provider = ?1 AND provider_session_id = ?2",
            params![provider.as_str(), agent_session],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .as_deref()
        .map(ProviderAccountId::parse)
        .transpose()
        .map_err(StoreError::InvalidData)
    }

    /// The account whose own home still holds this conversation: it ran
    /// there before every launch shared one, and has not resumed since.
    pub fn provider_session_account_home(
        &self,
        provider: Provider,
        agent_session: &AgentSessionId,
    ) -> StoreResult<Option<ProviderAccountId>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            "SELECT account_id FROM provider_session_accounts
             WHERE provider = ?1 AND provider_session_id = ?2 AND isolated = 1",
            params![provider.as_str(), agent_session],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .as_deref()
        .map(ProviderAccountId::parse)
        .transpose()
        .map_err(StoreError::InvalidData)
    }

    /// Record that `agent_session` ran in `account_id`'s own home, as every
    /// conversation did before launches shared one. Nothing writes this now.
    #[cfg(any(test, feature = "test-support"))]
    pub fn record_account_home_conversation(
        &self,
        provider: Provider,
        agent_session: &str,
        account_id: &str,
    ) {
        self.conn
            .lock()
            .expect("store mutex poisoned")
            .execute(
                "INSERT INTO provider_session_accounts
                    (provider, provider_session_id, account_id, created_at, isolated)
                 VALUES (?1, ?2, ?3, 1, 1)",
                params![provider.as_str(), agent_session, account_id],
            )
            .expect("record account-home conversation");
    }

    pub fn list_waves(&self, repo: Option<&str>) -> StoreResult<Vec<Wave>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        waves_in(&conn, repo)
    }

    pub fn list_child_waves(&self, parent: &WaveId) -> StoreResult<Vec<Wave>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, name, repo, created_at, parent_wave_id, promoted_at,
                    retired_at, superseded_by_wave_id, retirement_reason, slug
             FROM wave_addresses
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
                    retired_at, superseded_by_wave_id, retirement_reason, slug
             FROM wave_addresses WHERE id = ?1",
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
                    retired_at, superseded_by_wave_id, retirement_reason, slug
             FROM wave_addresses
             WHERE repo = ?1 AND slug = ?2 AND retired_at IS NULL",
        )?;
        let wave = stmt
            .query_row(params![locator.repo().to_string(), locator.slug()], |row| {
                Ok(map_wave_row(row))
            })
            .optional()?;
        wave.transpose()
    }

    pub fn find_waves_by_slug(&self, slug: &str) -> StoreResult<Vec<Wave>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, name, repo, created_at, parent_wave_id, promoted_at,
                    retired_at, superseded_by_wave_id, retirement_reason, slug
             FROM wave_addresses
             WHERE slug = ?1 AND retired_at IS NULL
             ORDER BY repo",
        )?;
        let rows = stmt.query_map(params![slug], |row| Ok(map_wave_row(row)))?;
        let mut waves = Vec::new();
        for wave in rows {
            waves.push(wave??);
        }
        Ok(waves)
    }

    pub fn relocate_waves(&self, updates: &[WaveLocatorUpdate]) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        for update in updates {
            let current = tx
                .query_row(
                    "SELECT repo, slug FROM wave_addresses WHERE id = ?1 AND retired_at IS NULL",
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
                    "SELECT id FROM wave_addresses
                     WHERE repo = ?1 AND slug = ?2 AND id != ?3
                       AND retired_at IS NULL",
                    params![
                        update.target.repo().to_string(),
                        update.target.slug(),
                        update.wave_id
                    ],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            if let Some(collision) = collision {
                return Err(StoreError::InvalidData(format!(
                    "Wave {}/{} already exists as {collision}",
                    update.target.repo(),
                    update.target.slug()
                )));
            }
        }

        for update in updates {
            let (parent_slug, name) = update
                .target
                .slug()
                .rsplit_once('/')
                .map_or((None, update.target.slug()), |(parent, name)| {
                    (Some(parent), name)
                });
            let repo = update.target.repo().to_string();
            let parent: Option<WaveId> = match parent_slug {
                Some(slug) => Some(tx.query_row(
                    "SELECT id FROM wave_addresses
                     WHERE repo = ?1 AND slug = ?2 AND retired_at IS NULL",
                    params![repo, slug],
                    |row| row.get(0),
                )?),
                None => None,
            };
            validate_wave_parent(&tx, &update.wave_id, name, &repo, parent.as_ref())?;
            tx.execute(
                "UPDATE waves SET repo=?2 WHERE id=?1 AND repo IS NOT ?2",
                params![update.wave_id, repo],
            )?;
            planning_write::local(
                &tx,
                &crate::work::edit::WorkEdit::for_target(
                    update.wave_id.as_str(),
                    &[
                        crate::work::edit::WorkValue::WaveName(name.into()),
                        crate::work::edit::WorkValue::WaveParent(parent.map(|p| p.to_string())),
                    ],
                ),
            )?;
        }
        tx.commit()?;
        Ok(())
    }

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

    /// Write a Process row the way a released lf does; fixtures for the
    /// upgrade bridge and the released-lf readers LOO-965 deletes seed with it.
    #[cfg(any(test, feature = "test-support"))]
    pub fn record_process(&self, process: &crate::process::LfProcess) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO processes(id,trace_id,parent_lf_process_id,command,repo,cwd,started_at,
                via_agent,caller_session_id,caller_agent_process_id,completed_at,outcome,exit_code,signal,error,pid,os_started_at)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)
             ON CONFLICT(id) DO UPDATE SET completed_at=excluded.completed_at,
                outcome=excluded.outcome,exit_code=excluded.exit_code,signal=excluded.signal,error=excluded.error
             WHERE processes.completed_at IS NULL AND excluded.completed_at IS NOT NULL",
            params![process.id,process.trace_id,process.parent_lf_process_id,process.command,process.repo,process.cwd,
                process.started_at,process.via_agent,process.caller_session_id,process.caller_agent_process_id,
                process.completed_at,process.outcome,process.exit_code,process.signal,process.error,process.pid,
                process.os_started_at],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod fixture_tests {
    use super::SqliteStore;

    #[test]
    fn concurrent_copies_share_one_initialized_machine() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("store.db");
        let start = std::sync::Barrier::new(8);
        let machines = std::thread::scope(|scope| {
            let workers: Vec<_> = (0..8)
                .map(|_| {
                    scope.spawn(|| {
                        start.wait();
                        super::copy_test_store(&path).unwrap();
                        rusqlite::Connection::open(&path)
                            .unwrap()
                            .query_row("SELECT id FROM machines WHERE is_local=1", [], |row| {
                                row.get::<_, String>(0)
                            })
                            .unwrap()
                    })
                })
                .collect();
            workers
                .into_iter()
                .map(|worker| worker.join().unwrap())
                .collect::<Vec<_>>()
        });
        assert!(machines.iter().all(|machine| machine == &machines[0]));
    }

    #[test]
    fn test_store_image_matches_migrations_and_copies_are_independent() {
        let migrated = rusqlite::Connection::open_in_memory().unwrap();
        crate::migration_runner::apply_sqlite(
            &crate::test_fixture::build(),
            &migrated,
            &crate::test_fixture::observers(),
        )
        .unwrap();
        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("first.db");
        let second = directory.path().join("second.db");
        super::copy_test_store(&first).unwrap();
        super::copy_test_store(&second).unwrap();
        let image = rusqlite::Connection::open(&first).unwrap();
        let schema = crate::migration_schema::product_schema(&migrated).unwrap();
        assert_eq!(
            *schema,
            *crate::migration_schema::product_schema(&image).unwrap()
        );
        for table in schema.iter().filter(|object| object.object_type == "table") {
            let rows = |conn: &rusqlite::Connection| {
                let mut query = conn
                    .prepare(&format!("SELECT * FROM [{}]", table.name))
                    .unwrap();
                let columns = query.column_count();
                let mut rows: Vec<_> = query
                    .query_map([], |row| {
                        (if table.name == "machines" { 3 } else { 0 }..columns)
                            .map(|column| row.get::<_, rusqlite::types::Value>(column))
                            .collect::<rusqlite::Result<Vec<_>>>()
                    })
                    .unwrap()
                    .map(|row| format!("{:?}", row.unwrap()))
                    .collect();
                rows.sort();
                rows
            };
            assert_eq!(rows(&migrated), rows(&image), "{}", table.name);
        }
        let machine = |conn: &rusqlite::Connection| {
            conn.query_row("SELECT id FROM machines WHERE is_local=1", [], |row| {
                row.get::<_, String>(0)
            })
            .unwrap()
        };
        assert_ne!(
            machine(&image),
            machine(&rusqlite::Connection::open(&second).unwrap())
        );
        image
            .execute_batch("CREATE TABLE fixture_write(value); INSERT INTO fixture_write VALUES(1)")
            .unwrap();
        super::copy_test_store(&first).unwrap();
        assert_eq!(
            image
                .query_row("SELECT value FROM fixture_write", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert!(rusqlite::Connection::open(second)
            .unwrap()
            .prepare("SELECT * FROM fixture_write")
            .is_err());
    }

    #[test]
    fn observation_reads_identity_without_execution_schema() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE waves (id TEXT, name TEXT, created_at INTEGER, parent_wave_id TEXT);
             CREATE TABLE projects (id TEXT, wave_id TEXT, project_slug TEXT, external_project_id TEXT, created_at INTEGER);
             CREATE TABLE tasks (id TEXT, project_id TEXT, issue_identifier TEXT, external_issue_id TEXT, created_at INTEGER);
             INSERT INTO waves VALUES ('00000000-0000-0000-0000-000000000001', 'product', 1, NULL);
             INSERT INTO projects VALUES ('proj_desktop', '00000000-0000-0000-0000-000000000001', 'desktop', 'linear-project', 2);
             INSERT INTO tasks VALUES ('task_watcher', 'proj_desktop', 'LOO-293', 'linear-issue', 3);
             INSERT INTO tasks VALUES ('task_abcdef10000000000000000000000001', NULL, 'lf-abcdef10000000000000000000000001', NULL, 4);",
        ).unwrap();
        let directory = tempfile::tempdir().unwrap();
        let reference = crate::test_fixture::open(&directory.path().join("reference.db")).unwrap();
        let view: String = reference
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT sql FROM sqlite_master WHERE name='wave_addresses'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        conn.execute_batch(&view).unwrap();
        conn.execute_batch("PRAGMA query_only = ON").unwrap();
        let store = SqliteStore::with_history(conn, crate::test_fixture::key, None);
        let identities = store.work_identities().unwrap();
        assert_eq!(identities.len(), 4);
        assert_eq!(identities[3].subject, "abcdef");
        assert!(identities[3].parent.is_none());
        assert_eq!(identities[2].subject, "LOO-293");
        assert_eq!(identities[2].external_id.as_deref(), Some("linear-issue"));
        assert_eq!(identities[2].parent.as_ref(), Some(&identities[1].work));
        assert_eq!(identities[1].parent.as_ref(), Some(&identities[0].work));
    }
}

#[cfg(test)]
mod wal_tests {
    use super::{configure_write_connection, SQLITE_WRITE_BUSY_TIMEOUT, WAL_SIZE_LIMIT_BYTES};

    #[test]
    fn a_burst_does_not_leave_its_wal_on_disk() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("loopflow.db");
        let mut conn = rusqlite::Connection::open(&path).unwrap();
        configure_write_connection(&mut conn, &path, SQLITE_WRITE_BUSY_TIMEOUT).unwrap();
        conn.execute_batch("CREATE TABLE history (payload BLOB)")
            .unwrap();
        let wal = directory.path().join("loopflow.db-wal");

        conn.execute(
            "INSERT INTO history VALUES (zeroblob(?1))",
            [2 * WAL_SIZE_LIMIT_BYTES],
        )
        .unwrap();
        assert!(std::fs::metadata(&wal).unwrap().len() > WAL_SIZE_LIMIT_BYTES as u64);
        // The commit above checkpointed; the next write restarts the log.
        conn.execute("INSERT INTO history VALUES (x'00')", [])
            .unwrap();

        assert!(std::fs::metadata(&wal).unwrap().len() <= WAL_SIZE_LIMIT_BYTES as u64);
    }

    #[test]
    fn a_transaction_that_reads_then_writes_waits_for_a_concurrent_writer() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("loopflow.db");
        let open = || {
            let mut conn = rusqlite::Connection::open(&path).unwrap();
            configure_write_connection(&mut conn, &path, SQLITE_WRITE_BUSY_TIMEOUT).unwrap();
            conn
        };
        let mut first = open();
        first
            .execute_batch("CREATE TABLE rows (value INTEGER)")
            .unwrap();
        let tx = first.transaction().unwrap();
        let before: i64 = tx
            .query_row("SELECT COUNT(*) FROM rows", [], |row| row.get(0))
            .unwrap();
        // Another process writes while this transaction holds its read.
        let second = std::thread::spawn({
            let second = open();
            move || second.execute("INSERT INTO rows VALUES (2)", []).unwrap()
        });
        std::thread::sleep(std::time::Duration::from_millis(200));
        tx.execute("INSERT INTO rows VALUES (?1)", [before + 1])
            .unwrap();
        tx.commit().unwrap();
        second.join().unwrap();
        let count: i64 = first
            .query_row("SELECT COUNT(*) FROM rows", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 2);
    }
}
