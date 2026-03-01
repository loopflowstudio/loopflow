use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use tokio::sync::RwLock;

const DEFAULT_TOKEN_TTL: Duration = Duration::from_secs(60 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnectionTokenState {
    Available,
    Claimed,
    Revoked,
}

impl ConnectionTokenState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Claimed => "claimed",
            Self::Revoked => "revoked",
        }
    }

    fn from_db(value: &str) -> Self {
        match value {
            "claimed" => Self::Claimed,
            "revoked" => Self::Revoked,
            _ => Self::Available,
        }
    }
}

#[derive(Debug, Clone)]
struct ConnectionTokenRecord {
    hash: String,
    state: ConnectionTokenState,
    expires_at: i64,
    claimed_at: Option<i64>,
    revoked_at: Option<i64>,
}

impl ConnectionTokenRecord {
    fn is_expired(&self, now: i64) -> bool {
        self.expires_at <= now
    }

    fn allows_use(&self, now: i64) -> bool {
        !self.is_expired(now) && self.state != ConnectionTokenState::Revoked
    }
}

#[derive(Debug, Clone)]
pub struct TokenLedger {
    db_path: Arc<PathBuf>,
    cache: Arc<RwLock<HashMap<String, ConnectionTokenRecord>>>,
    ttl: Duration,
}

impl TokenLedger {
    pub async fn new(db_path: PathBuf) -> Result<Self, TokenLedgerError> {
        Self::with_ttl(db_path, DEFAULT_TOKEN_TTL).await
    }

    pub async fn with_ttl(db_path: PathBuf, ttl: Duration) -> Result<Self, TokenLedgerError> {
        let ledger = Self {
            db_path: Arc::new(db_path),
            cache: Arc::new(RwLock::new(HashMap::new())),
            ttl,
        };
        ledger.ensure_schema().await?;
        ledger.load_cache().await?;
        Ok(ledger)
    }

    pub async fn mint(&self, count: usize) -> Result<Vec<String>, TokenLedgerError> {
        if count == 0 {
            return Ok(Vec::new());
        }

        let ttl = self.ttl;
        let minted = self
            .run_db(move |conn| {
                let now = now_unix();
                let expires_at = now + i64::try_from(ttl.as_secs()).unwrap_or(3600);
                let tx = conn.unchecked_transaction()?;
                let mut inserted = Vec::with_capacity(count);
                while inserted.len() < count {
                    let token = crate::lfd::session_token::generate();
                    let hash = hash_token(&token);
                    let affected = tx.execute(
                        "INSERT OR IGNORE INTO connection_tokens
                         (token_hash, state, issued_at, expires_at, claimed_at, revoked_at)
                         VALUES (?1, ?2, ?3, ?4, NULL, NULL)",
                        params![
                            hash,
                            ConnectionTokenState::Available.as_str(),
                            now,
                            expires_at
                        ],
                    )?;
                    if affected == 1 {
                        inserted.push((token, hash, expires_at));
                    }
                }
                tx.commit()?;
                Ok(inserted)
            })
            .await?;

        let mut cache = self.cache.write().await;
        let mut tokens = Vec::with_capacity(minted.len());
        for (token, hash, expires_at) in minted {
            tokens.push(token);
            cache.insert(
                hash.clone(),
                ConnectionTokenRecord {
                    hash,
                    state: ConnectionTokenState::Available,
                    expires_at,
                    claimed_at: None,
                    revoked_at: None,
                },
            );
        }
        Ok(tokens)
    }

    pub async fn validate(&self, token: &str) -> Result<bool, TokenLedgerError> {
        if token.trim().is_empty() {
            return Ok(false);
        }

        let hash = hash_token(token);
        let now = now_unix();
        {
            let mut cache = self.cache.write().await;
            if let Some(record) = cache.get_mut(&hash) {
                if !record.allows_use(now) {
                    return Ok(false);
                }
                if record.state == ConnectionTokenState::Available {
                    self.mark_claimed(&hash, now).await?;
                    record.state = ConnectionTokenState::Claimed;
                    record.claimed_at = Some(now);
                }
                return Ok(true);
            }
        }

        let Some(mut record) = self.load_record(&hash).await? else {
            return Ok(false);
        };
        if !record.allows_use(now) {
            return Ok(false);
        }
        if record.state == ConnectionTokenState::Available {
            self.mark_claimed(&hash, now).await?;
            record.state = ConnectionTokenState::Claimed;
            record.claimed_at = Some(now);
        }

        self.cache.write().await.insert(hash, record);
        Ok(true)
    }

    pub async fn revoke(&self, token_hash_prefix: &str) -> Result<u32, TokenLedgerError> {
        let prefix = token_hash_prefix.trim().to_ascii_lowercase();
        if prefix.is_empty() {
            return Err(TokenLedgerError::InvalidPrefix);
        }

        let now = now_unix();
        let like_pattern = format!("{prefix}%");
        let revoked = self
            .run_db(move |conn| {
                let affected = conn.execute(
                    "UPDATE connection_tokens
                     SET state = ?1, revoked_at = ?2
                     WHERE token_hash LIKE ?3 AND state != ?4",
                    params![
                        ConnectionTokenState::Revoked.as_str(),
                        now,
                        like_pattern,
                        ConnectionTokenState::Revoked.as_str()
                    ],
                )?;
                Ok(affected as u32)
            })
            .await?;

        if revoked > 0 {
            let mut cache = self.cache.write().await;
            for record in cache.values_mut() {
                if record.hash.starts_with(&prefix) {
                    record.state = ConnectionTokenState::Revoked;
                    record.revoked_at = Some(now);
                }
            }
        }

        Ok(revoked)
    }

    pub async fn revoke_all(&self) -> Result<u32, TokenLedgerError> {
        let now = now_unix();
        let revoked = self
            .run_db(move |conn| {
                let affected = conn.execute(
                    "UPDATE connection_tokens
                     SET state = ?1, revoked_at = ?2
                     WHERE state != ?3",
                    params![
                        ConnectionTokenState::Revoked.as_str(),
                        now,
                        ConnectionTokenState::Revoked.as_str()
                    ],
                )?;
                Ok(affected as u32)
            })
            .await?;

        if revoked > 0 {
            let mut cache = self.cache.write().await;
            for record in cache.values_mut() {
                record.state = ConnectionTokenState::Revoked;
                record.revoked_at = Some(now);
            }
        }
        Ok(revoked)
    }

    pub async fn prune(&self) -> Result<u32, TokenLedgerError> {
        let now = now_unix();
        let deleted = self
            .run_db(move |conn| {
                let affected = conn.execute(
                    "DELETE FROM connection_tokens WHERE expires_at <= ?1",
                    params![now],
                )?;
                Ok(affected as u32)
            })
            .await?;

        if deleted > 0 {
            let mut cache = self.cache.write().await;
            cache.retain(|_, record| !record.is_expired(now));
        }
        Ok(deleted)
    }

    pub async fn available_count(&self) -> usize {
        let now = now_unix();
        self.cache
            .read()
            .await
            .values()
            .filter(|record| {
                record.state == ConnectionTokenState::Available && !record.is_expired(now)
            })
            .count()
    }

    async fn ensure_schema(&self) -> Result<(), TokenLedgerError> {
        self.run_db(|conn| {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS connection_tokens (
                    token_hash TEXT PRIMARY KEY,
                    state TEXT NOT NULL,
                    issued_at INTEGER NOT NULL,
                    expires_at INTEGER NOT NULL,
                    claimed_at INTEGER,
                    revoked_at INTEGER
                );
                CREATE INDEX IF NOT EXISTS idx_connection_tokens_state_expires
                    ON connection_tokens(state, expires_at);",
            )?;
            Ok(())
        })
        .await
    }

    async fn load_cache(&self) -> Result<(), TokenLedgerError> {
        let now = now_unix();
        let records = self
            .run_db(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT token_hash, state, expires_at, claimed_at, revoked_at
                     FROM connection_tokens
                     WHERE expires_at > ?1",
                )?;
                let rows = stmt.query_map(params![now], |row| {
                    Ok(ConnectionTokenRecord {
                        hash: row.get(0)?,
                        state: ConnectionTokenState::from_db(row.get::<_, String>(1)?.as_str()),
                        expires_at: row.get(2)?,
                        claimed_at: row.get(3)?,
                        revoked_at: row.get(4)?,
                    })
                })?;
                rows.collect::<Result<Vec<_>, _>>()
                    .map_err(TokenLedgerError::from)
            })
            .await?;

        let mut cache = self.cache.write().await;
        cache.clear();
        for record in records {
            cache.insert(record.hash.clone(), record);
        }
        Ok(())
    }

    async fn load_record(
        &self,
        token_hash: &str,
    ) -> Result<Option<ConnectionTokenRecord>, TokenLedgerError> {
        let token_hash = token_hash.to_string();
        self.run_db(move |conn| {
            conn.query_row(
                "SELECT token_hash, state, expires_at, claimed_at, revoked_at
                 FROM connection_tokens
                 WHERE token_hash = ?1",
                params![token_hash],
                |row| {
                    Ok(ConnectionTokenRecord {
                        hash: row.get(0)?,
                        state: ConnectionTokenState::from_db(row.get::<_, String>(1)?.as_str()),
                        expires_at: row.get(2)?,
                        claimed_at: row.get(3)?,
                        revoked_at: row.get(4)?,
                    })
                },
            )
            .optional()
            .map_err(TokenLedgerError::from)
        })
        .await
    }

    async fn mark_claimed(
        &self,
        token_hash: &str,
        claimed_at: i64,
    ) -> Result<(), TokenLedgerError> {
        let token_hash = token_hash.to_string();
        self.run_db(move |conn| {
            conn.execute(
                "UPDATE connection_tokens
                 SET state = ?1, claimed_at = ?2
                 WHERE token_hash = ?3 AND state = ?4",
                params![
                    ConnectionTokenState::Claimed.as_str(),
                    claimed_at,
                    token_hash,
                    ConnectionTokenState::Available.as_str()
                ],
            )?;
            Ok(())
        })
        .await
    }

    async fn run_db<T, F>(&self, operation: F) -> Result<T, TokenLedgerError>
    where
        T: Send + 'static,
        F: FnOnce(Connection) -> Result<T, TokenLedgerError> + Send + 'static,
    {
        let path = self.db_path.as_ref().clone();
        tokio::task::spawn_blocking(move || {
            let connection = open_connection(&path)?;
            operation(connection)
        })
        .await
        .map_err(|error| TokenLedgerError::Internal(format!("token ledger task failed: {error}")))?
    }
}

fn open_connection(path: &Path) -> Result<Connection, TokenLedgerError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            TokenLedgerError::Internal(format!("failed creating token ledger dir: {error}"))
        })?;
    }

    let connection = Connection::open(path)?;
    connection.execute_batch("PRAGMA journal_mode = WAL; PRAGMA busy_timeout = 5000;")?;
    Ok(connection)
}

fn hash_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    hex::encode(digest)
}

fn now_unix() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

#[derive(Debug, thiserror::Error)]
pub enum TokenLedgerError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("invalid token hash prefix")]
    InvalidPrefix,
    #[error("{0}")]
    Internal(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn minted_tokens_transition_to_claimed_and_remain_valid() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ledger = TokenLedger::new(dir.path().join("ledger.db"))
            .await
            .expect("ledger");
        let tokens = ledger.mint(1).await.expect("mint");
        let token = tokens.first().expect("token").clone();

        assert!(ledger.validate(&token).await.expect("validate first use"));
        assert!(ledger
            .validate(&token)
            .await
            .expect("validate claimed token reuse"));
        assert_eq!(ledger.available_count().await, 0);
    }

    #[tokio::test]
    async fn unknown_token_is_rejected() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ledger = TokenLedger::new(dir.path().join("ledger.db"))
            .await
            .expect("ledger");
        assert!(!ledger.validate("not-known").await.expect("validate"));
    }

    #[tokio::test]
    async fn revoke_prefix_disables_matching_tokens() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ledger = TokenLedger::new(dir.path().join("ledger.db"))
            .await
            .expect("ledger");
        let token = ledger.mint(1).await.expect("mint").pop().expect("token");
        let hash = hash_token(&token);
        let prefix = &hash[..12];
        let revoked = ledger.revoke(prefix).await.expect("revoke");
        assert_eq!(revoked, 1);
        assert!(!ledger.validate(&token).await.expect("validate revoked"));
    }

    #[tokio::test]
    async fn revoke_all_disables_all_tokens() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ledger = TokenLedger::new(dir.path().join("ledger.db"))
            .await
            .expect("ledger");
        let tokens = ledger.mint(3).await.expect("mint");
        assert_eq!(ledger.revoke_all().await.expect("revoke all"), 3);
        for token in tokens {
            assert!(!ledger
                .validate(&token)
                .await
                .expect("validate revoked token"));
        }
    }

    #[tokio::test]
    async fn prune_removes_expired_tokens() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ledger = TokenLedger::with_ttl(dir.path().join("ledger.db"), Duration::from_millis(20))
            .await
            .expect("ledger");
        let token = ledger.mint(1).await.expect("mint").pop().expect("token");
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert!(!ledger
            .validate(&token)
            .await
            .expect("expired token rejected"));
        assert_eq!(ledger.prune().await.expect("prune"), 1);
    }

    #[tokio::test]
    async fn cache_miss_falls_back_to_sqlite() {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("ledger.db");
        let token = {
            let ledger = TokenLedger::new(db_path.clone()).await.expect("ledger");
            ledger.mint(1).await.expect("mint").pop().expect("token")
        };

        let reloaded = TokenLedger::new(db_path).await.expect("reloaded");
        assert!(reloaded
            .validate(&token)
            .await
            .expect("validate after reload"));
    }
}
