# 02: Discovery Auth Modes

**Finish line:** Mobile connects to lfd using connection tokens handed out by studio. Desktop keeps using static tokens locally. Token mint → distribute → validate → revoke → reject works end-to-end.

## Carried context

- Discovery is shipped: lfd sends `url` + `repos` in register/heartbeat, mobile discovers via studio and connects directly.
- Studio `validate-connection` endpoint exists (studio repo). DualAuth replaces this with local validation — no round-trip on connect.
- `xcodebuild test -scheme Concerto` fails locally during ConcertoUITests link (`open() failed, errno=1`). Swift package tests pass. May be environment-specific.

## Key decisions (from design)

**Single-claim, not single-use.** A token transitions `Available → Claimed` on first use and stays `Claimed`. WebSocket reconnects reuse the same token. Claimed tokens work until expiry (1 hour) or revocation.

**In-memory only.** `RwLock<HashMap>` holds the token pool. Daemon restart mints a fresh pool via registration — no persistence needed. Tokens have 1-hour TTL and the pool replenishes well before exhaustion, so surviving restarts adds complexity for no benefit.

**Loopback restriction on static token.** In DualAuth mode, static token only accepted from loopback. Connection tokens work remotely.

**DualAuth as a fourth `AuthProvider` variant.** Auth dispatch is a match arm, not a conditional chain. Keeps the type system doing the work.

**Store token hashes, not raw tokens.** SHA-256 hash in the `HashMap`. Minting returns raw tokens; only hashes are stored.

**TLS is a startup guard, not a feature.** Refuse to start DualAuth/Studio on non-loopback, non-Tailscale HTTP binds. Actual TLS serving is separate work.

## What to build

1. **Token ledger** (`lfd/token_ledger.rs`) — `TokenLedger` struct with `mint`, `validate`, `revoke`, `revoke_all`, `prune`. In-memory `RwLock<HashMap>`. Prune task every 5 minutes, TTL 1 hour. Pool size 5, replenish threshold 2.

2. **`AuthProvider::DualAuth`** (`lfd/auth.rs`) — new variant `DualAuth { local_token, ledger }`. Loopback + static token match → true. Otherwise → `ledger.validate()`. Config value: `auth.provider: "dual"`.

3. **Registration extension** (`lfd/registration.rs`) — `connection_tokens` array in register payload. `new_tokens` in heartbeat request, `tokens_remaining` and `revoke` in heartbeat response. Replenish when `tokens_remaining < 2`.

4. **WebSocket re-validation** (`lfd/http/routes/ws.rs`) — extract bearer token before upgrade, pass into `handle_ws`. 60-second interval timer in `tokio::select!`. Re-validate token; close with `4401` on failure. Mobile client treats `4401` as "token revoked" — requests a new token from studio and reconnects. New `validate()` method on `AuthProvider`.

5. **Desktop toggle** (Swift, `ConnectionSettingsView`) — "Connect with my phone" toggle. On: check studio sign-in, restart lfd with `LFD_AUTH_PROVIDER=dual` + `LFD_HTTP_ADDR=0.0.0.0:port`. Off: restart with `static` + `127.0.0.1:port`, deregister. Docker: port mapping changes from `127.0.0.1:port:2486` to `port:2486`.

6. **Token revocation** — `POST /v0/tokens/revoke` (local token only). `lfq token revoke <prefix>` and `--all`. Heartbeat-driven revocation via `revoke` array in response.

7. **TLS startup guard** (`bin/lfd.rs`) — refuse non-loopback, non-Tailscale bind over HTTP for DualAuth/Studio. `--allow-insecure-bind` escape hatch. Tailscale range: `100.64.0.0/10`.

8. **Container mode default** (`lfd/config.rs`) — `mode: container` defaults `auth.provider` to `loopflow.studio`.

## Implementation order

Build bottom-up, each step testable independently:

1. Token ledger — pure Rust, unit tests for lifecycle
2. DualAuth variant — wire into auth middleware, test with mock ledger
3. Registration extension — add fields to payloads, test with mock server
4. TLS startup guard — simple conditional, test with addr/auth combinations
5. Container mode default — config unit test
6. WS re-validation — integration test with revocation during active session
7. Desktop toggle — Swift UI + daemon manager, manual verification
8. Token revocation endpoint + CLI — end-to-end test

## Risks

**Studio coordination.** Studio needs pool storage and token handout endpoints. These don't exist yet. lfd-side implementation can proceed independently, but end-to-end testing requires studio work.

**WS re-validation is architecturally new.** No per-session token check exists today. The 60-second interval timer in the WebSocket select loop is a new pattern on a critical path. Needs careful testing.

## Studio-side dependencies

Coordinated but in studio repo:
- Token pool endpoints (receive pool from lfd, hand out on discover)
- Discover rate limiting per user per daemon
- Heartbeat `tokens_remaining` + `revoke` fields

## Out of scope

- Per-repo or per-capability token scoping
- Actual TLS serving (cert loading, config)
- Auto-revoke on suspicious patterns (same token from multiple IPs)
- `advertise_url` config for reverse proxy setups
- Tailscale LocalAPI migration (follow-up from 01)

## Done when

- `cargo test --all` passes with new tests covering token lifecycle, dual auth dispatch, and WS re-validation
- Desktop toggle restarts lfd with correct env vars (verified via `scripts/concerto-dev.py`)
- Token mint → register → validate → revoke → reject flow works end-to-end
- TLS guard prevents insecure bind (unit test)
- Container mode defaults to studio auth (unit test)
- Existing tests unchanged and passing
