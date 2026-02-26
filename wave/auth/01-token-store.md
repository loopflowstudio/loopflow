# 01: Token Store

DB-backed token persistence, replacing filesystem detection as the primary auth status source.

## What to build

### Migration (016_provider_tokens.sql)

```sql
CREATE TABLE IF NOT EXISTS provider_tokens (
    provider TEXT PRIMARY KEY,
    access_token TEXT NOT NULL,
    refresh_token TEXT,
    expires_at BIGINT,
    login TEXT,
    updated_at BIGINT NOT NULL
);
```

`provider` matches `Provider::as_str()`: `"github"`, `"claude"`, `"codex"`. `expires_at` is Unix seconds, NULL if the provider doesn't expose expiry. Plaintext tokens for now — lfd.db is 0600.

### ProviderToken struct

```rust
#[derive(Debug, Clone)]
pub struct ProviderToken {
    pub provider: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<i64>,
    pub login: Option<String>,
    pub updated_at: i64,
}
```

### Store trait extension

Add to a new `TokenStore` capability (following the `WaveStateStore`/`ExecutionStore`/`SessionStore` pattern):

```rust
async fn get_provider_token(&self, provider: &str) -> StoreResult<Option<ProviderToken>>;
async fn upsert_provider_token(&self, token: &ProviderToken) -> StoreResult<()>;
async fn delete_provider_token(&self, provider: &str) -> StoreResult<()>;
async fn list_provider_tokens(&self) -> StoreResult<Vec<ProviderToken>>;
```

Implement in both `sqlite.rs` and `postgres.rs`. Follow the `upsert_live_pr_state` pattern for upsert shape.

### Token extraction after auth flow

Add to `AuthBroker` trait with a default no-op:

```rust
async fn extract_token(&self) -> Option<ProviderToken> {
    None
}
```

Per-broker extraction:
- **GitHub**: parse `oauth_token` from `~/.config/gh/hosts.yml` (already read by `read_github_login`)
- **Claude**: parse `accessToken` + `refreshToken` from `~/.claude/.credentials.json` (may not exist on Mac where keychain is primary — return None)
- **Codex**: parse from `~/.codex/auth.json` if it exists

In `ProviderAuthService::start_auth`, after the lifecycle task sees `auth.connected`, call `extract_token` and upsert to store. Thread `SharedStore` into `ProviderAuthService::new()`.

### check_status: DB-first

Each broker's `check_status` checks the store first. If a token row exists and isn't expired, return `Active` with the stored login. If expired, return `Expired`. If no row, fall back to existing filesystem probe.

### disconnect: delete row

`ProviderAuthService::disconnect` calls `delete_provider_token` after the broker-level disconnect.

## Constraints

- Filesystem heuristics stay as fallback. Existing installs that authenticated before this phase must keep working.
- Don't encrypt tokens beyond filesystem permissions in this phase. Add a TODO comment.
- `ProviderAuthService::new()` gains a `SharedStore` parameter. Update all callers.

## Validation

```bash
cargo fmt --all -- --check
cargo clippy -p loopflow --all-targets -- -D warnings
cargo test -p loopflow provider_auth
cargo test -p loopflow store
```

## Done when

- Token round-trips through SQLite and Postgres (upsert, read, delete)
- `check_status` returns Active from DB when filesystem artifact is absent
- `disconnect` removes the DB row
- `start_auth` captures and stores the token after successful auth flow
- Existing filesystem-only auth still works when no DB row exists
