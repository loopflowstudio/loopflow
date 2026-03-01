# Discovery Auth Modes

## Problem

Mobile discovery works — lfd registers with studio, iOS finds daemons. But connecting is manual: copy a host, port, and token into the iOS app. There's no auth model for handing out short-lived credentials to authenticated mobile users.

The missing piece: studio knows who you are and which daemons are yours, but can't give you a credential that the daemon will accept. Today's auth is either a long-lived static token (shared manually) or studio-validated (every connection round-trips to studio). Neither fits mobile: static tokens are hard to distribute, and studio-validated adds latency and a single point of failure.

We need a mode where lfd mints its own tokens, shares a pool with studio, and studio hands them out to authenticated users on discover. Local desktop connections keep working with the static token. Studio going down doesn't break active sessions.

## Approach

Add a fourth `AuthProvider` variant: `DualAuth`. It accepts two kinds of credentials:

1. **Static token** — constant-time match, loopback only. Desktop Concerto uses this. Identical to today's `Static` variant for local connections.
2. **Connection tokens** — 256-bit random tokens minted by lfd, distributed via studio. Mobile uses these. Validated locally against an in-memory cache backed by SQLite.

A desktop toggle in Concerto ("Connect with my phone") flips lfd between `Static` auth on `127.0.0.1` and `DualAuth` on `0.0.0.0`.

### Token lifecycle

```
lfd mints 5 tokens → sends pool to studio on register
                                                         ↓
studio stores pool ← mobile user hits discover → studio hands out 1 token
                                                         ↓
mobile connects to lfd with token → lfd validates locally (Available → Claimed)
                                                         ↓
heartbeat: lfd sends new_tokens if pool < 2 ← studio reports tokens_remaining
                                                         ↓
token expires after 1 hour → pruned from ledger
```

Tokens are single-claim, not single-use: once claimed, the same token keeps working (avoids burning tokens on WebSocket reconnects). Revoked or unknown tokens are rejected.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Studio-validated only (`ConnectionValidator`) | Every remote connection round-trips to studio. Studio outage blocks new connections. Studio sees every connection event. | Latency on connect. SPOF. Privacy. |
| Challenge-response with asymmetric keys | lfd generates keypair, studio signs tokens with it. No pool management needed — studio can mint unlimited tokens. | Over-engineered for v1. Key distribution adds complexity. Crypto is harder to audit. |
| Extend static token to network | Just share the static token and allow non-loopback bind. | One leaked token compromises everything permanently. No expiry, no revocation, no rotation. |
| HMAC-signed tokens (studio-issued) | Studio signs tokens with a shared secret. lfd validates signature locally. | Requires shared secret distribution. Clock skew issues with expiry. No lfd-side revocation without maintaining a blacklist anyway. |

## Key decisions

**Single-claim, not single-use.** A token transitions `Available → Claimed` on first use and stays `Claimed`. This matters because WebSocket connections drop and reconnect — burning a token per reconnect would exhaust the pool. A claimed token works until it expires (1 hour) or is revoked.

**In-memory cache over SQLite.** The token ledger holds a `RwLock<HashMap>` in front of the SQLite `tokens` table. Normal operation (5 tokens, rare validation) hits only memory. SQLite provides persistence across daemon restarts. Cache misses fall through to SQLite — this handles the startup case where tokens exist in the database but not yet in memory.

**Loopback restriction on static token.** In DualAuth mode, the static token is only accepted from loopback addresses. This is a deliberate tightening: when the daemon is network-accessible, you can't use the long-lived static token from a remote machine. Only connection tokens work remotely.

**WS re-validation is new architecture.** No per-session validation exists after HTTP upgrade today. Adding a 60-second timer per WebSocket session is architecturally new. On validation failure, close with frame `4401` and drop. This prevents a revoked token from holding an open session indefinitely.

**TLS is a startup guard, not a feature.** We refuse to start in DualAuth/Studio mode on non-loopback, non-Tailscale HTTP binds. This prevents accidentally exposing the daemon unencrypted on a public network. Actual TLS serving is separate work — no cert loading or TLS config parsing exists today.

## Scope

### In scope

1. **Token ledger** — new Rust module `lfd/token_ledger.rs`
2. **`AuthProvider::DualAuth`** — new variant in `lfd/auth.rs`
3. **Registration payload extension** — `connection_tokens` in register, `new_tokens`/`tokens_remaining`/`revoke` in heartbeat
4. **WebSocket re-validation** — per-session timer in `ws.rs`
5. **Desktop toggle** — "Connect with my phone" in `ConnectionSettingsView`
6. **Token revocation** — CLI command + HTTP endpoint + heartbeat-driven
7. **TLS startup guard** — refuse insecure remote bind
8. **Container mode default** — `mode: container` defaults auth to `loopflow.studio`

### Out of scope

- Per-repo or per-capability token scoping
- Actual TLS serving (cert loading, config)
- IP-based access control (security relies on unguessable tokens)
- Studio-side endpoints (coordinated but in studio repo)
- Tailscale LocalAPI migration (follow-up from 01)

## Implementation

### 1. Token ledger (`rust/loopflow/src/lfd/token_ledger.rs`)

New module. Owns a `tokens` table via SQLite migration.

```rust
pub struct TokenLedger {
    conn: Arc<Mutex<Connection>>,   // shared with SqliteStore
    cache: RwLock<HashMap<String, TokenEntry>>,
}

struct TokenEntry {
    status: TokenStatus,
    created_at: Instant,
}

enum TokenStatus { Available, Claimed, Revoked }
```

**API:**
- `mint(count: usize) -> Vec<SecretString>` — generate 256-bit random tokens, insert as `Available`, return hex strings
- `validate(token: &str) -> bool` — check cache → fallback to SQLite. `Available` → transition to `Claimed` and return true. `Claimed` → return true. `Revoked`/missing → return false.
- `revoke(prefix: &str)` — mark matching tokens as `Revoked` in both cache and SQLite
- `revoke_all()` — revoke everything
- `prune()` — delete tokens older than `TOKEN_TTL` (1 hour)

**Constants:**
- `TOKEN_TTL`: 1 hour
- `INITIAL_POOL_SIZE`: 5
- `REPLENISH_THRESHOLD`: 2 (mint more when pool drops below this)
- `PRUNE_INTERVAL`: 5 minutes

**Migration** (`027_connection_tokens.sql`):
```sql
CREATE TABLE IF NOT EXISTS tokens (
    token_hash TEXT PRIMARY KEY,
    status TEXT NOT NULL DEFAULT 'available',
    created_at INTEGER NOT NULL,
    claimed_at INTEGER
);
```

Store the SHA-256 hash of the token, not the token itself. Minting returns the raw token; only the hash persists. Validation hashes the incoming token and looks up the hash.

**Pruning:** Spawn a `tokio::interval` task on ledger creation. Every 5 minutes, delete rows where `created_at` is older than 1 hour.

### 2. `AuthProvider::DualAuth` (`rust/loopflow/src/lfd/auth.rs`)

New variant on the existing enum:

```rust
pub enum AuthProvider {
    Local { session_token: SecretString },
    Static { token: SecretString },
    Studio { validator: ConnectionValidator },
    DualAuth { local_token: SecretString, ledger: Arc<TokenLedger> },
}
```

**Auth logic in middleware** (new arm in `authorize_with_provider`):

```rust
AuthProvider::DualAuth { local_token, ledger } => {
    let peer_is_loopback = peer_addr.ip().is_loopback();
    if peer_is_loopback && token_matches(local_token, provided_token) {
        true
    } else {
        ledger.validate(provided_token).await
    }
}
```

Static token match uses existing `token_matches()` (constant-time via `subtle::ConstantTimeEq`). Ledger validation is not constant-time (timing doesn't leak token values — the hash lookup is the secret, not the comparison).

**Config value:** `auth.provider: "dual"`. Parsed in `setup_auth()` alongside existing variants.

**Setup path** (`mod.rs`):
```rust
"dual" => {
    let local_token = config.auth.token.clone()
        .unwrap_or_else(|| SecretString::new(generate_random_token()));
    let ledger = Arc::new(TokenLedger::new(store.conn())?);
    let tokens = ledger.mint(INITIAL_POOL_SIZE);
    // ... setup registration with tokens ...
    (AuthProvider::DualAuth { local_token, ledger }, Some(client), Some(creds))
}
```

### 3. Registration extension (`rust/loopflow/src/lfd/registration.rs`)

**Register payload** — add `connection_tokens` array:
```json
{
    "machine_id": "...",
    "machine_name": "...",
    "capabilities": ["waves", "terminal"],
    "url": "http://192.168.1.5:2486",
    "repos": [...],
    "connection_tokens": ["aef3...", "b912...", "c4d1...", "d7e2...", "e8f3..."]
}
```

`RegistrationClient` needs access to the token ledger to mint replenishment tokens.

**Heartbeat request** — add `new_tokens` for replenishment:
```json
{
    "machine_id": "...",
    "url": "...",
    "repos": [...],
    "new_tokens": ["f1a2...", "g3b4..."]
}
```

**Heartbeat response** — parse `tokens_remaining` and optional `revoke`:
```json
{
    "tokens_remaining": 3,
    "revoke": ["aef3"]
}
```

After each heartbeat:
1. If `tokens_remaining < REPLENISH_THRESHOLD`, mint more tokens and include them in the next heartbeat's `new_tokens`.
2. If `revoke` is present, call `ledger.revoke(prefix)` for each entry.

Replenishment is best-effort. If the heartbeat fails, tokens aren't minted — the next successful heartbeat handles it.

### 4. WebSocket re-validation (`rust/loopflow/src/lfd/http/routes/ws.rs`)

The auth token used during HTTP upgrade needs to survive into the WebSocket session. Currently `ws_handler` doesn't capture it.

**Changes:**
1. Extract the bearer token in `ws_handler` (before upgrade) and pass it into `handle_ws`.
2. Add a new arm to the `tokio::select!` loop — a 60-second interval timer.
3. On tick, re-validate the token against the auth provider (same path as middleware).
4. On failure, send close frame with code `4401` and break the loop.

```rust
// In handle_ws, new arm:
_ = revalidation_interval.tick() => {
    if !state.auth.validate(&session_token).await {
        let _ = sender.send(Message::Close(Some(CloseFrame {
            code: 4401,
            reason: "token revoked".into(),
        }))).await;
        break;
    }
}
```

This requires adding a `validate(&self, token: &str) -> bool` method on `AuthProvider` that mirrors what the middleware does (minus the HTTP-specific bits like throttling).

### 5. Desktop toggle (Swift, macOS)

**`ConnectionSettingsView`** — new toggle in the bundled daemon section:

```swift
@AppStorage("mobileAccessEnabled") private var mobileAccessEnabled = false

Toggle("Connect with my phone", isOn: $mobileAccessEnabled)
    .onChange(of: mobileAccessEnabled) { _, enabled in
        Task { await toggleMobileAccess(enabled) }
    }
```

**`toggleMobileAccess` logic:**
1. If enabling: check studio sign-in status. If not signed in, prompt and bail if declined.
2. Call `daemonManager.stop()`.
3. Call `daemonManager.start(mobileAccess: enabled)`.
4. If disabling: also deregister from studio.

**`BundledDaemonManager.start(mobileAccess:)`** — new parameter:

```swift
// Native daemon
if mobileAccess {
    env["LFD_HTTP_ADDR"] = "0.0.0.0:\(port)"
    env["LFD_AUTH_PROVIDER"] = "dual"
    // LFD_AUTH_TOKEN already set — becomes the local_token for DualAuth
} else {
    env["LFD_HTTP_ADDR"] = "127.0.0.1:\(port)"
    env["LFD_AUTH_PROVIDER"] = "static"
}
```

For Docker: same env var changes, but the port mapping changes from `-p "127.0.0.1:\(port):2486"` to `-p "\(port):2486"` (bind all interfaces on host).

### 6. Token revocation

**HTTP endpoint** — `POST /v0/tokens/revoke`:
```json
{ "prefix": "aef3" }        // revoke matching prefix
{ "prefix": "*" }            // revoke all
```

Protected by auth middleware. In DualAuth mode, only the local token can call this endpoint (add a route-level check or a separate middleware). This prevents a connection token holder from revoking other tokens.

**CLI** — `lfq token revoke <prefix>` and `lfq token revoke --all`. Calls the HTTP endpoint with the local token.

**Heartbeat-driven** — already covered in section 3. Studio includes `revoke` array in heartbeat response. lfd processes immediately.

### 7. TLS startup guard (`rust/loopflow/src/bin/lfd.rs`)

After auth setup, before `TcpListener::bind`:

```rust
if !http_addr.ip().is_loopback()
    && !is_tailscale_ip(http_addr.ip())
    && matches!(auth_provider, AuthProvider::DualAuth { .. } | AuthProvider::Studio { .. })
    && !cli_args.allow_insecure_bind
{
    tracing::error!(
        "refusing to bind DualAuth/Studio to {} over plain HTTP. \
         Use --allow-insecure-bind to override.",
        http_addr
    );
    std::process::exit(1);
}
```

Tailscale range: `100.64.0.0/10`. Check with `IpNet` or manual range comparison.

### 8. Container mode default (`rust/loopflow/src/lfd/config.rs`)

In `apply_env_overrides()` or in `resolve()`, after mode is determined:

```rust
if self.mode == Some("container".into()) && self.auth.provider == "local" {
    self.auth.provider = "loopflow.studio".to_string();
}
```

This ensures container mode gets studio auth by default without requiring explicit config.

## Implementation order

Build bottom-up. Each step is testable independently:

1. **Token ledger** — pure Rust, no integration points. Unit tests for mint/validate/revoke/prune lifecycle.
2. **DualAuth variant** — wire into auth middleware. Test with mock ledger.
3. **Registration extension** — add fields to register/heartbeat payloads. Test with mock server.
4. **TLS startup guard** — simple conditional. Test with different addr/auth combinations.
5. **Container mode default** — one-line config change. Test in config unit tests.
6. **WS re-validation** — needs DualAuth working. Integration test with token revocation during active session.
7. **Desktop toggle** — Swift UI + daemon manager changes. Manual verification.
8. **Token revocation endpoint + CLI** — needs ledger + DualAuth. End-to-end test.

## Done when

- `cargo test --all` passes with new tests covering token lifecycle, dual auth dispatch, and WS re-validation
- Desktop toggle restarts lfd with correct env vars (verified via `scripts/concerto-dev.py`)
- Token mint → register → validate → revoke → reject flow works end-to-end
- TLS guard prevents insecure bind (unit test)
- Container mode defaults to studio auth (unit test)
- Existing tests unchanged and passing

### Wave goals advanced

From wave 02 "Done when":
- *Dual auth mode works: static token locally, connection token remotely* — DualAuth variant + auth middleware dispatch
- *Desktop toggle restarts lfd with correct auth + bind configuration* — `mobileAccess` parameter + env var changes
- *lfd mints, pools, and validates connection tokens via local ledger* — TokenLedger module
- *Token revocation works via CLI and heartbeat* — HTTP endpoint + heartbeat revoke array
- *TLS enforcement prevents insecure remote binds* — startup guard
