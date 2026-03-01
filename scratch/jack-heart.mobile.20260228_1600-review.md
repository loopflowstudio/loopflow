# Review: Discovery Auth Modes (02)

## What was implemented

Connection token system for mobile-to-desktop lfd authentication. Desktop keeps using static tokens locally; mobile uses connection tokens minted by lfd and distributed via studio.

**Token lifecycle:** `TokenLedger` (SQLite-backed, in-memory cache) mints opaque tokens, stores SHA-256 hashes, validates with single-claim semantics (Available → Claimed on first use, stays Claimed until expiry/revocation). 1-hour TTL, 5-minute prune interval, pool of 5 with replenish threshold of 2.

**Auth dispatch:** New `AuthProvider::DualAuth` variant. Loopback + static token → local admin. Otherwise → ledger validation. Clean match arm, not conditional chain.

**Registration extension:** `connection_tokens` array in register payload, `new_tokens` in heartbeat request, `tokens_remaining` + `revoke` in heartbeat response. Replenishment is demand-driven: studio signals low pool via `tokens_remaining`, lfd mints on next heartbeat.

**WebSocket re-validation:** 60-second interval re-checks bearer token. Closes with code `4401` on failure (revoked/expired token).

**Desktop toggle:** "Connect with my phone" in ConnectionSettingsView. Restarts lfd with `dual` auth + `0.0.0.0` bind (or reverts to `static` + `127.0.0.1`). Requires studio sign-in.

**Token revocation:** `POST /v0/tokens/revoke` (local admin only). CLI via `lfq token revoke <prefix>` and `--all`. Heartbeat-driven revocation via `revoke` array.

**TLS startup guard:** Refuses non-loopback, non-Tailscale HTTP bind for dual/studio auth. `--allow-insecure-bind` escape hatch.

## Key choices

| Decision | Why | Alternative rejected |
|----------|-----|---------------------|
| Single-claim, not single-use | WebSocket reconnects reuse the same token; single-use would force new token acquisition on every reconnect | Single-use (simpler but poor UX on flaky connections) |
| In-memory + SQLite sidecar | Fast validation without network round-trip; survives daemon restart via SQLite reload | Postgres table (couples to storage backend), pure in-memory (loses state on restart) |
| SHA-256 hashes stored, not raw tokens | Raw tokens never persisted; compromise of ledger DB doesn't leak usable tokens | Storing raw tokens (simpler but worse security posture) |
| `HeartbeatResponse` parsing is best-effort | Existing studio heartbeat may return empty/non-JSON body; hard failure would break `AuthProvider::Studio` registration | Required JSON parsing (would regress studio auth) |
| `--allow-insecure-bind` passed from BundledDaemonManager | User explicitly opted in via toggle; the flag makes intent explicit in process args | Env var override (less visible), skip TLS guard for unspecified addr (too permissive) |

## How it fits together

```
Desktop (Concerto)                    Studio                    Mobile
    │                                   │                         │
    │ toggle "Connect with my phone"    │                         │
    │──restart lfd(dual, 0.0.0.0)──►    │                         │
    │                                   │                         │
    │ register(jwt, connection_tokens)  │                         │
    │──────────────────────────────────►│                         │
    │                                   │ discover(user)          │
    │                                   │◄────────────────────────│
    │                                   │ {url, token}            │
    │                                   │────────────────────────►│
    │                                   │                         │
    │ WS connect(Bearer: connection_token)                        │
    │◄────────────────────────────────────────────────────────────│
    │ re-validate every 60s             │                         │
```

Auth middleware dispatches on `AuthProvider` variant. `DualAuth` checks loopback+static first, falls through to ledger. Token revocation endpoint is admin-only (loopback + static token). Heartbeat carries revocation commands from studio.

## Risks and bottlenecks

- **Studio coordination required for end-to-end.** lfd mints and sends tokens, but studio needs pool storage and token handout endpoints. Those don't exist yet. lfd-side works independently but the full flow can't be tested without studio.
- **WS re-validation is a new pattern.** No per-session token check existed before. The 60-second interval in the WebSocket select loop is on a critical path. If `validate()` takes too long (e.g., SQLite contention), it could stall the event stream briefly.
- **SQLite sidecar for Postgres deployments.** When main storage is Postgres, the token ledger still uses `~/.lf/connection_tokens.db`. This is a second data store to manage. Acceptable for v1 since tokens are ephemeral (1-hour TTL, replenished on heartbeat).
- **`0.0.0.0` bind exposes lfd on all interfaces.** The TLS guard + `--allow-insecure-bind` flag provide the safety net, but there's no actual TLS serving yet — that's future work.

## What's not included

- TLS certificate loading and serving (separate work)
- Per-repo or per-capability token scoping
- Auto-revoke on suspicious patterns (same token from multiple IPs)
- `advertise_url` config for reverse proxy setups
- Tailscale LocalAPI migration (follow-up from 01)
- Studio-side token pool endpoints (studio repo)

## Gate fixes applied

1. **BundledDaemonManager missing `--allow-insecure-bind`** — lfd would refuse to start when "Connect with my phone" was enabled because the TLS guard rejected `0.0.0.0` bind for `dual` auth. Fixed in both native and container daemon paths.
2. **`send_heartbeat` JSON parsing regression** — changed from hard error to best-effort parsing so existing studio auth (which may return empty heartbeat responses) isn't broken.
3. **`tokens.rs` repeated full crate path** — replaced with `use` import per style guide.
