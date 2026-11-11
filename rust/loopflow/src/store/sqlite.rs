use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::durable::{ProjectId, TaskId, WorkRef};
use crate::id::{AgentSessionId, WaveId};
use crate::profile::{
    AccessProfile, AuthBrowserBinding, EmailAddress, ProfileId, ProviderRoute, RouteScope,
};
use crate::provider_auth::Provider;
use crate::store::rows::{map_wave_row, now_unix};
use crate::store::token_crypto;
use crate::store::{
    AccountLimitRow, CredentialState, ProviderAccount, ProviderAccountId, ProviderAccountSelection,
    ProviderTokenReplacement, RoutingState, StoreError, StoreResult, WaveLocatorUpdate,
};
use crate::work::wave::{Wave, WaveLocator};

mod admission;
mod children;
pub(crate) mod ci_incidents;
pub mod creation;
mod durable;
mod github_people;
mod graph_inventory;
#[cfg(test)]
mod linear_migration_tests;
mod project_adoption;
mod subscription_events;

mod linear_team_checks;
mod machine_records;
mod metrics;
mod native_input;
pub(crate) mod planning_changes;
pub(crate) mod planning_export;
pub(crate) mod planning_order;
mod planning_peers;
mod planning_sync;
pub(crate) mod planning_write;
mod pr_landings;
mod processes;
mod program_status;
pub(crate) mod project_content;
pub(crate) mod project_selection;
pub mod provider_transitions;
mod rentals;
mod repositories;
pub(crate) mod retention;
mod revisions;
mod scope_defaults;
pub(crate) mod selections;
mod session_events;
pub(crate) mod sessions;
pub(crate) mod task_ownership;

mod steers;
mod task_comments;
mod task_content;
mod task_details;
mod task_retirement;
mod transport;
pub(crate) use task_retirement::{TaskRetirement, ViewerHold, WORKSPACE_GRACE};
pub(crate) use transport::Transport;
pub(crate) mod task_history;
pub(crate) mod task_state_delivery;
pub(crate) mod task_work;
pub(crate) mod wave_definitions;
mod wave_lifecycle;

pub use children::ClosedTaskPage;
pub(crate) use graph_inventory::{entry_of, AgentLimitRecovery, GraphExecutionFacts};
pub use project_selection::{ProjectActivation, ProjectReadiness, ProjectReadinessState};
pub use revisions::StoreRevisions;
pub use task_details::TaskDetailFacts;
pub(crate) use task_work::TaskShipping;

/// A fleet can legitimately queue longer than SQLite's common five-second
/// default while every process opens and records its first receipt. Durable
/// writes wait for that bounded local contention instead of dropping evidence.
pub(crate) const SQLITE_WRITE_BUSY_TIMEOUT: Duration = Duration::from_secs(15);

/// A provider hook at a turn or tool boundary waits this long for a locked
/// store, then treats it as unavailable: a busy install never stalls a turn.
pub(crate) const HOOK_BUSY_TIMEOUT: Duration = Duration::from_secs(1);

/// Bytes of write-ahead log kept on disk after a checkpoint resets it.
const WAL_SIZE_LIMIT_BYTES: i64 = 64 * 1024 * 1024;

/// The first rollback-to-WAL transition can return BUSY immediately even with
/// a busy handler: two readers cannot both upgrade their journal lock. Reuse
/// migration exclusion only for that transition; ordinary WAL opens stay reads.
/// Why an ordinary open refuses a shared store this build is ahead of, and the fix.
fn unapplied_message(path: &Path, pending: &str) -> String {
    if super::migrations::unmerged_migrations().contains(&pending) {
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
        let _lock = super::migrations::migration_lock(path)?;
        let mode: String = conn.pragma_query_value(None, "journal_mode", |row| row.get(0))?;
        if mode != "wal" {
            conn.pragma_update(None, "journal_mode", "WAL")?;
        }
    }
    conn.pragma_update(None, "foreign_keys", "ON")?;
    super::contention::watch_write_holds(conn)?;
    // A WAL file never shrinks by itself: one migration or burst leaves its
    // high-water mark on disk forever. Truncate it whenever a checkpoint resets it.
    conn.pragma_update(None, "journal_size_limit", WAL_SIZE_LIMIT_BYTES)?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct SqliteStore {
    pub(crate) conn: Arc<Mutex<Connection>>,
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
    pub(crate) fn home_dir(&self) -> StoreResult<PathBuf> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        home_dir_in(&conn)
    }
}

fn home_dir_in(conn: &Connection) -> StoreResult<PathBuf> {
    conn.path()
        .and_then(|path| Path::new(path).parent())
        .map(Path::to_path_buf)
        .ok_or_else(|| StoreError::InvalidData("store has no owning Machine path".into()))
}

/// Recorded checkout evidence remains usable without chapter metadata.
#[derive(Debug, Clone)]
pub(crate) struct TaskCheckout {
    pub task_id: TaskId,
    pub issue_identifier: String,
    pub worktree: PathBuf,
    pub machine_id: Option<crate::durable::MachineId>,
}

/// Stable identity fields for observation, independent of execution schema.
#[derive(Debug)]
pub(crate) struct WorkIdentity {
    pub work: WorkRef,
    pub parent: Option<WorkRef>,
    pub subject: String,
    pub external_id: Option<String>,
}

fn deleted_task_issues_in(
    conn: &Connection,
    wave_id: &WaveId,
) -> StoreResult<std::collections::HashSet<String>> {
    let mut statement = conn.prepare("SELECT issue_id FROM task_deletions WHERE wave_id=?1")?;
    let rows = statement.query_map([wave_id.as_str()], |row| row.get::<_, String>(0))?;
    rows.map(|row| row.map_err(StoreError::from)).collect()
}

pub(crate) fn read_nonterminal_task_worktrees(path: &Path) -> StoreResult<Vec<PathBuf>> {
    Ok(read_task_worktree_ownership(path)?
        .into_iter()
        .filter(|owner| owner.unfinished)
        .map(|owner| owner.path)
        .collect())
}

#[derive(Debug)]
pub(crate) struct TaskWorktreeOwnership {
    pub task: String,
    pub path: PathBuf,
    pub completed: bool,
    pub unfinished: bool,
}

/// Open a registry read-only without migrating it.
fn open_unmigrated(path: &Path) -> StoreResult<Connection> {
    let conn = Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    crate::performance::observe_sqlite(&conn);
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
pub(crate) fn read_task_branches(path: &Path) -> StoreResult<Vec<(String, bool)>> {
    let conn = open_unmigrated(path)?;
    let (open, _) = task_state_sql(&conn)?;
    let mut statement = conn.prepare(&format!(
        "SELECT b.branch,({open}) FROM tasks t JOIN \
         (SELECT id AS task_id, branch FROM tasks UNION SELECT task_id, branch FROM task_prs) b \
         ON b.task_id=t.id"
    ))?;
    let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
    rows.map(|row| row.map_err(StoreError::from)).collect()
}

pub(crate) fn read_task_worktree_ownership(path: &Path) -> StoreResult<Vec<TaskWorktreeOwnership>> {
    let conn = open_unmigrated(path)?;
    let (open, done) = task_state_sql(&conn)?;
    // Old registries still protect unfinished work, but cannot grant deletion
    // until they can establish this machine's positive checkout ownership.
    let placed: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('tasks') WHERE name='checkout_machine_id')",
        [],
        |row| row.get(0),
    )?;
    let (local_flag, local_route): (bool, bool) = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('machines') WHERE name='is_local'), \
         EXISTS(SELECT 1 FROM pragma_table_info('machines') WHERE name='route')",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let local = if placed && local_flag {
        "COALESCE(t.checkout_machine_id=(SELECT id FROM machines WHERE is_local=1),0)"
    } else if placed && local_route {
        "COALESCE(t.checkout_machine_id=(SELECT id FROM machines WHERE route='local'),0)"
    } else {
        "0"
    };
    let mut statement = conn.prepare(&format!(
        "SELECT t.id,t.worktree,({done}) AND ({local}),({open}) FROM tasks t \
         WHERE t.worktree IS NOT NULL AND t.worktree != ''"
    ))?;
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
        // Authored preferences are applied from one configuration read per operation.
        login_email: None,
        routing_state: RoutingState::Automatic,
        plan: None,
        paid_through: None,
    })
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
        model: row.get(8)?,
        limited_until: row.get(9)?,
    }))
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
    #[cfg(test)]
    pub(crate) fn assert_no_historical_runs(&self) {
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

    /// Open the store for ordinary use. This never advances the shared release
    /// frontier: against `~/.lf/loopflow.db` it reads and validates but leaves
    /// the migration frontier where the installed `lf` left it. Advancing the
    /// shared frontier is the promotion boundary's job — see
    /// [`Self::open_as_promotion_boundary`].
    pub fn new(path: &Path) -> StoreResult<Self> {
        Self::open(
            path,
            super::FrontierAdvance::Forbidden,
            SQLITE_WRITE_BUSY_TIMEOUT,
        )
    }

    /// Revalidate a connection after a child executable may have upgraded it.
    pub(crate) fn validate_current_schema(&self) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        super::migrations::validate_sqlite_schema(&conn)
    }

    /// Open the shared store as `lf install promote` — the single authorized
    /// owner of the migration frontier. Applies pending migrations under the
    /// caller's exclusive promotion lock.
    pub(crate) fn open_as_promotion_boundary(path: &Path) -> StoreResult<Self> {
        Self::open(
            path,
            super::FrontierAdvance::Authorized,
            SQLITE_WRITE_BUSY_TIMEOUT,
        )
    }

    /// Open a hermetic, fully-migrated store at `path`: this build's whole
    /// migration catalog, reading **no**
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
        let mut conn = Connection::open(path)?;
        crate::performance::observe_sqlite(&conn);
        configure_write_connection(&mut conn, path, SQLITE_WRITE_BUSY_TIMEOUT)?;
        super::migrations::apply_sqlite(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    fn open(path: &Path, advance: super::FrontierAdvance, busy: Duration) -> StoreResult<Self> {
        Self::open_with(path, &super::machine_home_dir(), advance, busy)
    }

    /// Open resolving the migration decision against an explicit machine home.
    /// Production opens pass the real one through [`Self::open`]; the
    /// same-module shared-frontier regressions pass a temp home.
    fn open_with(
        path: &Path,
        home: &Path,
        advance: super::FrontierAdvance,
        busy: Duration,
    ) -> StoreResult<Self> {
        let existing_database = std::fs::metadata(path).is_ok_and(|metadata| metadata.len() > 0);
        // Resolve the frontier authority before touching the filesystem. An
        // ordinary open of a shared store it may not initialize refuses here,
        // before create_dir_all/Connection::open would leave an empty
        // ~/.lf/loopflow.db behind — a file whose mere existence a liveness or
        // bootstrap check could misread as "the shared store is initialized".
        let may_apply_migrations =
            super::may_apply_migrations(path, home, advance).map_err(|error| {
                StoreError::InvalidData(format!("resolve migration authority: {error}"))
            })?;
        let shared_database = super::same_database_file(path, &home.join(".lf/loopflow.db"))
            .map_err(|error| {
                StoreError::InvalidData(format!("resolve shared store identity: {error}"))
            })?;

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

        let mut conn = Connection::open(path)?;
        crate::performance::observe_sqlite(&conn);
        // Install the handler before journal-mode negotiation: that pragma can
        // itself meet another process opening the same WAL database.
        configure_write_connection(&mut conn, path, busy)?;

        if shared_database && !may_apply_migrations {
            // Validate the applied history first (preserving divergent/incompatible
            // and store-ahead errors), then refuse if this binary knows a migration
            // the store has not applied. An ordinary open must not hand back a store
            // whose schema is older than this binary's code, which may query the
            // columns that pending migration adds.
            super::migrations::validate_sqlite_schema(&conn)?;
            if let Some(pending) = super::migrations::pending_shared_migration(&conn)? {
                return Err(StoreError::InvalidData(unapplied_message(path, &pending)));
            }
        } else if existing_database {
            super::migrations::apply_sqlite_with_backup(&conn, path)?;
            if !shared_database {
                // A private home holds exactly this build's schema; one changed
                // outside lf is never repaired.
                super::migrations::validate_sqlite_schema(&conn).map_err(|error| {
                    StoreError::InvalidData(format!(
                        "{error}\nDatabase: {}\nA private LF_HOME is disposable: start a fresh one; \
                         lf will not repair it.",
                        path.display()
                    ))
                })?;
            }
        } else {
            super::migrations::apply_sqlite(&conn)?;
        }
        if may_apply_migrations {
            migrate_plaintext_provider_tokens(&mut conn)?;
        }

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Open only the Process rows without schema or token writes.
    /// Observability commands use this when a source build may be older than
    /// the machine's release-owned database.
    pub(crate) fn open_processes_read_only(path: &Path) -> StoreResult<Self> {
        let store = Self::open_read_only(path)?;
        {
            let conn = store.conn.lock().expect("store mutex poisoned");
            validate_process_schema(&conn)?;
        }
        Ok(store)
    }

    /// Append process evidence to an existing compatible store, never initialize it.
    pub(crate) fn open_existing_processes(path: &Path, wait: Duration) -> StoreResult<Self> {
        let conn = Connection::open_with_flags(
            path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        crate::performance::observe_sqlite(&conn);
        conn.busy_timeout(wait.min(Duration::from_secs(5)))?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        // An older ledger without the current process owner is not a writable
        // observation destination. Leave upgrade decisions to ordinary admission.
        conn.prepare(
            "SELECT id, trace_id, parent_lf_process_id, started_at, completed_at, outcome, exit_code, pid FROM processes LIMIT 0",
        )?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub(crate) fn open_read_only(path: &Path) -> StoreResult<Self> {
        let conn = Connection::open_with_flags(
            path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        crate::performance::observe_sqlite(&conn);
        conn.execute_batch("PRAGMA query_only = ON; PRAGMA busy_timeout = 5000;")?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub(crate) fn work_identities(&self) -> StoreResult<Vec<WorkIdentity>> {
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

    #[cfg(test)]
    pub(crate) fn apply_migration_for_test(&self, name: &str) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        if crate::store::migrations::migration_is_applied_for_test(&conn, name)? {
            return Ok(());
        }
        conn.execute_batch(&crate::store::migrations::migration_sql_for_test(name))?;
        Ok(())
    }

    pub fn deleted_task_issues(
        &self,
        wave_id: &WaveId,
    ) -> StoreResult<std::collections::HashSet<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        deleted_task_issues_in(&conn, wave_id)
    }

    fn upsert_wave(&self, wave: &Wave) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
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
        )?;
        if !saved {
            tx.execute("UPDATE waves SET created_at=?2,retired_at=?3,superseded_by_wave_id=?4,retirement_reason=?5 WHERE id=?1",params![wave.id(),created_at,wave.retired_at().map(|at|at.unix_timestamp()),wave.superseded_by_wave_id(),wave.retirement_reason()])?;
        }
        tx.execute(
            "UPDATE waves SET promoted_at=COALESCE(promoted_at,?2) WHERE id=?1",
            params![wave.id(), wave.promoted_at().map(|at| at.unix_timestamp())],
        )?;
        tx.commit()?;
        Ok(())
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

fn validate_process_schema(conn: &Connection) -> StoreResult<()> {
    conn.prepare(
        "SELECT id,trace_id,parent_lf_process_id,started_at,completed_at,outcome,pid FROM processes LIMIT 0",
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

    pub(crate) fn applied_schema_version(&self) -> StoreResult<Option<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        super::migrations::latest_applied_version_sqlite(&conn)
    }

    // -- Provider tokens -------------------------------------------------------

    pub(crate) fn provider_auth_snapshot(
        &self,
        provider: Provider,
    ) -> StoreResult<Option<crate::provider_auth::ProviderAuthSnapshot>> {
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
            Ok(crate::provider_auth::ProviderAuthSnapshot {
                provider,
                status: if expires_at.is_some_and(|expiry| expiry <= now_unix()) {
                    crate::provider_auth::AuthStatus::Expired
                } else {
                    crate::provider_auth::AuthStatus::Active { login }
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

    pub(crate) fn replace_provider_token(
        &self,
        expected: &super::ProviderToken,
        replacement: &super::ProviderToken,
        deadline: std::time::Instant,
    ) -> StoreResult<super::ProviderTokenReplacement> {
        let expired = || StoreError::InvalidData("credential replacement deadline elapsed".into());
        let access = token_crypto::encrypt_token(&replacement.access_token)
            .map_err(|_| StoreError::InvalidData("credential encryption failed".into()))?;
        let refresh = token_crypto::encrypt_optional(replacement.refresh_token.as_deref())
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
            ).optional()?.map(decrypt_token_row).transpose()?;
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
            tokens.push(decrypt_token_row(row?)?);
        }
        Ok(tokens)
    }

    // -- Provider accounts -----------------------------------------------------

    pub fn upsert_provider_account(&self, account: &ProviderAccount) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute("INSERT INTO provider_accounts(provider,account_id,home,credential_state,utilization_percent,cooldown_until,cooldown_reason,last_selected_at,created_at,updated_at,observed_email,observed_subject,observed_plan,observed_credential_digest)
            VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)
            ON CONFLICT(provider,account_id) DO UPDATE SET home=excluded.home,credential_state=excluded.credential_state,utilization_percent=excluded.utilization_percent,cooldown_until=excluded.cooldown_until,cooldown_reason=excluded.cooldown_reason,last_selected_at=excluded.last_selected_at,updated_at=excluded.updated_at,observed_email=excluded.observed_email,observed_subject=excluded.observed_subject,observed_plan=excluded.observed_plan,observed_credential_digest=excluded.observed_credential_digest",
            params![account.provider,account.account_id.as_str(),account.home.as_ref().map(|path| path.to_string_lossy().to_string()),account.credential_state.as_str(),account.utilization_percent,account.cooldown_until,account.cooldown_reason,account.last_selected_at,account.created_at,account.updated_at,account.observed_email,account.observed_subject,account.observed_plan,account.observed_credential_digest])?;
        crate::config::placement::write_account_preferences(&tx, account)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        tx.commit()?;
        Ok(())
    }

    pub fn record_provider_account_identity(
        &self,
        provider: &str,
        account_id: &ProviderAccountId,
        email: &str,
        subject: &str,
        plan: Option<&str>,
        credential_digest: Option<&str>,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE provider_accounts SET observed_email = ?3, observed_subject = ?4,
            observed_plan = ?5, observed_credential_digest = ?7, updated_at = ?6 WHERE provider = ?1 AND account_id = ?2",
            params![
                provider,
                account_id.as_str(),
                email,
                subject,
                plan,
                now_unix(),
                credential_digest
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
            &format!("SELECT {PROVIDER_ACCOUNT_COLUMNS} FROM provider_accounts WHERE provider = ?1 AND account_id = ?2"),
        )?;
        statement
            .query_row(params![provider, account_id.as_str()], |row| {
                Ok(read_provider_account(row))
            })
            .optional()?
            .transpose()?
            .map(|account| {
                crate::config::placement::global_settings(&conn)
                    .and_then(|config| {
                        crate::config::placement::account_preferences(&config, account)
                    })
                    .map_err(|error| StoreError::InvalidData(error.to_string()))
            })
            .transpose()
    }

    pub fn list_provider_accounts(
        &self,
        provider: Option<&str>,
    ) -> StoreResult<Vec<ProviderAccount>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(&format!(
            "SELECT {PROVIDER_ACCOUNT_COLUMNS} FROM provider_accounts
             WHERE (?1 IS NULL OR provider = ?1) ORDER BY provider, account_id"
        ))?;
        let accounts = statement.query_map([provider], |row| Ok(read_provider_account(row)))?;
        let config = crate::config::placement::global_settings(&conn)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        accounts
            .map(|account| {
                crate::config::placement::account_preferences(&config, account??)
                    .map_err(|error| StoreError::InvalidData(error.to_string()))
            })
            .collect()
    }

    pub fn update_provider_account_lifecycle(&self, account: &ProviderAccount) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM provider_accounts WHERE provider=?1 AND account_id=?2)",
            params![account.provider, account.account_id.as_str()],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(StoreError::NotFound);
        }
        crate::config::placement::write_account_preferences(&conn, account)
            .map_err(|error| StoreError::InvalidData(error.to_string()))
    }

    pub fn clear_provider_account_cooldown(
        &self,
        provider: &str,
        account_id: &ProviderAccountId,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let changed = conn.execute(
            "UPDATE provider_accounts SET cooldown_until = NULL, cooldown_reason = NULL,
             updated_at = ?3 WHERE provider = ?1 AND account_id = ?2",
            params![provider, account_id.as_str(), now_unix()],
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

    /// Verification changes credential evidence, never routing or cooldown policy.
    pub fn update_provider_account_credential_state(
        &self,
        provider: &str,
        account_id: &ProviderAccountId,
        state: CredentialState,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let changed = conn.execute(
            "UPDATE provider_accounts SET credential_state = ?3, updated_at = ?4
             WHERE provider = ?1 AND account_id = ?2",
            params![provider, account_id.as_str(), state.as_str(), now_unix()],
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
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
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction()?;
        let now = now_unix();
        for window in windows {
            transaction.execute(
                "INSERT INTO provider_account_limits
                     (provider, account_id, window, used_percent, resets_at, plan, observed_at, source, model, limited_until)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                 ON CONFLICT(provider, account_id, window) DO UPDATE SET
                     used_percent = excluded.used_percent,
                     resets_at = excluded.resets_at,
                     plan = excluded.plan,
                     observed_at = excluded.observed_at,
                     source = excluded.source,
                     model = excluded.model,
                     limited_until = excluded.limited_until",
                params![
                    provider,
                    account_id.as_str(),
                    window.window,
                    window.used_percent,
                    window.resets_at,
                    window.plan,
                    now,
                    source,
                    window.model,
                    window.limited_until,
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn provider_account_limits(
        &self,
        provider: Option<&str>,
    ) -> StoreResult<Vec<crate::store::AccountLimitRow>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT provider, account_id, window, used_percent, resets_at, plan, observed_at, source, model, limited_until
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

    pub fn set_auth_browser_profiles(
        &self,
        provider: Provider,
        account_id: Option<&ProviderAccountId>,
        profile_ids: &[ProfileId],
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut path = vec!["auth_browsers", provider.as_str()];
        if let Some(account) = account_id {
            path.extend(["accounts", account.as_str()]);
        } else {
            path.push("default");
        }
        crate::config::placement::write(
            &conn,
            None,
            &path,
            Some(
                serde_yaml_ng::to_value(profile_ids)
                    .map_err(|error| StoreError::InvalidData(error.to_string()))?,
            ),
        )
        .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        Ok(())
    }

    pub fn list_auth_browser_profiles(
        &self,
        provider: Option<Provider>,
        account_id: Option<&ProviderAccountId>,
    ) -> StoreResult<Vec<AuthBrowserBinding>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let declarations = crate::config::placement::global_settings(&conn)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?
            .auth_browsers;
        let mut out = Vec::new();
        for (name, declaration) in declarations {
            let selected = name
                .parse::<Provider>()
                .map_err(|error| StoreError::InvalidData(error.to_string()))?;
            if provider.is_some_and(|provider| provider != selected) {
                continue;
            }
            for (account, profiles) in std::iter::once((None, declaration.default)).chain(
                declaration
                    .accounts
                    .into_iter()
                    .map(|(account, profiles)| (Some(account), profiles)),
            ) {
                if account_id.is_some_and(|id| account.as_deref() != Some(id.as_str())) {
                    continue;
                }
                for (position, profile) in profiles.into_iter().enumerate() {
                    out.push(AuthBrowserBinding {
                        provider: selected,
                        account_id: account
                            .as_deref()
                            .map(ProviderAccountId::parse)
                            .transpose()
                            .map_err(StoreError::InvalidData)?,
                        position,
                        profile_id: ProfileId::parse(&profile).map_err(StoreError::InvalidData)?,
                    });
                }
            }
        }
        Ok(out)
    }

    pub fn set_provider_route(&self, route: &ProviderRoute) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let (path, prefix) = crate::config::placement::route_location(&conn, &route.scope)
            .map_err(|e| StoreError::InvalidData(e.to_string()))?;
        let parts: Vec<_> = prefix
            .iter()
            .map(String::as_str)
            .chain(["account", route.provider.as_str()])
            .collect();
        crate::config::files::update(&path, |document| {
            crate::config::settings::insert(
                document,
                &parts,
                serde_yaml_ng::to_value(&route.accounts)?,
            );
            Ok(())
        })
        .map_err(|e| StoreError::InvalidData(e.to_string()))
    }

    pub fn clear_provider_route(
        &self,
        scope: &RouteScope,
        provider: Provider,
    ) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let (path, prefix) = crate::config::placement::route_location(&conn, scope)
            .map_err(|e| StoreError::InvalidData(e.to_string()))?;
        let parts: Vec<_> = prefix
            .iter()
            .map(String::as_str)
            .chain(["account", provider.as_str()])
            .collect();
        crate::config::files::update(&path, |document| {
            Ok(crate::config::files::remove(document, &parts))
        })
        .map_err(|e| StoreError::InvalidData(e.to_string()))
    }

    pub(crate) fn account_declarations(
        &self,
        scopes: &[RouteScope],
    ) -> StoreResult<Vec<crate::config::ConfigDeclarations>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        scopes
            .iter()
            .map(|scope| {
                let (path, prefix) = crate::config::placement::route_location(&conn, scope)
                    .map_err(|error| StoreError::InvalidData(error.to_string()))?;
                let mut document = crate::config::files::read(&path)
                    .map_err(|error| StoreError::InvalidData(error.to_string()))?;
                crate::config::remote::apply_document(&path, &mut document)
                    .map_err(|error| StoreError::InvalidData(error.to_string()))?;
                let accounts = prefix
                    .iter()
                    .try_fold(&document, |value, key| value.get(key))
                    .and_then(|value| value.get("account"));
                let mut value = serde_yaml_ng::Value::Mapping(Default::default());
                if let Some(accounts) = accounts {
                    value["account"] = accounts.clone();
                }
                Ok(crate::config::ConfigDeclarations {
                    source: crate::config::ConfigSource::Personal,
                    scope: match scope {
                        RouteScope::Wave(_) => crate::config::settings::SettingScope::Wave,
                        RouteScope::Repo(_) => crate::config::settings::SettingScope::Repository,
                        RouteScope::Default => crate::config::settings::SettingScope::Global,
                    },
                    path,
                    value,
                })
            })
            .collect()
    }

    pub fn provider_route(
        &self,
        scope: &RouteScope,
        provider: Provider,
    ) -> StoreResult<Option<ProviderRoute>> {
        let observed = self.list_provider_accounts(Some(provider.as_str()))?;
        let conn = self.conn.lock().expect("store mutex poisoned");
        let (path, prefix) = crate::config::placement::route_location(&conn, scope)
            .map_err(|e| StoreError::InvalidData(e.to_string()))?;
        let mut document = crate::config::files::read(&path)
            .map_err(|e| StoreError::InvalidData(e.to_string()))?;
        crate::config::remote::apply_document(&path, &mut document)
            .map_err(|e| StoreError::InvalidData(e.to_string()))?;
        let parts: Vec<_> = prefix
            .iter()
            .map(String::as_str)
            .chain(["account", provider.as_str()])
            .collect();
        let Some(value) = parts
            .iter()
            .try_fold(&document, |value, key| value.get(*key))
        else {
            return Ok(None);
        };
        let order: crate::config::placement::AccountOrder =
            serde_yaml_ng::from_value(value.clone())
                .map_err(|e| StoreError::InvalidData(e.to_string()))?;
        let candidates: Vec<_> = observed.iter().collect();
        let accounts = order
            .iter()
            .map(
                |selector| match crate::provider_account::match_account(&candidates, selector) {
                    crate::provider_account::AccountMatch::One(account) => {
                        Ok(account.account_id.clone())
                    }
                    crate::provider_account::AccountMatch::None => Err(StoreError::InvalidData(
                        format!("account reference {selector} has no resident login"),
                    )),
                    crate::provider_account::AccountMatch::Ambiguous(_) => {
                        Err(StoreError::InvalidData(format!(
                            "account reference {selector} is ambiguous"
                        )))
                    }
                },
            )
            .collect::<StoreResult<Vec<_>>>()?;
        Ok(Some(ProviderRoute {
            scope: scope.clone(),
            provider,
            accounts,
            created_at: 0,
            updated_at: 0,
        }))
    }

    pub fn pin_provider_session_route(
        &self,
        provider: Provider,
        agent_session: &AgentSessionId,
        account_id: &ProviderAccountId,
    ) -> StoreResult<()> {
        // Every launch runs in the shared home, so the conversation lives
        // there from now on, wherever it ran before.
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO provider_session_accounts (
                provider, provider_session_id, account_id, created_at, isolated
             ) VALUES (?1, ?2, ?3, ?4, 0)
             ON CONFLICT(provider, provider_session_id) DO UPDATE SET
                account_id = excluded.account_id,
                created_at = excluded.created_at,
                isolated = 0",
            params![
                provider.as_str(),
                agent_session,
                account_id.as_str(),
                now_unix(),
            ],
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

    pub fn record_provider_account_switch(
        &self,
        provider: Provider,
        account_id: &ProviderAccountId,
        cause: &str,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO provider_account_switches (
                provider, account_id, switched_at, cause
             ) VALUES (?1, ?2, ?3, ?4)",
            params![provider.as_str(), account_id.as_str(), now_unix(), cause],
        )?;
        Ok(())
    }

    /// The account the provider's native home was last switched to at or
    /// after `since`; `None` when it has not changed account since then.
    pub fn provider_account_switched_since(
        &self,
        provider: Provider,
        since: i64,
    ) -> StoreResult<Option<ProviderAccountId>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            "SELECT account_id FROM provider_account_switches
             WHERE provider = ?1 AND switched_at >= ?2
             ORDER BY switched_at DESC, id DESC LIMIT 1",
            params![provider.as_str(), since],
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
    #[cfg(test)]
    pub(crate) fn record_account_home_conversation(
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

    /// The most urgent eligible candidate for `model` (`None`: a
    /// launch whose model the provider picks, held back by every scoped limit).
    pub fn select_provider_account(
        &self,
        provider: Provider,
        candidates: &[ProviderAccountId],
        model: Option<&str>,
    ) -> StoreResult<Option<ProviderAccountSelection>> {
        if candidates.is_empty() {
            return Ok(None);
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let now = now_unix();
        let newest_selection = transaction.query_row(
            "SELECT COALESCE(MAX(last_selected_at), 0)
             FROM provider_accounts WHERE provider = ?1",
            [provider.as_str()],
            |row| row.get::<_, i64>(0),
        )?;
        let selection_time = now.max(newest_selection + 1);
        let limits = {
            let mut statement = transaction.prepare(
                "SELECT provider, account_id, window, used_percent, resets_at, plan, observed_at, source, model, limited_until
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
            &format!("SELECT {PROVIDER_ACCOUNT_COLUMNS} FROM provider_accounts WHERE provider = ?1 AND account_id = ?2"),
        )?;
        let config = crate::config::placement::global_settings(&transaction)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        let mut available = Vec::new();
        for account_id in candidates {
            let account = account_statement
                .query_row(params![provider.as_str(), account_id.as_str()], |row| {
                    Ok(read_provider_account(row))
                })
                .optional()?
                .transpose()?;
            let account = account
                .map(|account| {
                    crate::config::placement::account_preferences(&config, account)
                        .map_err(|error| StoreError::InvalidData(error.to_string()))
                })
                .transpose()?;
            if let Some(account) = account.filter(|account| {
                crate::provider_account::account_route_eligible(account, &limits, model, now)
            }) {
                available.push(account);
            }
        }
        drop(account_statement);

        crate::provider_account::order_accounts_by_urgency(&mut available, &limits, model, now);
        let Some(mut account) = available.into_iter().next() else {
            transaction.commit()?;
            return Ok(None);
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
        Ok(Some(ProviderAccountSelection { account }))
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

    /// Record a Process start or exit. An exit forgets this Process and the
    /// finished children it was keeping when nothing else needs them
    /// ([`processes::FORGETTABLE`]). Each statement commits on its own so a
    /// writer holds the lock only briefly; a row an interrupted exit leaves is
    /// forgotten by the retention prune.
    pub fn record_process(&self, process: &crate::process::LfProcess) -> StoreResult<()> {
        self.record_hosted_process(process, None)
    }

    /// Record a Process start or exit; a start inside a held terminal names it.
    fn record_hosted_process(
        &self,
        process: &crate::process::LfProcess,
        held_terminal: Option<&crate::id::TerminalId>,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        record_process_in(&conn, process, held_terminal)?;
        if process.completed_at.is_some() {
            let forget = |scope: &str| {
                conn.execute(
                    &format!(
                        "DELETE FROM processes WHERE id IN (SELECT p.id FROM processes p
                         WHERE {scope} AND {})",
                        processes::FORGETTABLE
                    ),
                    [&process.id],
                )
            };
            // Children first: any child left would keep this Process.
            forget("p.parent_lf_process_id=?1")?;
            forget("p.id=?1")?;
        }
        Ok(())
    }

    /// Record a process, waiting at most `wait` for another writer; a zero wait
    /// is one attempt.
    pub(crate) fn record_process_within(
        &self,
        process: &crate::process::LfProcess,
        held_terminal: Option<&crate::id::TerminalId>,
        wait: Duration,
    ) -> StoreResult<()> {
        let previous: u32 = {
            let conn = self.conn.lock().expect("store mutex poisoned");
            let previous = conn.pragma_query_value(None, "busy_timeout", |row| row.get(0))?;
            conn.busy_timeout(wait)?;
            previous
        };
        let result = self.record_hosted_process(process, held_terminal);
        self.conn
            .lock()
            .expect("store mutex poisoned")
            .busy_timeout(Duration::from_millis(u64::from(previous)))?;
        result
    }

    pub fn process_is_recorded(&self, lf_process_id: &str) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare("SELECT 1 FROM processes WHERE id=?1")?;
        Ok(query.exists([lf_process_id])?)
    }
}

/// A start record never overwrites an existing row; a terminal record completes
/// the row it started. `held_terminal` is the terminal a held lf process runs
/// in, so rules scoped to that terminal find the work started inside it.
pub(super) fn record_process_in(
    conn: &rusqlite::Connection,
    process: &crate::process::LfProcess,
    held_terminal: Option<&crate::id::TerminalId>,
) -> StoreResult<()> {
    conn.execute(
        "INSERT INTO processes(id,trace_id,parent_lf_process_id,command,repo,cwd,started_at,
            via_agent,caller_session_id,caller_agent_process_id,completed_at,outcome,exit_code,signal,error,pid,os_started_at,
            held_terminal_id)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18)
         ON CONFLICT(id) DO UPDATE SET completed_at=excluded.completed_at,
            outcome=excluded.outcome,exit_code=excluded.exit_code,signal=excluded.signal,error=excluded.error
         WHERE processes.completed_at IS NULL AND excluded.completed_at IS NOT NULL",
        params![process.id,process.trace_id,process.parent_lf_process_id,process.command,process.repo,process.cwd,
            process.started_at,process.via_agent,process.caller_session_id,process.caller_agent_process_id,
            process.completed_at,process.outcome,process.exit_code,process.signal,process.error,process.pid,
            process.os_started_at,held_terminal],
    )?;
    Ok(())
}

#[cfg(test)]
mod frontier_tests {
    use super::SqliteStore;
    use crate::durable::{WorkRef, WorkStatus};
    use crate::id::WaveId;
    use crate::store::migrations::{
        apply_all_but_head, apply_newer_migration, latest_applied_version_sqlite,
        latest_known_version, prior_known_version,
    };
    use crate::store::FrontierAdvance::{self, Authorized, Forbidden};
    use crate::work::wave::Wave;
    use std::path::{Path, PathBuf};

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
        let reference =
            SqliteStore::open_ephemeral(&directory.path().join("reference.db")).unwrap();
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
        let store = SqliteStore {
            conn: std::sync::Arc::new(std::sync::Mutex::new(conn)),
        };
        let identities = store.work_identities().unwrap();
        assert_eq!(identities.len(), 4);
        assert_eq!(identities[3].subject, "abcdef");
        assert!(identities[3].parent.is_none());
        assert_eq!(identities[2].subject, "LOO-293");
        assert_eq!(identities[2].external_id.as_deref(), Some("linear-issue"));
        assert_eq!(identities[2].parent.as_ref(), Some(&identities[1].work));
        assert_eq!(identities[1].parent.as_ref(), Some(&identities[0].work));
    }

    /// The machine home whose `.lf/loopflow.db` `may_apply_migrations` treats as
    /// the shared release store. The regressions inject it so they never touch a
    /// developer's real `~/.lf`.
    struct SharedMachine {
        _dir: tempfile::TempDir,
        home: PathBuf,
    }

    impl SharedMachine {
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
        home: &Path,
        advance: FrontierAdvance,
    ) -> crate::store::StoreResult<SqliteStore> {
        SqliteStore::open_with(path, home, advance, super::SQLITE_WRITE_BUSY_TIMEOUT)
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

    fn seed_current_wave(path: &Path) {
        rusqlite::Connection::open(path).unwrap().execute(
            "INSERT INTO waves(id,name,repo,created_at) VALUES('current-wave','Current','/repo',100)", [],
        ).unwrap();
    }

    fn current_wave(path: &Path) -> String {
        rusqlite::Connection::open(path)
            .unwrap()
            .query_row(
                "SELECT name FROM waves WHERE id='current-wave'",
                [],
                |row| row.get(0),
            )
            .unwrap()
    }

    #[test]
    fn any_build_uses_compatible_shared_work_without_migrating() {
        let shared = SharedMachine::new();
        let path = shared.shared_db();
        open(&path, &shared.home, Authorized).unwrap();
        seed_current_wave(&path);
        let before = frontier(&path);
        let store = open(&path, &shared.home, Forbidden).unwrap();
        let wave = Wave::new(WaveId::new(), "From app".into(), "/repo".into());
        store.create_wave(&wave).unwrap();
        assert_eq!(current_wave(&path), "Current");
        assert_eq!(frontier(&path), before);
        open(&path, &shared.home, Forbidden).unwrap();
    }

    /// A newer build leaves this lf reading and writing the shared store.
    /// Promotion selecting this build applies nothing to history it does not know.
    #[test]
    fn an_older_lf_keeps_working_on_a_store_a_newer_build_extended() {
        let shared = SharedMachine::new();
        let path = shared.shared_db();
        open(&path, &shared.home, Authorized).unwrap();
        seed_current_wave(&path);
        let conn = rusqlite::Connection::open(&path).unwrap();
        apply_newer_migration(
            &conn,
            "ALTER TABLE waves ADD COLUMN colour TEXT;
             CREATE TABLE wave_badges (wave_id TEXT NOT NULL, badge TEXT);",
        )
        .unwrap();
        let newer = frontier(&path);

        let store =
            open(&path, &shared.home, Forbidden).expect("a newer build leaves the store readable");
        let wave = Wave::new(WaveId::new(), "From older lf".into(), "/repo".into());
        store.create_wave(&wave).unwrap();
        assert_eq!(
            store
                .get_wave(wave.id())
                .unwrap()
                .map(|wave| wave.name().to_string()),
            Some("From older lf".to_string())
        );
        assert_eq!(current_wave(&path), "Current");
        assert_eq!(frontier(&path), newer);
        open(&path, &shared.home, Authorized).expect("selecting an older build applies nothing");
        assert_eq!(frontier(&path), newer);
    }

    /// A contract migration past its staging window still opens: the one
    /// statement that reads what it dropped fails, naming the column.
    #[test]
    fn a_dropped_column_fails_the_statement_that_reads_it_and_nothing_else() {
        let shared = SharedMachine::new();
        let path = shared.shared_db();
        open(&path, &shared.home, Authorized).unwrap();
        let running = open(&path, &shared.home, Forbidden).unwrap();
        let conn = rusqlite::Connection::open(&path).unwrap();
        apply_newer_migration(&conn, "ALTER TABLE waves DROP COLUMN promoted_at;").unwrap();

        running
            .validate_current_schema()
            .expect("the running reader keeps its store");
        let store = open(&path, &shared.home, Forbidden).expect("a fresh open keeps working");

        let error = store.get_wave(&WaveId::new()).unwrap_err().to_string();

        assert!(error.contains("promoted_at"), "{error}");
        assert!(!error.contains("lf install"), "{error}");
        store.health_check().expect("the rest of the store answers");
    }

    /// A Graph holds its store across steps and revalidates it after each one
    /// (`validate_current_schema`). A newer build that adds a slug migration
    /// meanwhile leaves that running reader working.
    #[test]
    fn a_running_older_reader_survives_an_upgrade_under_it() {
        let shared = SharedMachine::new();
        let path = shared.shared_db();
        open(&path, &shared.home, Authorized).unwrap();
        let running = open(&path, &shared.home, Forbidden).unwrap();
        let before = Wave::new(WaveId::new(), "Before upgrade".into(), "/repo".into());
        running.create_wave(&before).unwrap();

        let newer = rusqlite::Connection::open(&path).unwrap();
        apply_newer_migration(
            &newer,
            "ALTER TABLE waves ADD COLUMN colour TEXT;
             CREATE TABLE wave_badges (wave_id TEXT NOT NULL, badge TEXT);",
        )
        .unwrap();

        running
            .validate_current_schema()
            .expect("a newer migration keeps the running reader valid");
        let after = Wave::new(WaveId::new(), "After upgrade".into(), "/repo".into());
        running.create_wave(&after).unwrap();
        for wave in [&before, &after] {
            assert_eq!(
                running
                    .get_wave(wave.id())
                    .unwrap()
                    .map(|w| w.name().to_string()),
                Some(wave.name().to_string())
            );
        }
    }

    #[test]
    fn a_private_store_opens_with_the_current_work_schema() {
        let shared = SharedMachine::new();
        let path = shared.home.join("private/loopflow.db");
        let store = open(&path, &shared.home, Forbidden).expect("a private store migrates itself");
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
        let shared = SharedMachine::new();
        let path = shared.shared_db();

        let error = open(&path, &shared.home, Forbidden)
            .expect_err("an ordinary open must not initialize the shared store");
        assert!(
            error.to_string().contains("lf self install"),
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
        let shared = SharedMachine::new();
        let path = shared.shared_db();
        seed_shared_store_at_prior_head(&path);
        let installed_frontier = frontier(&path).unwrap();
        assert_eq!(installed_frontier, prior_known_version());
        assert_ne!(installed_frontier, latest_known_version());

        // The candidate is one migration ahead; its ordinary open refuses rather
        // than reuse a schema older than its own code.
        let error = open(&path, &shared.home, Forbidden)
            .expect_err("an ordinary open ahead of the frontier must refuse");
        // `lf install` finishes an app Sparkle replaced; `--dev` would move a
        // stable Machine to dev.
        let error = error.to_string();
        assert!(
            error.contains(&latest_known_version())
                && (error.contains("run `lf install` to finish")
                    || error.contains("private LF_HOME"))
                && !error.contains("--dev"),
            "the refusal names the pending migration and its fix: {error}"
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
        let shared = SharedMachine::new();
        let path = shared.shared_db();

        // Initialization from absent.
        open(&path, &shared.home, Authorized).expect("boundary initializes");
        assert_eq!(
            frontier(&path).as_deref(),
            Some(latest_known_version().as_str())
        );

        // Once the boundary has initialized it, an ordinary open validates the
        // now-existing store read-only and leaves the frontier at the head.
        open(&path, &shared.home, Forbidden)
            .expect("an ordinary open validates the initialized store");
        assert_eq!(
            frontier(&path).as_deref(),
            Some(latest_known_version().as_str())
        );

        // Advancement from a prior-head store.
        let advanced = SharedMachine::new();
        let advanced_path = advanced.shared_db();
        seed_shared_store_at_prior_head(&advanced_path);
        assert_eq!(
            frontier(&advanced_path).as_deref(),
            Some(prior_known_version().as_str())
        );
        open(&advanced_path, &advanced.home, Authorized).expect("boundary advances");
        assert_eq!(
            frontier(&advanced_path).as_deref(),
            Some(latest_known_version().as_str())
        );
    }

    /// The 2026-07-17 incident shape, exercised as two binary generations: a
    /// branch candidate knows one migration the installed release does not.
    /// Ordinary candidate use must leave both the shared frontier and existing
    /// Wave untouched; explicit promotion advances once and retains the Wave.
    #[test]
    fn branch_candidate_cannot_advance_shared_store_or_damage_current_state_outside_promotion() {
        let shared = SharedMachine::new();
        let path = shared.shared_db();
        seed_shared_store_at_prior_head(&path);
        seed_current_wave(&path);
        let installed_frontier = prior_known_version();
        assert_eq!(
            frontier(&path).as_deref(),
            Some(installed_frontier.as_str())
        );
        assert_eq!(current_wave(&path), "Current");

        open(&path, &shared.home, Forbidden)
            .expect_err("ordinary branch candidate must not apply its migration");
        assert_eq!(
            frontier(&path).as_deref(),
            Some(installed_frontier.as_str())
        );
        assert_eq!(current_wave(&path), "Current");
        let installed = rusqlite::Connection::open(&path).unwrap();
        assert!(
            crate::store::migrations::old_reader_recognizes(&installed),
            "the installed release must still recognize the candidate's untouched store"
        );
        drop(installed);

        open(&path, &shared.home, Authorized)
            .expect("explicit promotion advances the shared frontier");
        let promoted_frontier = latest_known_version();
        assert_eq!(frontier(&path).as_deref(), Some(promoted_frontier.as_str()));
        assert_eq!(current_wave(&path), "Current");

        open(&path, &shared.home, Authorized)
            .expect("repeating promotion at the same frontier is a no-op");
        assert_eq!(frontier(&path).as_deref(), Some(promoted_frontier.as_str()));
        assert_eq!(current_wave(&path), "Current");
        open(&path, &shared.home, Forbidden)
            .expect("ordinary current binary opens after promotion");
    }

    /// Opening the shared store reads its schema, never its rows: a dangling
    /// reference is installation preflight's and `lf doctor`'s to report.
    #[test]
    fn an_ordinary_open_of_the_shared_store_does_not_scan_stored_rows() {
        let shared = SharedMachine::new();
        let path = shared.shared_db();
        open(&path, &shared.home, Authorized).expect("boundary initializes");
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            "PRAGMA foreign_keys = OFF;
             INSERT INTO projects(id, wave_id, external_project_id, created_at)
             VALUES ('orphan', 'absent-wave', 'external', 100);",
        )
        .unwrap();

        open(&path, &shared.home, Forbidden)
            .expect("an ordinary open validates ledger and schema only");
        crate::store::migrations::validate_sqlite(&conn)
            .expect_err("full diagnosis still reports the dangling reference");
    }

    /// A private path (not ~/.lf/loopflow.db) initializes and migrates itself.
    #[test]
    fn a_private_store_initializes_without_the_promotion_boundary() {
        let shared = SharedMachine::new();
        let private = shared.home.join(".lf-dev/branch/loopflow.db");
        open(&private, &shared.home, Forbidden).expect("private DB initializes");
        assert_eq!(
            frontier(&private).as_deref(),
            Some(latest_known_version().as_str())
        );
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

#[cfg(test)]
mod linear_oauth_tests {
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use super::{SqliteStore, SQLITE_WRITE_BUSY_TIMEOUT};
    use crate::store::{CredentialType, ProviderToken, ProviderTokenReplacement, Store};

    #[tokio::test]
    async fn linear_oauth_cancelled_commit_retains_lock_until_blocking_work_settles() {
        let directory = tempfile::tempdir().unwrap();
        let sqlite = SqliteStore::open_ephemeral(&directory.path().join("registry.db")).unwrap();
        let store = Arc::new(Store::from_sqlite_for_test(sqlite.clone()));
        let original = ProviderToken {
            provider: "linear".into(),
            access_token: "A1".into(),
            refresh_token: Some("R1".into()),
            oauth_client_id: Some("client".into()),
            expires_at: Some(1),
            login: None,
            updated_at: 1,
            credential_type: CredentialType::OAuth,
        };
        let replacement = ProviderToken {
            access_token: "A2".into(),
            refresh_token: Some("R2".into()),
            expires_at: Some(time::OffsetDateTime::now_utc().unix_timestamp() + 86400),
            ..original.clone()
        };
        store.upsert_provider_token(&original).await.unwrap();
        let lock_path = directory.path().join("refresh.lock");
        let lock = std::fs::File::create(&lock_path).unwrap();
        fs2::FileExt::try_lock_exclusive(&lock).unwrap();
        let observer = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&lock_path)
            .unwrap();
        // Hold the real connection mutex on a separate thread, across cancellation.
        let (held_tx, held_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let held_sqlite = sqlite.clone();
        let holder = std::thread::spawn(move || {
            let _connection = held_sqlite.conn.lock().unwrap();
            held_tx.send(()).unwrap();
            release_rx.recv().unwrap();
        });
        held_rx.await.unwrap();
        let deadline = Instant::now() + Duration::from_millis(50);
        let write = store.replace_provider_token(&original, &replacement, lock, deadline);
        assert!(tokio::time::timeout(Duration::from_millis(75), write)
            .await
            .is_err());
        assert!(fs2::FileExt::try_lock_exclusive(&observer).is_err());
        release_tx.send(()).unwrap();
        holder.join().unwrap();
        tokio::time::timeout(Duration::from_secs(2), async {
            while fs2::FileExt::try_lock_exclusive(&observer).is_err() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        // The expired queued closure declined to write, and restored normal SQLite policy.
        assert!(store.get_provider_token("linear").await.unwrap().as_ref() == Some(&original));
        let busy: u32 = sqlite
            .conn
            .lock()
            .unwrap()
            .pragma_query_value(None, "busy_timeout", |r| r.get(0))
            .unwrap();
        assert_eq!(u128::from(busy), SQLITE_WRITE_BUSY_TIMEOUT.as_millis());
        let outcome = store
            .replace_provider_token(
                &original,
                &replacement,
                observer,
                Instant::now() + Duration::from_secs(2),
            )
            .await
            .unwrap();
        assert!(matches!(outcome, ProviderTokenReplacement::Replaced));
        assert!(store.get_provider_token("linear").await.unwrap().as_ref() == Some(&replacement));
    }
}

#[cfg(test)]
mod account_observation_tests {
    use super::SqliteStore;
    use crate::store::{
        AccountLimitWindow, CredentialState, ProviderAccount, ProviderAccountId, RoutingState,
    };

    fn account() -> ProviderAccount {
        ProviderAccount {
            provider: "claude".into(),
            account_id: ProviderAccountId::parse("primary").unwrap(),
            home: None,
            login_email: None,
            observed_email: None,
            observed_subject: None,
            observed_credential_digest: None,
            observed_plan: None,
            credential_state: CredentialState::Connected,
            routing_state: RoutingState::Disabled,
            plan: Some("configured".into()),
            paid_through: None,
            utilization_percent: Some(98),
            cooldown_until: Some(1900000000),
            cooldown_reason: Some("weekly".into()),
            last_selected_at: Some(7),
            created_at: 1,
            updated_at: 1,
        }
    }

    #[test]
    fn account_file_edits_apply_to_single_and_filtered_reads() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let directory = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&directory.path().join("registry.db")).unwrap();
        let mut expected = account();
        expected.login_email =
            Some(crate::profile::EmailAddress::parse("work@example.com").unwrap());
        expected.paid_through =
            Some(time::Date::from_calendar_date(2026, time::Month::December, 1).unwrap());
        expected.observed_email = Some("observed@example.com".into());
        expected.observed_subject = Some("subject".into());
        expected.observed_plan = Some("observed plan".into());
        expected.observed_credential_digest = Some("digest".into());
        store.upsert_provider_account(&expected).unwrap();

        let assert_reads = |expected: &ProviderAccount| {
            assert_eq!(
                store
                    .get_provider_account("claude", &expected.account_id)
                    .unwrap()
                    .as_ref(),
                Some(expected)
            );
            for provider in [None, Some("claude")] {
                assert_eq!(
                    store.list_provider_accounts(provider).unwrap(),
                    std::slice::from_ref(expected)
                );
            }
            assert!(store
                .list_provider_accounts(Some("codex"))
                .unwrap()
                .is_empty());
        };
        assert_reads(&expected);
        let path = directory.path().join("config.yaml");
        let mut declarations = crate::config::files::read(&path).unwrap();
        declarations["accounts"]["claude"]["primary"]["plan"] = "edited".into();
        std::fs::write(&path, serde_yaml_ng::to_string(&declarations).unwrap()).unwrap();
        expected.plan = Some("edited".into());
        assert_reads(&expected);

        std::fs::write(path, "{}\n").unwrap();
        expected.login_email = None;
        expected.routing_state = RoutingState::Automatic;
        expected.plan = None;
        expected.paid_through = None;
        assert_reads(&expected);
    }

    #[test]
    fn account_observations_preserve_policy_and_omitted_windows() {
        let directory = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&directory.path().join("registry.db")).unwrap();
        let expected = account();
        store.upsert_provider_account(&expected).unwrap();
        let weekly = AccountLimitWindow {
            window: "weekly".into(),
            used_percent: 98,
            resets_at: Some(1900000000),
            plan: Some("max".into()),
            model: None,
            limited_until: None,
        };
        store
            .upsert_provider_account_limits("claude", &expected.account_id, &[weekly], "stream")
            .unwrap();
        let previous = store.provider_account_limits(None).unwrap().remove(0);
        for state in [CredentialState::Missing, CredentialState::Connected] {
            store
                .update_provider_account_credential_state("claude", &expected.account_id, state)
                .unwrap();
            let mut actual = store
                .get_provider_account("claude", &expected.account_id)
                .unwrap()
                .unwrap();
            assert_eq!(actual.credential_state, state);
            actual.credential_state = expected.credential_state;
            actual.updated_at = expected.updated_at;
            assert_eq!(actual, expected);
        }
        let session = AccountLimitWindow {
            window: "session".into(),
            used_percent: 2,
            resets_at: None,
            plan: None,
            model: None,
            limited_until: None,
        };
        store
            .upsert_provider_account_limits("claude", &expected.account_id, &[session], "poll")
            .unwrap();
        let rows = store.provider_account_limits(None).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows.iter().find(|row| row.window == "weekly"),
            Some(&previous)
        );
    }

    #[test]
    fn account_observation_batch_rolls_back_on_later_window_failure() {
        let directory = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&directory.path().join("registry.db")).unwrap();
        let account = account();
        store.upsert_provider_account(&account).unwrap();
        let session = AccountLimitWindow {
            window: "session".into(),
            used_percent: 22,
            resets_at: None,
            plan: None,
            model: None,
            limited_until: None,
        };
        store
            .upsert_provider_account_limits(
                "claude",
                &account.account_id,
                std::slice::from_ref(&session),
                "stream",
            )
            .unwrap();
        let previous = store.provider_account_limits(None).unwrap();
        store.conn.lock().unwrap().execute_batch("CREATE TRIGGER reject_weekly BEFORE INSERT ON provider_account_limits WHEN NEW.window = 'weekly' BEGIN SELECT RAISE(ABORT, 'simulated disk failure'); END;").unwrap();
        let windows = [
            AccountLimitWindow {
                used_percent: 40,
                ..session.clone()
            },
            AccountLimitWindow {
                window: "weekly".into(),
                ..session
            },
        ];
        assert!(store
            .upsert_provider_account_limits("claude", &account.account_id, &windows, "poll")
            .is_err());
        assert_eq!(store.provider_account_limits(None).unwrap(), previous);
    }
}
