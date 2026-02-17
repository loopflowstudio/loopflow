# 01: Loopback Auth

Stop treating loopback as proof of identity. A rogue local process, browser extension, or compromised dependency can reach `127.0.0.1:2486` and control all waves.

## What exists today

`auth_middleware` in `auth.rs:36-41` bypasses auth entirely for loopback connections, regardless of the configured `AuthProvider`:

```rust
if addr.ip().is_loopback() {
    return next.run(request).await;
}
```

This means:
- **Native mode**: Any process on the host can hit every lfd endpoint — create waves, run agents, stop runs, land PRs.
- **Container mode**: lfd binds `0.0.0.0:2486`. Other containers on the Docker bridge network see lfd as non-loopback and need a token. But the lfd container itself gets full access via localhost.
- **Caddy topology**: Caddy reverse-proxies to `lfd:2486`. lfd sees the source as Docker-network, not loopback, so auth applies. But the healthcheck (`curl -sf http://localhost:2486/health`) runs inside the lfd container as loopback — acceptable since `/health` is read-only.

The risk is native mode. OWASP API2 (Broken Authentication) applies: the auth mechanism has a blanket bypass that any co-located process can exploit.

## What exists after this

lfd generates a session token on startup and writes it to a known path. Concerto reads the token from that path. Loopback connections without the token get read-only access (health, status). Mutation routes require the token regardless of source IP.

## Implementation

### Startup token generation

On startup, lfd generates a random 32-byte hex token and writes it to `~/.lf/session-token`. File permissions: `0o600`. The token rotates on every daemon restart.

When `AuthProvider::Local` is configured (the default), this token is the only auth mechanism. When `Static` or `Studio` is configured, the session token is not generated — those providers handle all auth.

### Route classification

Split lfd routes into three tiers:

| Tier | Auth required | Examples |
|------|--------------|---------|
| **Public** | None | `GET /health`, `GET /metrics` |
| **Read** | Loopback OR token | `GET /v0/status`, `GET /v0/waves`, `GET /v0/wave_runs`, `GET /v0/flows`, `GET /v0/worktrees`, `GET /v0/waves/:id/logs`, `GET /v0/ws` |
| **Mutate** | Token always | `POST /v0/waves`, `PUT /v0/waves/:id`, `DELETE /v0/waves/:id`, `POST /v0/waves/:id/run`, `POST /v0/waves/:id/stop`, `POST /v0/waves/:id/land`, `POST /v0/waves/:id/next`, `POST /v0/waves/:id/combine`, `PUT /v0/waves/:id/stimuli`, `POST /v0/hooks/*` |

Rationale: read-only loopback access lets monitoring tools and scripts query status without auth. Anything that creates, modifies, or triggers execution requires the token. This follows the tiered key model used by Supabase (anon/user/service) at a simpler scale.

### Middleware change

```rust
// Before: loopback bypasses everything
if addr.ip().is_loopback() {
    return next.run(request).await;
}

// After: loopback bypasses read-only routes, mutations require token
if addr.ip().is_loopback() && !route_is_mutation(&request) {
    return next.run(request).await;
}
// All mutation routes check token regardless of source
```

### Concerto integration

Concerto reads the session token from `~/.lf/session-token` on launch and when reconnecting. Sends it as `Authorization: Bearer <token>` on all requests. If the file is missing (lfd not running), Concerto shows the existing "lfd not running" state.

This is transparent — no user configuration needed for local mode.

### CLI integration

`lf` CLI commands that talk to lfd (e.g., `lf wave list`) read the session token from the same path. The Python client (`lfq`) reads from `LFD_TOKEN` env var or the file path.

## What this doesn't do

- Doesn't change `Static` or `Studio` auth providers — they already require tokens for all non-loopback requests
- Doesn't add TLS to local connections — loopback traffic is machine-local
- Doesn't add per-wave or per-user authorization — all token holders have full access
- Doesn't add rate limiting on auth failures — that's Phase 04
