# Security Loopback Auth — Design Review

## What was implemented

Session-token auth for `lfd` local mode. Loopback connections no longer bypass auth for mutations — a random 32-byte hex token is generated on startup, written to `~/.lf/session-token` (0600), and required on all `POST`/`PATCH`/`DELETE` requests. Loopback reads remain unauthenticated. All three clients (Python, Swift, Rust binary) discover the token automatically.

## Key choices

**Method-based classification over route tagging.** `is_mutation()` checks the HTTP method rather than maintaining a route allowlist. Simpler, no maintenance burden when routes are added. The trade-off is that a future read route using POST would be misclassified — acceptable since lfd follows REST conventions.

**Token generated at startup, not static config.** Rotates on every daemon restart. No user configuration needed for local mode. Simpler UX than requiring users to set up tokens manually.

**Local-only file fallback in clients.** Both Python and Swift clients read `~/.lf/session-token` only when the base URL is local. This prevents accidentally sending local credentials to remote servers. `LFD_TOKEN` env var always takes precedence.

**Opportunistic token send in Swift.** `applyAuthorization` sends the token on all local requests (not just when `authMode.requiresToken`), so mutations work without requiring authMode changes in the connection model.

## How it fits together

```
lfd startup (Local mode)
  └─ generate_and_write() → ~/.lf/session-token (0600)
  └─ session_token stored in HttpState

Request arrives → auth_middleware
  ├─ loopback + GET/HEAD/OPTIONS → bypass (should_bypass_auth)
  └─ everything else → match on AuthProvider
       ├─ Local → authorize_local(session_token, provided, is_loopback)
       ├─ Static → constant-time compare against config token
       └─ Studio → validator.validate() against loopflow.studio

Clients (Python/Swift)
  └─ resolve token: explicit > LFD_TOKEN env > ~/.lf/session-token (local only)
```

## Risks and bottlenecks

**Same-user read access.** Any process running as the same OS user can read `~/.lf/session-token`. This is documented and accepted — OS-level user isolation is the boundary, not file-based secrets.

**Token rotation on restart.** If a client caches the old token and lfd restarts, requests fail until the client re-reads the file. Both Swift and Python read at init time, so creating a new client after restart picks up the new token. Long-lived clients would need to handle 401/403 by re-reading.

**Loopback read bypass applies to all providers.** Phase 06 will scope the bypass to `Local` only. Currently a co-located process can read wave status without auth even when `Static`/`Studio` is configured. Documented in `06-auth-provider-isolation.md`.

## What's not included

- WebSocket message-level auth (HTTP handshake only).
- Auth-failure rate limiting or body-size limits (Phase 04).
- TLS/mTLS for local connections.
- Per-provider loopback isolation (Phase 06).
- Manual Concerto verification against a running `lfd`.
