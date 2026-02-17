# 01: Loopback Auth

Stop treating loopback as proof of identity. A rogue local process, browser extension, or compromised dependency can reach `127.0.0.1:2486` and control all waves.

## Problem

`auth_middleware` in `auth.rs:36-41` bypasses auth entirely for loopback connections:

```rust
if addr.ip().is_loopback() {
    return next.run(request).await;
}
```

Any process on the host can create waves, run agents, stop runs, land PRs. No credentials needed. OWASP API2 (Broken Authentication) — a blanket loopback bypass lets co-located processes execute privileged actions.

The highest risk is native mode (macOS, `127.0.0.1:2486`), but loopback trust is fragile in any topology. Browser extensions, compromised npm packages, and malicious repos can all reach localhost.

## Approach

Three changes, all in lfd's Rust code:

1. **Generate a session token on startup** — 32 random bytes, hex-encoded, written to `~/.lf/session-token` with `0o600` permissions. Rotates every daemon restart.

2. **Classify routes into tiers** — public (no auth), read (loopback OR token), mutate (token always). The middleware checks HTTP method + path to decide.

3. **AuthProvider::Local learns to validate tokens** — today `Local` means "loopback only, reject remote." After this, `Local` also accepts the session token from any source. This is how Concerto and lfq authenticate locally.

Client changes are minimal:
- **Concerto**: add a `FileTokenProvider` that reads `~/.lf/session-token`. Local connections use it automatically.
- **lfq**: fall back to reading `~/.lf/session-token` when `LFD_TOKEN` is unset.
- **lf CLI**: does not talk to lfd directly (it's a local step runner). No change.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Unix domain socket | Eliminates network exposure entirely | Breaks Concerto's URLSession (no UDS support on macOS without custom transport), breaks container mode, and is a larger change than needed for Phase 01. Worth revisiting for Phase 06+ |
| CORS-only defense | Block browser-originated requests via Origin header | Only blocks browser fetch/XHR. Doesn't stop `curl`, rogue processes, or extensions with native messaging. Defense-in-depth, not primary control |
| Per-request HMAC signing | Sign each request with a shared secret + timestamp | Over-engineered for local-only auth. Bearer token with constant-time comparison is sufficient when the transport is loopback |
| mTLS with self-signed certs | Mutual TLS between lfd and clients | Massive complexity for local connections. Makes debugging painful (`curl` stops working without client certs). Save for remote auth |

## Key decisions

### Session token, not static config token

The ingested item suggests `~/.lf/session-token`. This is the right call. A static token in `lfd.yaml` would be yet another credential to manage and wouldn't rotate. Generating on startup means:
- No user configuration needed
- Token rotates automatically on restart
- File permissions are the OS-level access control (same-user reads it, other users can't)

The wave security README says *"No 'it's just localhost' exceptions"* — this eliminates the biggest one.

### Three tiers, not two

The ingested item proposes Public / Read / Mutate. Two tiers (public vs authenticated) would be simpler but breaks monitoring workflows. `curl http://localhost:2486/v0/waves` is how operators check status in scripts and terminals. Requiring auth for reads would force every monitoring script to locate and read the token file, which is friction without meaningful security gain — if an attacker can read GET responses, they learn wave names and statuses, not credentials.

The wave security invariant *"No unauthenticated mutation"* is the bright line. Reads across loopback are an acceptable residual risk.

### Method-based classification, not separate routers

The ingested item shows `route_is_mutation(&request)`. Two implementation options:

**Option A**: Split the axum router into three groups (public, read-auth, mutate-auth) with separate middleware layers. Clean separation but requires moving routes between groups, touching every handler registration.

**Option B**: Keep the single router, classify inside the middleware by checking `request.method()`. `GET` and `HEAD` are reads; everything else is a mutation. Simple, correct for all current routes, and trivially auditable.

Going with **Option B**. Every mutation route uses POST, PATCH, or DELETE. Every read route uses GET. This is already the convention and HTTP semantics require it. The classification function is four lines, not a data structure to maintain.

Edge cases:
- `GET /ws` (WebSocket upgrade): classified as read. WebSocket messages after upgrade are outside HTTP middleware scope. Phase 04 adds WebSocket-level auth.
- `POST /hooks/git` and `POST /v0/hooks/github`: not behind auth middleware at all (they're outside the `api_routes` and `protected_routes` layers). Git hooks validate via webhook secrets, not bearer tokens.

### AuthProvider::Local gains token validation

Today `Local` means "loopback only." After this change, `Local` means "loopback reads are open, all mutations require the session token, and the token also works for remote reads."

This means a remote client with the session token can reach a `Local`-mode lfd — useful for SSH tunnels and port-forwarded containers. The existing behavior of rejecting remote connections without auth stays for `Local` without a token.

The `Static` and `Studio` providers don't change. They don't generate session tokens. Phase 06 removes their loopback bypass entirely.

### Token stored in HttpState

The session token string is stored in `HttpState` alongside the `AuthProvider`. The middleware reads it from state. No global, no `lazy_static`, no file re-reads on every request.

```rust
pub struct HttpState {
    // ... existing fields ...
    pub session_token: Option<String>,  // None when provider != Local
}
```

## Scope

### In scope

**Rust (lfd)**:
- `session_token` module: generate token, write to `~/.lf/session-token` with `0o600`
- `auth.rs`: modify `auth_middleware` — loopback bypass only for non-mutation methods; validate session token for mutations
- `bin/lfd.rs`: generate and store token at startup, pass to `HttpState`
- `http/state.rs`: add `session_token: Option<String>` field
- Tests: middleware denies unauthenticated mutations, allows authenticated mutations, allows loopback reads, denies remote reads without token

**Swift (Concerto)**:
- `FileTokenProvider`: reads `~/.lf/session-token`, implements `TokenProvider`
- `ServerConnection`: local connections use `FileTokenProvider` automatically
- No UI changes — local auth is transparent

**Python (lfq)**:
- `client.py`: when `LFD_TOKEN` is unset, read `~/.lf/session-token`

### Out of scope

- `Static`/`Studio` loopback hardening (Phase 06)
- TLS for local connections
- Per-wave authorization
- Rate limiting on auth failures (Phase 04)
- WebSocket message-level auth
- Token rotation without restart

## Implementation details

### File changes (Rust)

**New file: `rust/loopflow/src/lfd/session_token.rs`**

```rust
use rand::RngCore;
use std::path::PathBuf;

pub fn generate_and_write() -> Result<String, std::io::Error> {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let token = hex::encode(bytes);

    let path = token_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Write then set permissions (Unix-only for 0o600)
    std::fs::write(&path, &token)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }

    Ok(token)
}

pub fn token_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".lf")
        .join("session-token")
}
```

**Modified: `rust/loopflow/src/lfd/auth.rs`**

```rust
pub async fn auth_middleware(
    State(state): State<HttpState>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Response {
    let is_loopback = connect_info
        .map(|ConnectInfo(addr)| addr.ip().is_loopback())
        .unwrap_or(false);

    // Loopback non-mutation requests bypass auth (read tier).
    if is_loopback && !is_mutation(&request) {
        return next.run(request).await;
    }

    match &state.auth {
        AuthProvider::Local => {
            // Local provider: validate session token for mutations
            // and for remote reads.
            match (&state.session_token, extract_token(&headers)) {
                (Some(expected), Some(provided))
                    if constant_time_eq(provided, expected) =>
                {
                    next.run(request).await
                }
                (Some(_), Some(_)) => {
                    auth_error(StatusCode::UNAUTHORIZED, "invalid token")
                }
                (Some(_), None) if is_loopback => {
                    // Loopback mutation without token
                    auth_error(
                        StatusCode::FORBIDDEN,
                        "mutations require session token",
                    )
                }
                _ => auth_error(
                    StatusCode::FORBIDDEN,
                    "remote access requires auth configuration",
                ),
            }
        }
        AuthProvider::Static { token } => { /* unchanged */ }
        AuthProvider::Studio { validator } => { /* unchanged */ }
    }
}

fn is_mutation(request: &Request) -> bool {
    !matches!(
        request.method(),
        &axum::http::Method::GET | &axum::http::Method::HEAD | &axum::http::Method::OPTIONS
    )
}
```

**Modified: `rust/loopflow/src/lfd/http/state.rs`**

Add `pub session_token: Option<String>` to `HttpState`.

**Modified: `rust/loopflow/src/bin/lfd.rs`**

After determining `AuthProvider::Local`, generate the session token:

```rust
let session_token = if matches!(&auth_provider, AuthProvider::Local) {
    match loopflow::lfd::session_token::generate_and_write() {
        Ok(token) => {
            tracing::info!(
                path = %loopflow::lfd::session_token::token_path().display(),
                "session token written"
            );
            Some(token)
        }
        Err(err) => {
            tracing::error!(error = %err, "failed to write session token");
            std::process::exit(1);
        }
    }
} else {
    None
};
```

### File changes (Swift)

**New: `FileTokenProvider`** in `swift/LoopflowCore/Services/`

Reads `~/.lf/session-token` from disk. Returns the token string. Conforms to existing `TokenProvider` protocol. Used automatically when `ServerConnection.isLocal` is true.

### File changes (Python)

**Modified: `python/loopflow/client.py`**

```python
def _resolve_token() -> Optional[str]:
    token = os.environ.get("LFD_TOKEN")
    if token:
        return token
    token_path = Path.home() / ".lf" / "session-token"
    try:
        return token_path.read_text().strip()
    except (FileNotFoundError, PermissionError):
        return None
```

### Route classification (complete)

| Route | Method | Tier |
|-------|--------|------|
| `/health` | GET | Public (not behind auth middleware) |
| `/metrics` | GET | Public (not behind auth middleware) |
| `/hooks/git` | POST | Public (webhook secret validation) |
| `/v0/hooks/github` | POST | Public (HMAC-SHA256 validation) |
| `/status` | GET | Read |
| `/ws` | GET | Read |
| `/v0/flows` | GET | Read |
| `/v0/repos` | GET | Read |
| `/v0/wave/schemas` | GET | Read |
| `/v0/waves` | GET | Read |
| `/v0/wave_runs` | GET | Read |
| `/v0/waves/:id` | GET | Read |
| `/v0/waves/:id/runs` | GET | Read |
| `/v0/waves/:id/logs` | GET | Read |
| `/v0/waves/:id/stimuli` | GET | Read |
| `/v0/worktrees` | GET | Read |
| `/v0/waves` | POST | Mutate |
| `/v0/waves/:id` | PATCH | Mutate |
| `/v0/waves/:id` | DELETE | Mutate |
| `/v0/waves/:id/run` | POST | Mutate |
| `/v0/waves/:id/stimulus` | POST | Mutate |
| `/v0/waves/:id/stimulus/:sid` | DELETE | Mutate |
| `/v0/waves/:id/stop` | POST | Mutate |
| `/v0/waves/:id/restart-step` | POST | Mutate |
| `/v0/waves/:id/continue` | POST | Mutate |
| `/v0/waves/:id/land` | POST | Mutate |
| `/v0/waves/:id/next` | POST | Mutate |
| `/v0/waves/:id/check-ci` | POST | Mutate |
| `/v0/waves/:id/combine` | POST | Mutate |

Every GET is a read. Every POST/PATCH/DELETE behind auth middleware is a mutation. The `is_mutation()` function uses HTTP method, not a route table.

## Done when

1. `cargo test -p loopflow` passes with new auth tests covering:
   - Loopback GET without token: 200
   - Loopback POST without token: 403
   - Loopback POST with valid token: 200
   - Remote GET without token: 403 (Local provider)
   - Remote GET with valid token: 200
   - Remote POST with valid token: 200

2. `~/.lf/session-token` exists after `lfd` startup with `0o600` permissions

3. `uv run pytest python/tests/` passes with token auto-discovery test

4. Concerto connects to local lfd and can create/run waves (manual verification)

5. `curl http://localhost:2486/v0/waves` returns 200 (read tier, loopback)

6. `curl -X POST http://localhost:2486/v0/waves -d '{}'` returns 403 (mutate tier, no token)
