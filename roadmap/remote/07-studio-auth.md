# 07: Studio Auth

Real JWT auth via loopflow.studio. Replaces static tokens with proper identity.

## What exists after this

Users sign in with GitHub/Google/Apple via loopflow.studio. lfd validates JWTs locally (no per-request roundtrip). Concerto stores the token in Keychain with automatic refresh.

## What already exists

Most of the client-side auth machinery is built:

| Component | File | Status |
|-----------|------|--------|
| OAuth flow | `AuthService.swift` | Done — `ASWebAuthenticationSession` to `loopflow.studio/auth/login`, callback via `loopflow://auth/callback` |
| Keychain storage | `AuthService.swift` | Done — service `studio.loopflow.auth`, `kSecAttrAccessibleAfterFirstUnlock` |
| Token refresh | `AuthService.swift` | Done — `POST /auth/refresh`, returns new JWT |
| Auth state | `AuthState.swift` | Done — observable, tracks `isAuthenticated`/`isExpired`/`needsRefresh`, auto-refresh monitor (hourly, 24h before expiry) |
| Token provider protocol | `TokenProvider.swift` | Done — `KeychainTokenProvider` and `NoAuthProvider` defined |
| lfd registration | `registration.rs` | Done — registers with studio on startup, heartbeats every 60s |
| lfd auth middleware | `auth.rs` | Done — distinguishes loopback vs non-loopback, validates bearer tokens |

## What's left to build

### loopflow.studio endpoints (studio repo)

The auth endpoints don't exist yet. This is the bulk of the work.

```
GET  /auth/login              → Redirect to OAuth provider (GitHub/Google/Apple)
GET  /auth/callback           → Exchange code, issue JWT, redirect to client
GET  /.well-known/jwks.json   → Public keys for JWT verification
POST /auth/refresh             → Refresh expired JWT
POST /api/v1/daemons/discover → Return user's registered lfd URL
```

### lfd: JWKS validation

Add JWT signature validation to the auth middleware. Short-lived JWTs (5 min) validated locally:

1. lfd fetches JWKS from `loopflow.studio/.well-known/jwks.json` on startup (cached, refreshed hourly)
2. On each request: verify JWT signature against cached JWKS
3. Check claims: `exp`, `aud` (loopflow-lfd), `iss` (loopflow.studio)
4. Check `sub` or `email` against `allowed_users` in config
5. No per-request roundtrip to loopflow.studio

```yaml
# ~/.lf/lfd.yaml
auth:
  provider: loopflow.studio
  allowed_users:
    - user@example.com
  jwks_url: https://loopflow.studio/.well-known/jwks.json
  audience: loopflow-lfd
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

### Concerto: auto-discovery

After sign-in, query studio for the user's lfd:

```
GET /api/v1/daemons/discover
Authorization: Bearer <JWT>

→ { "lfd_url": "https://ec2-host:2486", "last_heartbeat": "..." }
```

This eliminates manual host/port entry from Phase 05. Sign in → auto-discover lfd → connect.

## Deployment scenarios

| Scenario | Auth | Who validates |
|----------|------|--------------|
| Local (default) | None | lfd skips auth for loopback |
| Remote + static token | Bearer token | lfd checks against config |
| Remote + studio auth | JWT | lfd validates signature locally via JWKS |

## Registration (lfd → studio)

lfd registers on startup and sends heartbeats (already implemented in `registration.rs`):

```
POST /api/v1/daemons/register   → { "url": "https://...:2486", "version": "0.7.2" }
POST /api/v1/daemons/heartbeat  → every 60s
```

## Done when

- loopflow.studio serves auth endpoints (login, callback, JWKS, refresh, discover)
- lfd validates JWTs locally via cached JWKS
- `TokenProvider` wired into `LocalWaveService` and `LocalEventService`
- Sign-in UI exists in Concerto
- Concerto auto-discovers lfd URL after sign-in
- `allowed_users` config restricts access
- Static token auth (Phase 03) still works as fallback
