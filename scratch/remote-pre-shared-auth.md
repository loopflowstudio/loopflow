# Pre-shared Token Auth

Static bearer token auth for remote lfd connections. No external dependencies.

## Problem

lfd requires loopflow.studio registration for non-loopback connections. Studio isn't live. Remote lfd (Phase 04+) is blocked on auth that doesn't exist yet.

A static token unblocks remote development. Generated once, shared out-of-band, validated locally. No network calls, no JWT infra.

## Approach

Replace the binary `AuthContext { active, registered, validator }` with an `AuthProvider` enum. The provider is selected from config and determines how the auth middleware validates requests.

Three providers:
- **Local** — loopback bypass (existing default behavior)
- **Static** — constant-time comparison against a configured token
- **Studio** — loopflow.studio registration + remote validation (existing)

The auth middleware dispatches to the provider. Token extraction (`extract_token`) is unchanged — it already reads `Authorization: Bearer <token>`.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| mTLS client certs | Strongest security, no tokens to leak | Heavy setup for dev/testing, cert management overhead, blocks Phase 04 progress |
| IP allowlist only | Zero tokens, simple | Too coarse — any process on the allowed IP gets access, doesn't work with NAT/VPN |
| Hash the token in config | Prevents config file exposure | Unnecessary complexity — if attacker reads lfd.yaml, they already have the machine |
| Token file instead of inline config | Separates secret from config | Adds a second file to manage with no real security benefit for dev use |

## Key decisions

**Constant-time comparison for static tokens.** Even though this is dev-only, use `subtle::ConstantTimeEq` to avoid timing side channels. It costs nothing and builds good habits for when Studio auth ships. The `subtle` crate is lightweight (no dependencies).

**Provider enum replaces AuthContext fields.** The current `{ active, registered, validator }` struct is a state machine encoded as booleans — `active=false` means local, `active=true && registered=false` means broken studio, `active=true && registered=true` means working studio. An enum is clearer and extensible.

**`LFD_AUTH_TOKEN` env var override.** Follows the existing pattern (`LFD_EXECUTOR_TYPE`, `LFD_GITHUB_TOKEN`). Makes Docker Compose deployment easier — token in `.env` file, not baked into config.

**`LFD_AUTH_PROVIDER` env var override.** Lets Docker Compose set `provider: static` without mounting a custom lfd.yaml.

**Python client gets `token` parameter.** The httpx `Client` constructor gets an optional `token` kwarg that injects `Authorization: Bearer` headers. Also reads `LFD_TOKEN` env var as default. This keeps the client change minimal and backwards-compatible.

**No client-side changes for Concerto (Swift) in this phase.** Phase 05 (Concerto Remote Connection) handles `ServerConnection` with token support. This phase is Rust server + Python client only.

**Loopback always bypasses auth, regardless of provider.** Even with `provider: static`, requests from 127.0.0.1 skip auth. This preserves local development and the Python CLI (`lfq`) running on the same machine.

Per the remote roadmap: "Start with a pre-shared static token for dev testing (Phase 03). Replace with real JWT auth via loopflow.studio when studio endpoints are live (Phase 07)."

## Scope

### In scope
- `AuthProvider` enum with `Local`, `Static`, `Studio` variants
- Config parsing: `auth.provider` and `auth.token` fields in `lfd.yaml`
- Env overrides: `LFD_AUTH_PROVIDER`, `LFD_AUTH_TOKEN`
- Static token validation with constant-time comparison
- Startup validation: `provider: static` without token → error
- Python client: `token` kwarg + `LFD_TOKEN` env var
- Tests: all three provider scenarios + missing token + wrong token

### Out of scope
- Token rotation, expiry, revocation
- Token generation CLI (`openssl rand -hex 32` in docs is enough)
- Concerto (Swift) changes (Phase 05)
- JWKS validation for Studio (Phase 07)
- Rate limiting on auth failures

## Implementation

### 1. Add `subtle` dependency

```toml
# rust/loopflow/Cargo.toml
subtle = "2"
```

### 2. `AuthProvider` enum (auth.rs)

Replace `AuthContext` with `AuthProvider`:

```rust
use subtle::ConstantTimeEq;

#[derive(Debug, Clone)]
pub enum AuthProvider {
    /// Loopback connections only. Non-loopback → 403.
    Local,
    /// Validate against a pre-shared static token.
    Static { token: String },
    /// Validate via loopflow.studio registration (existing).
    Studio { validator: ConnectionValidator },
}
```

Middleware becomes:

```rust
pub async fn auth_middleware(...) -> Response {
    // Loopback always bypasses, regardless of provider.
    if is_loopback(&connect_info) {
        return next.run(request).await;
    }

    match &state.auth {
        AuthProvider::Local => {
            auth_error(StatusCode::FORBIDDEN, "remote access requires auth configuration")
        }
        AuthProvider::Static { token } => {
            match extract_token(&headers) {
                Some(provided) if constant_time_eq(&provided, token) => {
                    next.run(request).await
                }
                Some(_) => auth_error(StatusCode::UNAUTHORIZED, "invalid token"),
                None => auth_error(StatusCode::UNAUTHORIZED, "missing token"),
            }
        }
        AuthProvider::Studio { validator } => {
            // existing validation logic
        }
    }
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    a.as_bytes().ct_eq(b.as_bytes()).into()
}
```

### 3. Config changes (config.rs)

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct AuthConfig {
    #[serde(default = "default_provider")]
    pub provider: String,       // "local" | "static" | "loopflow.studio"
    pub token: Option<String>,  // for static provider
    #[serde(default = "default_base_url")]
    pub base_url: String,       // for loopflow.studio provider
}
```

Env overrides in `apply_env_overrides()`:
```rust
if let Ok(value) = std::env::var("LFD_AUTH_PROVIDER") {
    self.auth.provider = value;
}
if let Ok(value) = std::env::var("LFD_AUTH_TOKEN") {
    self.auth.token = Some(value);
}
```

### 4. Startup flow (lfd.rs / mod.rs)

Replace `setup_registration` dispatch with provider-based init:

```rust
pub async fn setup_auth(
    config: &LfdConfig,
    cancel: CancellationToken,
) -> (AuthProvider, Option<RegistrationClient>, Option<(String, String)>) {
    match config.auth.provider.as_str() {
        "local" => (AuthProvider::Local, None, None),
        "static" => {
            let token = config.auth.token.as_ref()
                .expect("auth.provider=static requires auth.token in config or LFD_AUTH_TOKEN env");
            (AuthProvider::Static { token: token.clone() }, None, None)
        }
        "loopflow.studio" => {
            // existing setup_registration logic, returns Studio variant
        }
        other => {
            tracing::error!(provider = other, "unknown auth provider");
            std::process::exit(1);
        }
    }
}
```

In `lfd.rs`, the `requires_auth` check changes: when binding to non-loopback with `provider: local`, log a warning but don't crash.

### 5. Python client (client.py)

```python
class Client:
    def __init__(
        self,
        base_url: Optional[str] = None,
        timeout: float = 10.0,
        token: Optional[str] = None,
    ) -> None:
        resolved = base_url.rstrip("/") if base_url else _resolve_base_url()
        self._base_url = resolved
        resolved_token = token or os.environ.get("LFD_TOKEN")
        headers = {}
        if resolved_token:
            headers["Authorization"] = f"Bearer {resolved_token}"
        self._client = httpx.Client(
            base_url=resolved, timeout=timeout, headers=headers,
        )
```

### 6. HttpState change

`HttpState.auth` field type changes from `AuthContext` to `AuthProvider`. The `registration` field stays for Studio provider deregistration on shutdown.

### 7. Tests

**Rust unit tests (auth.rs):**
- `static_provider_accepts_correct_token` — mock request with matching bearer token → 200
- `static_provider_rejects_wrong_token` — wrong token → 401
- `static_provider_rejects_missing_token` — no auth header → 401
- `local_provider_allows_loopback` — 127.0.0.1 → 200
- `local_provider_rejects_remote` — non-loopback → 403
- `loopback_bypasses_any_provider` — static provider + 127.0.0.1 → 200 without token

**Config tests (config.rs):**
- `static_auth_config_parses` — yaml with `provider: static` + token
- `static_auth_without_token_uses_env` — `LFD_AUTH_TOKEN` env var
- `env_overrides_auth_provider` — `LFD_AUTH_PROVIDER` env var

**Python tests:**
- `client_sends_bearer_token` — token kwarg adds Authorization header
- `client_reads_lfd_token_env` — `LFD_TOKEN` env var

## Done when

- `provider: static` + token in config → lfd starts, validates bearer tokens
- Request without token → 401
- Request with wrong token → 401
- Request with correct token → 200
- Loopback bypasses auth regardless of provider
- `provider: local` on non-loopback → 403 (not crash)
- `LFD_AUTH_PROVIDER` and `LFD_AUTH_TOKEN` env vars work
- Python `Client(token="...")` sends bearer header
- `cargo test` and `pytest` pass
