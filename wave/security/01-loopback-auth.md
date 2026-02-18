# 01: Loopback Auth — Done

Stop treating loopback as proof of identity. A rogue local process, browser extension, or compromised dependency can reach `127.0.0.1:2486` and control all waves.

## What shipped

### Startup token generation

lfd generates a random 32-byte hex token on startup and writes it to `~/.lf/session-token` with `0600` permissions. The token rotates on every daemon restart. Only generated when `AuthProvider::Local` is configured (the default).

Implementation: `session_token::generate_and_write()` in `rust/loopflow/src/lfd/session_token.rs`. Token stored in `HttpState.session_token`.

### Auth tiering

The middleware uses HTTP method to classify requests rather than per-route tagging:

| Tier | Auth required | Classification |
|------|--------------|----------------|
| **Public** | None | Health, metrics, webhooks (outside auth middleware) |
| **Read** | Loopback OR token | `GET`, `HEAD`, `OPTIONS` on protected routes |
| **Mutate** | Token always | `POST`, `PATCH`, `DELETE` on protected routes |

Method-based classification turned out simpler than the planned route-tagging approach. `should_bypass_auth()` and `is_mutation()` in `auth.rs` handle it in two functions.

### Client integration

**Python (lfq)**: `_resolve_token()` checks `LFD_TOKEN` env var first, then reads `~/.lf/session-token` for local base URLs only. Does not send the local token file to non-local servers.

**Swift (Concerto)**: `FileTokenProvider` reads `~/.lf/session-token`. `FileTokenProvider.resolveToken()` cascades: explicit provider → static connection token → local session-token file. Both `WaveService` and `EventService` use this cascade. `applyAuthorization` sends the token on all requests when `authMode.requiresToken`, and also sends it opportunistically for local connections (supports mutations without requiring authMode changes).

### Test coverage

- Rust: `should_bypass_auth`, `authorize_local` (loopback read bypass, loopback mutation forbidden, valid token allowed, remote without token forbidden), token generation/persistence/permissions.
- Python: `_resolve_token` (env precedence, file fallback, missing file, remote non-fallback).
- Swift: `FileTokenProvider` (trimmed reads, missing file, async token resolution).

## Security boundary

This phase prevents:

- A caller that can reach `127.0.0.1:2486` but does **not** have the session token cannot execute mutate routes.
- Browser-originated/local-web requests lose their old "localhost means trusted" mutation path.

This phase does not prevent:

- A rogue process running as the same OS user reading `~/.lf/session-token`.
- A fully compromised host.

## What this doesn't do

- Loopback read bypass applies to all providers, not just Local. Phase 06 removes it for Static/Studio.
- No TLS on local connections — loopback traffic is machine-local.
- No per-wave or per-user authorization — all token holders have full access.
- No rate limiting on auth failures — that's Phase 04.
