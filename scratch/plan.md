# Plan: Replace credential socket with DB-backed token store

## Context

The bundled container branch added a Unix socket bridge (Swift CredentialSocketServer ↔ Rust CredentialSocketClient) to proxy macOS Keychain credentials into a containerized lfd. The auth wave in loopflow.tmux designed a better approach: lfd owns tokens in its own DB, extracts them after CLI auth flows, and injects them as env vars at agent launch. This plan pulls that work forward, replacing the socket.

## What changes

### 1. Migration: `016_provider_tokens.sql`

```sql
CREATE TABLE IF NOT EXISTS provider_tokens (
    provider TEXT PRIMARY KEY,
    access_token TEXT NOT NULL,
    refresh_token TEXT,
    expires_at INTEGER,
    login TEXT,
    updated_at INTEGER NOT NULL
);
```

Portable SQL, works in both SQLite and Postgres. Add entry to `migrations.rs`.

### 2. `ProviderToken` struct + `TokenStore` trait

New file: `rust/loopflow/src/lfd/store/tokens.rs` (or inline in `mod.rs` following the session pattern).

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

Store methods (inline SQL, following sessions pattern):
- `get_provider_token(provider: &str) -> StoreResult<Option<ProviderToken>>`
- `upsert_provider_token(token: &ProviderToken) -> StoreResult<()>`
- `delete_provider_token(provider: &str) -> StoreResult<()>`
- `list_provider_tokens() -> StoreResult<Vec<ProviderToken>>`

Implement in both `sqlite.rs` and `postgres.rs`. Add `TokenStore` trait + dispatch in `mod.rs`.

### 3. Token extraction after auth flow

Add to `AuthBroker` trait with a default no-op:
```rust
async fn extract_token(&self) -> Option<ProviderToken> { None }
```

Per-broker extraction:
- **GhAuthBroker**: parse `oauth_token` from `~/.config/gh/hosts.yml` (already read by `read_github_login`)
- **ClaudeAuthBroker**: parse `accessToken` from `~/.claude/.credentials.json` (return None if missing — Mac keychain primary)
- **CodexAuthBroker**: parse from `~/.codex/auth.json` if it exists

### 4. `ProviderAuthService` gains `SharedStore`

`ProviderAuthService::new()` → `ProviderAuthService::new(store: SharedStore)`.

- `check_status`: DB first. If token row exists and isn't expired → `Active`. If expired → `Expired`. No row → fall back to existing filesystem probe.
- `start_auth`: after lifecycle task sees auth connected, call `extract_token` + `upsert_provider_token`.
- `disconnect`: call `delete_provider_token` after broker disconnect.

Update all callers: `lfd.rs`, test helpers in `chords.rs`, `hooks.rs`, `system.rs`.

### 5. Credential injection from DB

New free function in `provider_auth.rs`:
```rust
pub async fn provider_env_vars(store: &Store) -> Vec<(String, String)>
```

Maps provider tokens to env vars:
- `github` → `GH_TOKEN`
- `claude` → `ANTHROPIC_API_KEY`
- `codex` → `OPENAI_API_KEY`

**DockerExecutor**: Replace `cached_credentials` HashMap + socket client init with `provider_env_vars(store)` call in `collect_env()`. Store is already available (passed to `DockerExecutor::new`).

**LocalProcessExecutor**: Call `provider_env_vars(store)` before spawning agent process. Inject via `command.env()`. DB tokens take precedence; if no DB token exists, let host env through.

### 6. Delete credential socket code

Remove:
- `rust/loopflow/src/lfd/credential_socket.rs` (~323 lines)
- `SocketAuthBroker` from `provider_auth.rs` (~70 lines)
- `CredentialSocketServer.swift` (~337 lines)
- Socket mount in `BundledDaemonManager.buildDockerRunArgs`
- `credentialServer` lifecycle in `BundledDaemonManager`
- `LFD_CREDENTIAL_SOCKET` env var handling in `config.rs`
- `credential_socket` field from `LfdConfig`

### 7. Concerto: simplify BundledDaemonManager

Remove `CredentialSocketServer` start/stop from container launch. The Docker args lose the `-v .../concerto-auth.sock` mount and `LFD_CREDENTIAL_SOCKET` env var. Everything else stays.

### 8. Swift CredentialSocketServer.swift → delete entirely

No longer needed. Concerto doesn't proxy credentials.

## What stays unchanged

- `BundledDaemonManager` container/native mode toggle and UI
- `ConnectionSettingsView` provider connections UI (still goes through lfd HTTP API)
- `ConcertoConfig` container config parsing (mounts, image)
- Docker socket mount (`/var/run/docker.sock`) — still needed for executor
- Wave README and `07-bundled-container-hardening.md`

## Sequencing

1. Migration + ProviderToken struct + store methods + tests
2. AuthBroker extract_token + ProviderAuthService store integration
3. provider_env_vars + executor injection
4. Delete credential socket (Rust + Swift)
5. Update BundledDaemonManager to remove socket lifecycle
6. cargo fmt/clippy/test, Swift build verification

## Out of scope (later auth wave phases)

- Proactive token refresh (02-proactive-refresh)
- `lfd install` interactive onboarding (04-install-onboarding)
- Token encryption beyond file permissions
