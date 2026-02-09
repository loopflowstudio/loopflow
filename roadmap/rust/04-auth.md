# 04: Authentication

Enable remote access to lfd with authentication via loopflow.studio.

## Deployment Scenarios

| Scenario | Who runs lfd | Connection | TLS | Auth |
|----------|-------------|------------|-----|------|
| **Local** | Your Mac | `127.0.0.1` | None | None |
| **Remote self-hosted** | Your EC2/server | Relay via loopflow.studio | loopflow.studio terminates | JWT |
| **Remote loopflow-hosted** | loopflow.studio | Relay via loopflow.studio | loopflow.studio terminates | JWT |

**Local** is the default. Concerto on your Mac talks to lfd on your Mac. No network, no auth, no TLS.

**Remote self-hosted**: lfd maintains an outbound tunnel to loopflow.studio. Mobile requests route through that tunnel. Solves NAT traversal, TLS, and discovery.

**Remote loopflow-hosted**: loopflow.studio runs lfd for you. Same architecture, we just operate it.

## Token Validation

Connection tokens are short-lived JWTs (5 min) validated locally by lfd:

1. loopflow.studio issues connection tokens as signed JWTs
2. lfd fetches JWKS from `loopflow.studio/.well-known/jwks.json` (cached, refreshed hourly)
3. lfd validates JWT signature locally — no roundtrip per request
4. Revocation delayed until token expiry (acceptable for 5 min tokens)

## Architecture

```
lf auth login ──▶ loopflow.studio ──▶ WorkOS AuthKit (Google/GitHub/SSO)
                         │
                         ▼
                  Issue JWT, redirect to CLI callback
                         │
                         ▼
              lfd validates JWT locally (cached JWKS)
              Checks allowed_users config
```

## loopflow.studio Endpoints

```
GET  /auth/login              → Redirect to WorkOS AuthKit
GET  /auth/callback           → Exchange code, issue JWT, redirect to CLI
GET  /.well-known/jwks.json   → Public keys for JWT verification
POST /auth/device/code        → Device authorization flow (headless)
POST /auth/device/token       → Poll for token (device flow)
```

## CLI Auth

`lf auth login` starts a local callback server, opens the browser, waits for the JWT redirect, saves to `~/.lf/credentials.json`.

`lf auth login --device` for headless: displays a code, user visits URL, polls for token.

## lfd JWT Middleware

axum layer that:
1. Extracts `Authorization: Bearer <token>` header
2. Validates JWT signature against cached JWKS
3. Checks claims (exp, aud, iss)
4. Checks user against `allowed_users` config
5. Injects claims into request extensions

Skip auth for `/health` and local-only mode.

### Configuration

```yaml
# ~/.lf/lfd.yaml
auth:
  # Local mode (default)
  provider: local

  # Or: loopflow.studio
  provider: loopflow.studio
  allowed_users:
    - user_abc123
    - user@example.com
  jwks_url: https://loopflow.studio/.well-known/jwks.json
  audience: loopflow-lfd
```

## Done When

- [ ] loopflow.studio auth endpoints work (login, callback, JWKS, device flow)
- [ ] `lf auth login` opens browser, receives and saves token
- [ ] `lf auth login --device` works for headless
- [ ] lfd validates JWTs via axum middleware layer
- [ ] lfd caches JWKS with periodic refresh
- [ ] `allowed_users` config restricts access
- [ ] Local mode skips auth entirely (default)

## Dependencies

- Requires: Phase 1 complete
- Enables: Remote access to lfd
