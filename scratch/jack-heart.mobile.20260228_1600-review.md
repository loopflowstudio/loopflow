# Review: Discovery Auth Design Doc

## What was implemented

Design document for dual auth mode — letting lfd serve both local Concerto macOS (static token, loopback) and remote mobile clients (connection tokens, Tailscale) simultaneously. No code changes; this PR is the design artifact only.

The design covers: token ledger with SQLite persistence, `AuthProvider::DualAuth` variant, registration/heartbeat token pool management, WebSocket re-validation, desktop toggle UI, token revocation, TLS enforcement, and container mode auth default.

## Key choices

- **lfd mints tokens, not studio.** lfd controls its own security boundary. Studio distributes tokens but never validates them. This removes studio as a runtime dependency for active sessions.

- **Single-claim, not single-use.** A token transitions to `Claimed` on first use and stays valid until expiry. Avoids burning tokens on WS reconnects and HTTP retries. Trades theoretical token-sharing risk for practical robustness on mobile networks.

- **No IP binding.** Security comes from 256-bit unguessable tokens + 1-hour TTL + rate-limited discover, not source IP. This avoids requiring Tailscale on the phone (only desktop needs it for reachability).

- **Persisted ledger via shared SQLite connection.** Reuses lfd's existing `Arc<Mutex<Connection>>` with a new `tokens` table. Survives daemon restarts without re-minting or coordinating with studio.

## How it fits together

```
Phone → discover (studio) → gets one connection token
Phone → lfd (via Tailscale IP) → Bearer <connection_token>
lfd → TokenLedger.validate() → local, no network hop
Concerto macOS → lfd (loopback) → Bearer <local_token>
```

DualAuth tries the local token (loopback only) first, then the ledger. Both paths are local. Studio involvement is limited to initial registration and token distribution.

## Corrections made during gate

Three inaccuracies fixed against the actual codebase:

1. **`SqlitePool` → `Arc<Mutex<Connection>>`** — lfd uses `rusqlite`, not `sqlx`. The token ledger shares the existing connection.
2. **`BundledDaemonManager.restart()` doesn't exist** — replaced with `stop()` + `start(mobileAccess:)` which matches the existing API surface.
3. **`ModeProfile::for_mode` has no auth field** — container mode auth default goes through config loading path alongside `LFD_AUTH_PROVIDER` parsing, not through `ModeProfile`.

## Risks and bottlenecks

- **Single `Arc<Mutex<Connection>>` contention.** Token validation acquires the mutex on cache miss. Under normal load (5 tokens, infrequent cache misses) this is fine. Under pathological load (rate-limited auth failures hammering the DB) the mutex could slow other store operations. The in-memory cache mitigates this — validate hits memory first.

- **Studio coordination is out of scope.** This design requires studio-side changes (accept `connection_tokens` in register, hand out one per discover request, return `tokens_remaining` in heartbeat). Those endpoints don't exist yet. The designs need to ship together for the feature to work end-to-end.

- **No TLS infrastructure exists.** Section 7 describes TLS enforcement but the codebase has no TLS config parsing, no cert loading, no `rustls`/`native-tls` dependency. The enforcement check (refuse to start without TLS on non-Tailscale, non-loopback bind) is straightforward, but adding actual TLS serving is a separate effort. The Tailscale exemption makes this acceptable for v1.

- **WS re-validation is new territory.** Currently no per-session validation exists after the HTTP upgrade. Adding a 60-second background timer per WS session is architecturally new. The design is clear on the approach but implementation will need careful integration with the existing WS handler.

## What's not included

- No code changes — design doc only
- Studio-side endpoints (separate repo)
- Mobile-side changes (mobile already consumes connection tokens from discover)
- Per-repo or per-capability token scoping
- Actual TLS serving (only the startup guard that refuses insecure non-Tailscale binds)
