//! Process-wide SQLite connection pool keyed by database identity `(canonical
//! path, dev, ino)` and access mode (`Write` vs `ReadOnly`).
//!
//! Pooled entries carry `(Connection, Arc<HistoryFile>, Option<ValidatedStamp>)`
//! and never `token_key`, which callers supply per hand-out. `PoolLease::drop`
//! returns a connection only when no extra `Arc<Mutex<Connection>>` clone
//! escaped, the connection is in autocommit mode, and the underlying file still
//! has the same `(dev, ino)`.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};

use rusqlite::Connection;

use crate::history::HistoryFile;
use crate::StoreResult;

/// Prepared statement LRU cache size per pooled connection (rusqlite default is 16).
pub(crate) const STATEMENT_CACHE_CAPACITY: usize = 256;

const MAX_IDLE_WRITE_PER_KEY: usize = 4;
const MAX_IDLE_READ_ONLY_PER_KEY: usize = 8;
const MAX_TOTAL_IDLE: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PoolMode {
    Write,
    ReadOnly,
}

impl PoolMode {
    fn max_idle(self) -> usize {
        match self {
            Self::Write => MAX_IDLE_WRITE_PER_KEY,
            Self::ReadOnly => MAX_IDLE_READ_ONLY_PER_KEY,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PoolKey {
    pub(crate) path: PathBuf,
    pub(crate) dev: u64,
    pub(crate) ino: u64,
    pub(crate) mode: PoolMode,
}

impl PoolKey {
    pub(crate) fn for_path(path: &Path, mode: PoolMode) -> Option<Self> {
        let canonical = std::fs::canonicalize(path).ok()?;
        let metadata = std::fs::metadata(&canonical).ok()?;
        #[cfg(unix)]
        let (dev, ino) = {
            use std::os::unix::fs::MetadataExt;
            (metadata.dev(), metadata.ino())
        };
        #[cfg(not(unix))]
        let (dev, ino) = (0, 0);
        Some(Self {
            path: canonical,
            dev,
            ino,
            mode,
        })
    }
}

/// What a pooled connection last validated against this binary's schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidatedStamp {
    pub(crate) data_version: i64,
    pub(crate) schema_version: i64,
    pub(crate) migration_count: i64,
    pub(crate) migration_max_rowid: i64,
    /// Shared-store pending-migration check succeeded for this frontier.
    pub(crate) shared_checked: bool,
    /// Plaintext provider tokens were already checked/migrated on this DB.
    pub(crate) tokens_migrated: bool,
}

pub(crate) struct PooledConn {
    pub(crate) conn: Connection,
    pub(crate) history: Arc<HistoryFile>,
    pub(crate) validated: Option<ValidatedStamp>,
}

/// Returns a borrowed connection to the process pool when the last handle to a
/// [`crate::sqlite::SqliteStore`] drops.
#[derive(Debug)]
pub(crate) struct PoolLease {
    key: PoolKey,
    conn: Option<Arc<Mutex<Connection>>>,
    history: Option<Arc<HistoryFile>>,
    validated: Mutex<Option<ValidatedStamp>>,
}

impl PoolLease {
    pub(crate) fn new(
        key: PoolKey,
        conn: Arc<Mutex<Connection>>,
        history: Arc<HistoryFile>,
        validated: Option<ValidatedStamp>,
    ) -> Self {
        Self {
            key,
            conn: Some(conn),
            history: Some(history),
            validated: Mutex::new(validated),
        }
    }

    pub(crate) fn set_validated(&self, stamp: Option<ValidatedStamp>) {
        if let Ok(mut guard) = self.validated.lock() {
            *guard = stamp;
        }
    }
}

impl Drop for PoolLease {
    fn drop(&mut self) {
        let Some(arc_conn) = self.conn.take() else {
            return;
        };
        let Some(history) = self.history.take() else {
            return;
        };
        // Only return when no other clone (such as test-support `pub conn`) escaped.
        let Ok(mutex) = Arc::try_unwrap(arc_conn) else {
            return;
        };
        let Ok(conn) = mutex.into_inner() else {
            return;
        };
        if !conn.is_autocommit() {
            return;
        }
        // Skip pooling if the database file was removed or replaced on disk.
        if PoolKey::for_path(&self.key.path, self.key.mode).as_ref() != Some(&self.key) {
            return;
        }
        let validated = self.validated.lock().ok().and_then(|v| v.clone());
        put(
            self.key.clone(),
            PooledConn {
                conn,
                history,
                validated,
            },
        );
    }
}

struct PoolState {
    buckets: HashMap<PoolKey, Vec<PooledConn>>,
    order: VecDeque<PoolKey>,
    total: usize,
}

impl PoolState {
    fn new() -> Self {
        Self {
            buckets: HashMap::new(),
            order: VecDeque::new(),
            total: 0,
        }
    }

    fn take(&mut self, key: &PoolKey) -> Option<PooledConn> {
        let bucket = self.buckets.get_mut(key)?;
        let item = bucket.pop()?;
        self.total = self.total.saturating_sub(1);
        if bucket.is_empty() {
            self.buckets.remove(key);
        }
        if let Some(pos) = self.order.iter().rposition(|k| k == key) {
            self.order.remove(pos);
        }
        Some(item)
    }

    fn put(&mut self, key: PoolKey, item: PooledConn) {
        let limit = key.mode.max_idle();
        let bucket = self.buckets.entry(key.clone()).or_default();
        if bucket.len() >= limit {
            return;
        }
        bucket.push(item);
        self.order.push_back(key);
        self.total += 1;
        while self.total > MAX_TOTAL_IDLE {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if let Some(bucket) = self.buckets.get_mut(&oldest) {
                if !bucket.is_empty() {
                    bucket.remove(0);
                    self.total = self.total.saturating_sub(1);
                }
                if bucket.is_empty() {
                    self.buckets.remove(&oldest);
                }
            }
        }
    }
}

static POOL: LazyLock<Mutex<PoolState>> = LazyLock::new(|| Mutex::new(PoolState::new()));

pub(crate) fn take(key: &PoolKey) -> Option<PooledConn> {
    POOL.lock().ok()?.take(key)
}

fn put(key: PoolKey, item: PooledConn) {
    if let Ok(mut pool) = POOL.lock() {
        pool.put(key, item);
    }
}

#[cfg(test)]
fn idle_count(path: &Path, mode: PoolMode) -> usize {
    let Some(key) = PoolKey::for_path(path, mode) else {
        return 0;
    };
    POOL.lock()
        .ok()
        .and_then(|pool| pool.buckets.get(&key).map(Vec::len))
        .unwrap_or(0)
}

/// Fast-path checkout validation:
/// 1. `PRAGMA data_version` (<1 µs shm read): unchanged means no other
///    connection committed anything since this connection last validated.
/// 2. If `data_version` changed, check `(PRAGMA schema_version,
///    schema_migrations count & max rowid)` (<5 µs): unchanged means no DDL
///    or data-only migration occurred.
pub(crate) fn is_stamp_fresh(
    conn: &Connection,
    stamp: &mut ValidatedStamp,
    require_shared_check: bool,
) -> bool {
    if require_shared_check && !stamp.shared_checked {
        return false;
    }
    let Ok(data_version) = read_data_version(conn) else {
        return false;
    };
    if data_version == stamp.data_version {
        return true;
    }
    let Ok((schema_version, count, max_rowid)) = read_schema_fingerprint(conn) else {
        return false;
    };
    if schema_version == stamp.schema_version
        && count == stamp.migration_count
        && max_rowid == stamp.migration_max_rowid
    {
        stamp.data_version = data_version;
        return true;
    }
    false
}

pub(crate) fn stamp_for(
    conn: &Connection,
    shared_checked: bool,
    tokens_migrated: bool,
) -> Option<ValidatedStamp> {
    let data_version = read_data_version(conn).ok()?;
    let (schema_version, migration_count, migration_max_rowid) =
        read_schema_fingerprint(conn).ok()?;
    Some(ValidatedStamp {
        data_version,
        schema_version,
        migration_count,
        migration_max_rowid,
        shared_checked,
        tokens_migrated,
    })
}

fn read_data_version(conn: &Connection) -> StoreResult<i64> {
    Ok(conn.pragma_query_value(None, "data_version", |row| row.get(0))?)
}

fn read_schema_fingerprint(conn: &Connection) -> StoreResult<(i64, i64, i64)> {
    let schema_version: i64 = conn.pragma_query_value(None, "schema_version", |row| row.get(0))?;
    let (count, max_rowid): (i64, i64) = conn.query_row(
        "SELECT COUNT(*), COALESCE(MAX(rowid), 0) FROM schema_migrations",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    Ok((schema_version, count, max_rowid))
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::{idle_count, PoolMode};
    use crate::sqlite::{SqliteStore, SQLITE_WRITE_BUSY_TIMEOUT};
    use crate::test_fixture;
    use crate::token_crypto::TokenCryptoError;
    use crate::{CredentialType, ProviderToken};

    fn unavailable_key() -> Result<[u8; 32], TokenCryptoError> {
        Err(TokenCryptoError::KeyRetrieval("key unavailable".into()))
    }

    fn fixed_key() -> Result<[u8; 32], TokenCryptoError> {
        Ok([9u8; 32])
    }

    fn conn_handle(store: &SqliteStore) -> usize {
        let conn = store.conn.lock().unwrap();
        // SAFETY: `handle()` is only cast to an integer address for identity
        // comparison in tests; the pointer is never dereferenced.
        unsafe { conn.handle() as usize }
    }

    #[test]
    fn sequential_opens_reuse_one_physical_connection() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("loopflow.db");
        let build = test_fixture::build();
        let observers = test_fixture::observers();

        // First open creates and migrates the DB, then returns the warm connection to the pool.
        let first = SqliteStore::open_prepared(
            &path,
            false,
            true,
            SQLITE_WRITE_BUSY_TIMEOUT,
            &build,
            &observers,
            fixed_key,
        )
        .unwrap();
        let first_handle = conn_handle(&first);
        drop(first);
        assert_eq!(idle_count(&path, PoolMode::Write), 1);

        let second = SqliteStore::open_prepared(
            &path,
            false,
            true,
            SQLITE_WRITE_BUSY_TIMEOUT,
            &build,
            &observers,
            fixed_key,
        )
        .unwrap();
        assert_eq!(idle_count(&path, PoolMode::Write), 0);
        assert_eq!(
            conn_handle(&second),
            first_handle,
            "sequential write open should reuse the exact same sqlite3 handle"
        );
        second.health_check().unwrap();
        drop(second);
        assert_eq!(idle_count(&path, PoolMode::Write), 1);

        let ro_first = SqliteStore::open_read_only_validated(
            &path, false, true, &build, &observers, fixed_key,
        )
        .unwrap();
        let ro_handle = conn_handle(&ro_first);
        ro_first.health_check().unwrap();
        drop(ro_first);
        assert_eq!(idle_count(&path, PoolMode::ReadOnly), 1);

        let ro_second = SqliteStore::open_read_only_validated(
            &path, false, true, &build, &observers, fixed_key,
        )
        .unwrap();
        assert_eq!(
            conn_handle(&ro_second),
            ro_handle,
            "sequential validated read-only open should reuse the exact same sqlite3 handle"
        );
        ro_second.health_check().unwrap();
        drop(ro_second);
        assert_eq!(idle_count(&path, PoolMode::ReadOnly), 1);
    }

    #[test]
    fn token_key_is_per_caller_and_never_pooled() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("loopflow.db");
        let build = test_fixture::build();
        let observers = test_fixture::observers();

        let token = ProviderToken {
            provider: "linear".into(),
            access_token: "secret-access".into(),
            refresh_token: Some("secret-refresh".into()),
            oauth_client_id: None,
            expires_at: None,
            login: None,
            updated_at: 1,
            credential_type: CredentialType::OAuth,
        };

        let writer = SqliteStore::open_prepared(
            &path,
            false,
            true,
            SQLITE_WRITE_BUSY_TIMEOUT,
            &build,
            &observers,
            fixed_key,
        )
        .unwrap();
        let handle = conn_handle(&writer);
        writer.upsert_provider_token(&token).unwrap();
        drop(writer);

        let denied = SqliteStore::open_prepared(
            &path,
            false,
            true,
            SQLITE_WRITE_BUSY_TIMEOUT,
            &build,
            &observers,
            unavailable_key,
        )
        .unwrap();
        assert_eq!(conn_handle(&denied), handle);
        assert!(
            denied.get_provider_token("linear").is_err(),
            "pooled connection must use caller's unavailable token_key"
        );
        drop(denied);

        let allowed = SqliteStore::open_prepared(
            &path,
            false,
            true,
            SQLITE_WRITE_BUSY_TIMEOUT,
            &build,
            &observers,
            fixed_key,
        )
        .unwrap();
        assert_eq!(conn_handle(&allowed), handle);
        assert_eq!(allowed.get_provider_token("linear").unwrap(), Some(token));
    }

    #[test]
    fn ddl_and_data_only_schema_changes_trigger_revalidation_and_refuse_behind_store() {
        let build = test_fixture::build();
        let observers = test_fixture::observers();

        // 1. Data-only change to `schema_migrations` (no DDL, schema_version unchanged):
        // removing the head migration makes the store behind this binary, which a
        // pooled shared checkout must detect and refuse.
        let dir1 = tempfile::tempdir().unwrap();
        let path1 = dir1.path().join("loopflow.db");
        drop(
            SqliteStore::open_prepared(
                &path1,
                false,
                true,
                SQLITE_WRITE_BUSY_TIMEOUT,
                &build,
                &observers,
                fixed_key,
            )
            .unwrap(),
        );
        drop(
            SqliteStore::open_prepared(
                &path1,
                true,
                false,
                SQLITE_WRITE_BUSY_TIMEOUT,
                &build,
                &observers,
                fixed_key,
            )
            .unwrap(),
        );

        let external = Connection::open(&path1).unwrap();
        let head = crate::migration_runner::latest_known_version();
        external
            .execute("DELETE FROM schema_migrations WHERE version = ?1", [&head])
            .unwrap();
        drop(external);

        let err = SqliteStore::open_prepared(
            &path1,
            true,
            false,
            SQLITE_WRITE_BUSY_TIMEOUT,
            &build,
            &observers,
            fixed_key,
        )
        .unwrap_err()
        .to_string();
        assert!(
            err.contains(&head),
            "pooled connection must detect data-only migration change and refuse behind store: {err}"
        );

        // 2. DDL change (newer additive migration applied externally): pooled shared
        // checkout revalidates and keeps working.
        let dir2 = tempfile::tempdir().unwrap();
        let path2 = dir2.path().join("loopflow.db");
        drop(
            SqliteStore::open_prepared(
                &path2,
                false,
                true,
                SQLITE_WRITE_BUSY_TIMEOUT,
                &build,
                &observers,
                fixed_key,
            )
            .unwrap(),
        );
        let shared = SqliteStore::open_prepared(
            &path2,
            true,
            false,
            SQLITE_WRITE_BUSY_TIMEOUT,
            &build,
            &observers,
            fixed_key,
        )
        .unwrap();
        let handle = conn_handle(&shared);
        drop(shared);

        let external = Connection::open(&path2).unwrap();
        crate::migration_runner::apply_newer_migration(
            &observers,
            &build,
            &external,
            "CREATE TABLE extra_pool_check (id TEXT PRIMARY KEY);",
        )
        .unwrap();
        drop(external);

        let after_ddl = SqliteStore::open_prepared(
            &path2,
            true,
            false,
            SQLITE_WRITE_BUSY_TIMEOUT,
            &build,
            &observers,
            fixed_key,
        )
        .unwrap();
        assert_eq!(conn_handle(&after_ddl), handle);
        after_ddl.health_check().unwrap();
    }

    #[test]
    fn mid_transaction_and_escaped_conn_are_not_returned_to_pool() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("loopflow.db");
        let build = test_fixture::build();
        let observers = test_fixture::observers();

        let store = SqliteStore::open_prepared(
            &path,
            false,
            true,
            SQLITE_WRITE_BUSY_TIMEOUT,
            &build,
            &observers,
            fixed_key,
        )
        .unwrap();
        // Leave connection mid-transaction before drop.
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch("BEGIN DEFERRED")
            .unwrap();
        drop(store);
        assert_eq!(
            idle_count(&path, PoolMode::Write),
            0,
            "mid-transaction connection must be closed, not pooled"
        );

        let fresh = SqliteStore::open_prepared(
            &path,
            false,
            true,
            SQLITE_WRITE_BUSY_TIMEOUT,
            &build,
            &observers,
            fixed_key,
        )
        .unwrap();
        assert!(fresh.conn.lock().unwrap().is_autocommit());
        // Escape a clone of `conn` past drop.
        let escaped = fresh.conn.clone();
        drop(fresh);
        assert_eq!(
            idle_count(&path, PoolMode::Write),
            0,
            "connection with escaped Arc clone must not be pooled"
        );
        drop(escaped);
    }
}
