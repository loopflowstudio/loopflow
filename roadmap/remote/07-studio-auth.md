# 07: Studio Auth

Real JWT auth via auth.loopflow.studio. Replaces static tokens with proper identity.

## What exists after this

Users sign in via WorkOS (GitHub/Google/Apple) through auth.loopflow.studio. lfd validates JWTs locally (no per-request roundtrip). Concerto stores the token in Keychain with automatic refresh. CLI authenticates via device code flow.

## What already exists

### Client-side (Concerto + lfd)

| Component | File | Status |
|-----------|------|--------|
| OAuth flow | `AuthService.swift` | Done — `ASWebAuthenticationSession` to `auth.loopflow.studio/auth/login`, callback via `loopflow://auth/callback` |
| Keychain storage | `AuthService.swift` | Done — service `studio.loopflow.auth`, `kSecAttrAccessibleAfterFirstUnlock` |
| Token refresh | `AuthService.swift` | Done — `POST /auth/refresh`, returns new JWT |
| Auth state | `AuthState.swift` | Done — observable, tracks `isAuthenticated`/`isExpired`/`needsRefresh`, auto-refresh monitor (hourly, 24h before expiry) |
| Token provider protocol | `TokenProvider.swift` | Done — `KeychainTokenProvider` and `NoAuthProvider` defined |
| lfd registration | `registration.rs` | Done — registers with studio on startup, heartbeats every 60s |
| lfd auth middleware | `auth.rs` | Done — `AuthProvider` enum (`Local`, `Static`, `Studio`), loopback bypass, constant-time token comparison |

### Server-side (auth.loopflow.studio)

The auth server is built. It's a FastAPI service backed by SQLModel + SQLAlchemy (SQLite dev, Postgres production), using WorkOS for OAuth and RS256 JWTs.

#### Auth endpoints

```
POST /auth/device              → Start device code flow (for CLI)
POST /auth/device/token        → Poll for token (device flow)
GET  /cli/login?code={code}    → Web UI for device login
GET  /auth/login               → Redirect to WorkOS OAuth
GET  /auth/callback            → Exchange code, issue JWT, redirect
POST /auth/refresh             → Refresh JWT (Bearer required)
GET  /.well-known/jwks.json    → RSA public key for token verification
```

#### Daemon management endpoints (all require Bearer JWT)

```
POST /api/v1/daemons/register    → Register daemon instance
POST /api/v1/daemons/heartbeat   → Keep-alive
POST /api/v1/daemons/deregister  → Remove daemon
GET  /api/v1/daemons/discover    → List user's daemons
```

#### Device code flow

For CLI/daemon authentication where a browser isn't directly available:

```
POST /auth/device
→ {
    "device_code": "...",
    "user_code": "XXXX-XXXX",
    "verification_uri": "https://auth.loopflow.studio/cli/login?code=XXXX-XXXX",
    "expires_in": 900
  }
```

Client displays user_code and verification_uri. User authenticates in browser. Client polls:

```
POST /auth/device/token
{ "device_code": "..." }

→ { "error": "authorization_pending" }   // still waiting
→ { "token": "eyJ..." }                  // done
→ { "error": "expired_token" }           // TTL exceeded (15 min default)
```

#### Daemon registration

```
POST /api/v1/daemons/register
Authorization: Bearer <JWT>
{
    "machine_id": "unique-machine-id",
    "machine_name": "Studio Mac",          // optional
    "capabilities": ["discover", "relay"],  // optional, default []
    "url": "https://ec2-host:2486"         // optional
}
→ { "status": "registered" }
```

`machine_id` is unique globally — returns 403 if already claimed by a different user.

#### Daemon discovery

```
GET /api/v1/daemons/discover
Authorization: Bearer <JWT>

→ {
    "daemons": [
      {
        "machine_id": "...",
        "machine_name": "Studio Mac",
        "url": "https://ec2-host:2486",
        "last_heartbeat_at": "2026-02-13T..."
      }
    ]
  }
```

Returns all daemons owned by the authenticated user. Concerto uses this to offer a picker when the user has multiple machines.

#### JWT structure

RS256-signed, 7-day lifetime, 24h refresh window:

```json
{
  "sub": "user_123",
  "email": "user@example.com",
  "name": "User Name",
  "iss": "auth.loopflow.studio",
  "aud": "loopflow-lfd",
  "iat": 1739000000,
  "exp": 1739604800
}
```

Public key served at `/.well-known/jwks.json` for local validation.

## What's left to build

### lfd: JWKS validation

Add JWT signature validation to the auth middleware. 7-day JWTs validated locally:

1. lfd fetches JWKS from `auth.loopflow.studio/.well-known/jwks.json` on startup (cached, refreshed hourly)
2. On each request: verify JWT signature against cached JWKS
3. Check claims: `exp`, `aud` (`loopflow-lfd`), `iss` (`auth.loopflow.studio`)
4. Check `sub` or `email` against `allowed_users` in config
5. No per-request roundtrip to auth.loopflow.studio

The `setup_auth()` function in `lfd/mod.rs` already dispatches on `config.auth.provider` — the `"loopflow.studio"` branch calls `setup_studio_registration()`. JWKS validation extends this path.

```yaml
# ~/.lf/lfd.yaml
auth:
  provider: loopflow.studio
  allowed_users:
    - user@example.com
  jwks_url: https://auth.loopflow.studio/.well-known/jwks.json
  audience: loopflow-lfd
```

### lfd: update registration payload

Registration currently sends `url` and `version`. Update to match auth server's contract:

```rust
// registration.rs — update payload
DaemonRegisterRequest {
    machine_id: String,      // stable machine identifier
    machine_name: Option<String>,
    capabilities: Vec<String>,
    url: Option<String>,
}
```

Heartbeat and deregister use `machine_id` only:

```rust
DaemonMachineRequest {
    machine_id: String,
}
```

### Concerto: wire TokenProvider into requests

`TokenProvider` protocol exists but isn't integrated into `LocalWaveService`. Wire it in:

```swift
// LocalWaveService — inject TokenProvider
private let tokenProvider: TokenProvider

private func makeRequest(_ url: URL, method: String = "GET") -> URLRequest {
    var request = URLRequest(url: url)
    request.httpMethod = method
    if let token = try? await tokenProvider.token() {
        request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
    }
    return request
}
```

When `provider` is `loopflow.studio`, use `KeychainTokenProvider`. When static token, use the token from `ServerConnection`. When local, use `NoAuthProvider`.

### Concerto: sign-in UI

No auth UI exists. Add a sign-in view:

- Sign-in button in connection settings or welcome screen
- Calls `AuthState.signIn()` which opens `ASWebAuthenticationSession`
- On success, auto-discover lfd via studio (replaces manual host/port entry)
- Show loading/error states

### Concerto: auto-discovery with daemon picker

After sign-in, query studio for the user's daemons:

```
GET /api/v1/daemons/discover
Authorization: Bearer <JWT>

→ { "daemons": [ { "machine_id": "...", "machine_name": "...", "url": "...", "last_heartbeat_at": "..." } ] }
```

Discovery returns a list — the user may have multiple machines. Concerto shows a picker when more than one daemon is available. Single daemon auto-connects.

This eliminates manual host/port entry from Phase 05. Sign in → discover daemons → pick machine → connect.

### CLI: device code auth

Wire the device code flow into `lf` for CLI-based authentication:

1. `lf auth login` calls `POST /auth/device`
2. Display user_code and open verification_uri in browser
3. Poll `POST /auth/device/token` with backoff
4. Store JWT in `~/.lf/credentials` or OS keychain
5. Use JWT for lfd registration and API calls

## Deployment scenarios

Maps to `AuthProvider` enum variants:

| Scenario | `auth.provider` | `AuthProvider` variant | Who validates |
|----------|----------------|----------------------|--------------|
| Local (default) | `local` | `Local` | Loopback bypass; non-loopback gets 403 |
| Remote + static token | `static` | `Static { token }` | Constant-time comparison against config token |
| Remote + studio auth | `loopflow.studio` | `Studio { validator }` | JWKS validation (this phase) |

## Done when

- lfd validates JWTs locally via cached JWKS from `auth.loopflow.studio`
- lfd registration sends `machine_id`, `machine_name`, `capabilities`, `url`
- `TokenProvider` wired into `LocalWaveService` and `LocalEventService`
- Sign-in UI exists in Concerto
- Concerto auto-discovers daemons after sign-in (with picker for multiple)
- CLI authenticates via device code flow (`lf auth login`)
- `allowed_users` config restricts access
- Static token auth (Phase 03) still works as fallback
