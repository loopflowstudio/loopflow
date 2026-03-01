# 02: Discovery Auth Modes

**Finish line:** Dual auth mode works — static token locally (loopback), connection tokens remotely. Desktop toggle flips between modes. Design doc reviewed and corrected against the codebase.

## Carried context

- Discovery is shipped: lfd sends `url` + `repos` in register/heartbeat, mobile discovers via studio and connects directly.
- Address detection currently shells out to `tailscale status --json`. Follow-up should migrate to Tailscale LocalAPI for reliability.
- `xcodebuild test -scheme Concerto` fails locally during ConcertoUITests link (`open() failed, errno=1`). Swift package tests pass. May be environment-specific.
- Design doc written and reviewed (`scratch/mobile-discovery-auth.md`). Three codebase corrections applied during gate:
  - lfd uses `rusqlite` with `Arc<Mutex<Connection>>`, not `sqlx`/`SqlitePool`. Token ledger shares the existing connection.
  - `BundledDaemonManager` has `stop()` + `start()`, not `restart()`. Toggle calls stop then start with `mobileAccess: Bool` parameter.
  - `ModeProfile::for_mode` has no auth field. Container mode auth default goes through config loading path alongside `LFD_AUTH_PROVIDER` parsing.

## What to build

1. **Token ledger** (`lfd/token_ledger.rs`)
   - `TokenLedger` with `RwLock<HashMap>` cache over SQLite `tokens` table.
   - Mint: 256-bit random tokens (64-char hex), stored as `Available`.
   - Validate: `Available` → `Claimed` on first use. Already `Claimed` → allow (single-claim, not single-use — avoids burning tokens on WS reconnects). `Revoked`/unknown → reject.
   - Prune expired entries every 5 minutes. Constants: `TOKEN_TTL` 1 hour, `INITIAL_POOL_SIZE` 5, `REPLENISH_THRESHOLD` 2.

2. **`AuthProvider::DualAuth`** (`lfd/auth.rs`)
   - New enum variant: `DualAuth { local_token: SecretString, ledger: Arc<TokenLedger> }`.
   - Auth logic: constant-time match against `local_token` (loopback only) → ledger validate → reject.
   - Config value: `auth.provider: "dual"`.

3. **Registration with token pool** (`lfd/registration.rs`)
   - Include `connection_tokens` array in register payload.
   - Heartbeat request: `new_tokens` for replenishment. Response: `tokens_remaining`, optional `revoke` array.
   - Replenishment is best-effort after each heartbeat.

4. **WebSocket re-validation**
   - 60-second background timer per WS session. On failure: close frame `4401`, drop connection.
   - Note: no per-session validation exists today — this is architecturally new.

5. **Desktop toggle: "Connect with my phone"** (Swift, macOS)
   - Setting in `ConnectionSettingsView`, off by default. Persisted in UserDefaults.
   - On: check studio sign-in, stop lfd, start with `0.0.0.0` bind + dual auth + local token.
   - Off: stop lfd, start with `127.0.0.1` bind + static auth, deregister from studio.

6. **Token revocation**
   - CLI: `lfq token revoke <prefix>` and `--all`. HTTP endpoint `POST /v0/tokens/revoke` (local token auth only).
   - Heartbeat: studio includes `revoke` array, lfd processes immediately.

7. **TLS enforcement** (`lfd/mod.rs`)
   - Refuse dual/studio auth on non-loopback, non-Tailscale HTTP bind.
   - Tailscale IPs (100.64.0.0/10) exempt. `--allow-insecure-bind` escape hatch.
   - Note: no TLS config parsing or cert loading exists in the codebase yet. The startup guard is straightforward; actual TLS serving is separate.

8. **Container mode default**
   - `mode: container` defaults `auth.provider` to `loopflow.studio` in config loading path.

## Risks

- **Mutex contention.** Token validation acquires `Arc<Mutex<Connection>>` on cache miss. In-memory cache mitigates this under normal load (5 tokens). Pathological auth-failure hammering could slow other store operations.
- **Studio coordination.** Studio-side endpoints (token pool receive, discover handout, heartbeat fields) don't exist yet. Both sides must ship together for end-to-end flow.
- **WS re-validation is new.** No per-session validation exists after HTTP upgrade. Integration with existing WS handler needs care.

## Studio-side dependencies

Coordinated but in the studio repo:
- Register endpoint accepts `connection_tokens` array.
- Discover endpoint hands out one token per daemon per request (not the full pool).
- Heartbeat request accepts `new_tokens`. Response returns `tokens_remaining` + optional `revoke`.
- Rate limiting: max 10 discover requests per user per daemon per minute.

## Constraints

- Discovery remains additive. Manual connection always available.
- Local static-token connections unaffected by studio outage.
- No per-repo or per-capability token scoping in v1.
- No IP binding — security relies on 256-bit unguessable tokens + 1-hour TTL + rate-limited discover.

## Done when

- Dual auth mode works: static token locally, connection token remotely.
- Desktop toggle restarts lfd with correct auth + bind configuration.
- lfd mints, pools, and validates connection tokens via local ledger.
- Token revocation works via CLI and heartbeat.
- TLS enforcement prevents insecure remote binds.
- Existing tests pass, new tests cover token lifecycle and dual auth.
