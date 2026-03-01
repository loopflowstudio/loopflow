# Discovery Auth: Connection Tokens and Dual Auth

## Problem

Mobile connects to lfd through studio-issued connection tokens, but every request requires lfd to round-trip to studio's `validate-connection` endpoint. Studio is a single point of failure for remote access. Local Concerto macOS access uses a separate auth mode, so there's no way to serve both local and remote clients simultaneously.

Users need to flip one switch on their Mac and connect from their phone. The connection should be direct (lfd validates locally), resilient (studio outage doesn't break active sessions or local access), and secure (tokens are short-lived, single-claim, revocable).

## Prerequisites

Tailscale must be running on the desktop Mac so lfd is reachable from the phone. The phone does **not** need Tailscale — it connects over whatever network it has (WiFi, cellular). The desktop toggle UI should surface this requirement clearly.

## Approach

### 1. Token ledger (Rust, `lfd/token_ledger.rs`)

lfd maintains a persisted ledger of connection tokens it has minted, backed by a SQLite table in the existing lfd database (`LFD_DB_PATH`).

```rust
pub struct TokenLedger {
    cache: RwLock<HashMap<String, TokenEntry>>,
    db: SqlitePool,
}

struct TokenEntry {
    created_at: Instant,
    expires_at: Instant,
    state: TokenState,
}

enum TokenState {
    Available,
    Claimed { claimed_at: Instant },
    Revoked,
}
```

The in-memory `cache` is a read-through cache over the `tokens` SQLite table. Validate hits memory first, falls through to DB on miss. Writes go to both.

**Persistence**: on startup, `TokenLedger::new(db)` loads non-expired tokens from the database and prunes expired ones. This means lfd restarts (toggle off/on, crash recovery) preserve the token pool — no need to re-mint or coordinate replacement with studio.

**Schema**:
```sql
CREATE TABLE IF NOT EXISTS tokens (
    token TEXT PRIMARY KEY,
    created_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    state TEXT NOT NULL DEFAULT 'available',
    claimed_at INTEGER
);
```

Token lifecycle:
- **Mint**: `TokenLedger::mint(count) -> Vec<String>` generates 256-bit random tokens (64-char hex), stores as `Available` in both DB and cache, returns for sending to studio.
- **Validate**: `TokenLedger::validate(token) -> bool`. If `Available`, transitions to `Claimed`. If already `Claimed`, allows (single-claim, not single-use). If `Revoked` or unknown, rejects.
- **Revoke**: `TokenLedger::revoke_by_prefix(prefix)` and `revoke_all()`.
- **Prune**: Background task removes expired entries from both DB and cache every 5 minutes.

Constants:
- `TOKEN_TTL`: 1 hour
- `INITIAL_POOL_SIZE`: 5
- `REPLENISH_THRESHOLD`: 2

### 2. `AuthProvider::DualAuth` (Rust, `lfd/auth.rs`)

New enum variant:

```rust
pub enum AuthProvider {
    Local { session_token: SecretString },
    Static { token: SecretString },
    Studio { validator: ConnectionValidator },
    DualAuth {
        local_token: SecretString,
        ledger: Arc<TokenLedger>,
    },
}
```

Authorization logic in `authorize_with_provider`:
1. Try constant-time match against `local_token`. If match AND source is loopback → allow.
2. Try `ledger.validate(token)`. If valid → allow.
3. Reject.

The local token check is restricted to loopback for defense in depth. The token is only ever shared with the co-located Concerto macOS process, but restricting to loopback prevents a leaked token from being usable remotely.

Config value: `auth.provider: "dual"`.

### 3. Registration with token pool (Rust, `lfd/registration.rs`)

When auth provider is `dual`, registration includes the current valid, unclaimed tokens:

```json
{
  "machine_id": "...",
  "machine_name": "...",
  "capabilities": ["waves", "terminal"],
  "url": "http://100.64.1.5:2486",
  "repos": [...],
  "connection_tokens": ["aabbcc...", "ddeeff..."]
}
```

Since the ledger is persisted, a restarted lfd re-registers with the same tokens — idempotent, no harm. If some expired since last run, lfd mints fresh ones to fill the pool.

Heartbeat gains two fields:
- **Request**: `"new_tokens": [...]` — replenishment tokens when studio's pool is low.
- **Response**: `"tokens_remaining": 3` — how many unused tokens studio still has. `"revoke": ["prefix1"]` — tokens studio wants revoked (compromised user, account deactivation).

Replenishment is best-effort: after each heartbeat, if `tokens_remaining < REPLENISH_THRESHOLD`, mint `INITIAL_POOL_SIZE - tokens_remaining` new tokens and include in next heartbeat. Over-minting by 1-2 tokens between heartbeats is harmless — they expire in an hour.

### 4. WebSocket re-validation

Token is validated on the HTTP upgrade request (existing auth middleware). Active WebSocket sessions re-validate every 60 seconds on a background timer.

On re-validation failure (token revoked or expired):
- Send WS close frame with code `4401` (custom close code indicating auth revocation).
- Drop the connection.
- Phone handles `4401` by triggering re-discover.

This means a revoked token has up to 60 seconds of remaining access on an existing WS — acceptable for v1.

### 5. Desktop toggle: "Connect with my phone" (Swift, macOS)

New toggle in `ConnectionSettingsView`, off by default. Persisted in `UserDefaults` key `concerto.mobileAccess.enabled`.

**Toggle on:**
1. Check studio sign-in status via `AuthService`. Prompt sign-in if needed.
2. Stop current bundled lfd.
3. Restart lfd with:
   - `LFD_HTTP_ADDR=0.0.0.0:{port}` (bind all interfaces)
   - `LFD_AUTH_PROVIDER=dual`
   - `LFD_AUTH_TOKEN={local_token}` (same random token, for local access)
   - Studio JWT available via credential socket for registration.
4. Wait for health check.
5. Reconnect Concerto macOS using local token.

**Toggle off:**
1. Stop current lfd.
2. Restart with `LFD_HTTP_ADDR=127.0.0.1:{port}`, `LFD_AUTH_PROVIDER=static`.
3. lfd deregisters from studio on shutdown (existing behavior).
4. Reconnect.

The toggle triggers `BundledDaemonManager.restart(mobileAccess: Bool)` which stops and starts with the appropriate env vars.

### 6. Token revocation (Rust + Python)

Two revocation paths:

**CLI**: `lfq token revoke <prefix>` and `lfq token revoke --all`. Calls `POST /v0/tokens/revoke` with `{ "prefix": "abc" }` or `{ "all": true }`. Authenticated with the local token.

**Heartbeat**: Studio includes `"revoke": ["prefix1"]` in heartbeat response. lfd processes immediately.

New HTTP endpoint: `POST /v0/tokens/revoke` (auth-protected, local token only).

### 7. TLS enforcement (Rust, `lfd/mod.rs`)

At startup, when auth provider is `dual` or `studio`:
- If bind address is non-loopback and not a Tailscale IP → check for TLS config.
- If no TLS configured and no `advertise_url` set → refuse to start with clear error message.
- `--allow-insecure-bind` flag overrides the check (for development).
- Tailscale IPs (100.64.0.0/10) are exempt because Tailscale provides its own encryption.

Detection: reuse existing `detect_lfd_url` which already checks Tailscale status.

### 8. Container mode default

In `ModeProfile::for_mode`, when mode is `Container`, default `auth.provider` to `"loopflow.studio"` unless explicitly overridden. Single line change in config resolution.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| JWT tokens signed by lfd key pair | Stateless validation, natural user identity | Revocation needs CRL/blocklist. Key rotation adds complexity. No user identity needed yet — we're authenticating devices, not users. |
| Studio-minted, lfd-cached tokens | Studio stays the authority | Still needs sync. Cache invalidation is a source of bugs. Doesn't remove studio as validation dependency. |
| Keep current validate-connection round-trip | No new code | 60-second cache means stale auth. Studio outage blocks new connections. P95 latency includes network hop. |
| mTLS with client certificates | Strong device identity | Certificate management is heavy. Doesn't work well with mobile (cert provisioning UX). Overkill for this stage. |
| IP-bound tokens (original design) | Catches token sharing/theft | Breaks on WiFi→cellular handoff. Would require Tailscale on the phone, which is a bigger UX ask than Tailscale on desktop. |

## Key decisions

**Single-claim, not single-use.** A connection token transitions to `Claimed` on first use. Subsequent requests reuse it regardless of source IP. This avoids burning through the pool on WebSocket reconnects and multiple HTTP requests, and allows the phone to change networks freely.

**No IP binding.** Security relies on 256-bit unguessable tokens, HTTPS distribution from studio, 1-hour TTL, and rate-limited discover — not source IP. This removes the Tailscale-on-phone requirement. Tailscale is only needed on the desktop for reachability.

**Persisted ledger.** SQLite table in the existing lfd database. Survives restarts (toggle off/on, crash recovery). Re-registration sends the same tokens — idempotent. Studio doesn't need "replace pool" semantics; tokens expire naturally via TTL.

**lfd mints tokens, not studio.** lfd controls its own security boundary. Studio is a distribution mechanism, not an authority. This means lfd works even if studio's token logic has bugs.

**Local token restricted to loopback.** Even though the local token is a 256-bit secret that never leaves the machine, restricting to loopback adds defense in depth. If the token leaks (e.g., via a debug log), it's still unusable from the network.

**Pool size of 5.** Covers typical use: 1-2 phones + reconnections. Replenishes at 2 remaining. Minting is cheap (just random bytes). No reason to over-provision.

**1-hour TTL.** Long enough for a work session. Short enough to limit exposure. Mobile can re-discover to get a fresh token.

**WS re-validation every 60 seconds.** Balances security (revoked tokens can't persist indefinitely) against performance (no per-frame lookup). Up to 60 seconds of residual access after revocation — acceptable for v1.

**No daemon restart for toggle — just stop and start.** Concerto already manages lfd as a child process with clean start/stop. Hot-reloading bind address would require socket gymnastics. Stop/start is ~2 seconds and dead simple.

**Heartbeat replenishment is best-effort.** Over-minting by 1-2 tokens between heartbeats is harmless — they expire in an hour. No need to engineer exact synchronization.

## Scope

**In scope:**
- `AuthProvider::DualAuth` with persisted token ledger
- Token minting, validation, claiming, expiry, pruning (SQLite-backed)
- Registration and heartbeat with token pool
- WebSocket re-validation on 60-second timer
- Desktop toggle in Concerto macOS
- Token revocation (CLI, heartbeat)
- TLS enforcement at startup
- Container mode auth default
- Tests for token ledger, dual auth, pool lifecycle

**Out of scope:**
- Per-repo or per-capability token scoping (v1 constraint)
- Migrating Tailscale detection from shell-out to LocalAPI (separate follow-up)
- Studio-side endpoints (separate repo — token pool receive, discover handout, heartbeat response fields)
- Mobile-side changes (mobile already uses connection tokens from discover; the token format doesn't change)

## Studio-side dependencies

These must be coordinated but live in the studio repo:

1. **Register endpoint** accepts `connection_tokens` array.
2. **Discover endpoint** hands out one token per daemon per request (not the full pool).
3. **Heartbeat request** accepts `new_tokens` for replenishment.
4. **Heartbeat response** returns `tokens_remaining` and optional `revoke` array.
5. **Rate limiting**: max 10 discover requests per user per daemon per minute.
6. **Token expiry**: studio discards tokens older than `TOKEN_TTL` (1 hour). No explicit "replace pool" call needed.

## Done when

- `cargo test` passes with new tests covering: token ledger (mint, validate, claim, expire, revoke), dual auth (loopback local token, remote connection token, rejection), pool replenishment logic.
- `swift test --package-path swift` passes.
- Desktop toggle restarts lfd with correct auth mode and bind address.
- `lfq token revoke <prefix>` revokes tokens via HTTP endpoint.
- TLS enforcement prevents `dual` auth on non-loopback, non-Tailscale HTTP binds.
- Existing tests unchanged — dual auth is additive, not a refactor.

Advances wave goals: *"Dual auth mode works: static token locally, connection token remotely"* and *"lfd mints, pools, and validates connection tokens via local ledger."*
