# 03: Pre-shared Token Auth

Simple static token auth so lfd can accept remote connections without loopflow.studio.

## What exists after this

lfd on a remote host accepts HTTP/WS connections authenticated with a static bearer token. Local mode (127.0.0.1) still works without auth.

## Context

The auth middleware already exists in `rust/loopflow/src/lfd/auth.rs`. It distinguishes loopback (no auth) from non-loopback (auth required). Currently non-loopback requires registration with loopflow.studio, which isn't live yet.

Add a `static` auth provider: lfd reads a token from config, clients present it as a bearer token. No external dependencies.

## Configuration

```yaml
# ~/.lf/lfd.yaml
host: 0.0.0.0
port: 2486
auth:
  provider: static
  token: "your-secret-token-here"
```

When `provider: local` (default), auth is skipped for loopback connections. When `provider: static`, any non-loopback request must include the token.

## Implementation

### Rust changes (~100 LOC)

```rust
// lfd/auth.rs — add StaticTokenAuth variant

pub enum AuthProvider {
    Local,                          // existing: skip auth for loopback
    Static { token: String },       // new: validate against config token
    LoopflowStudio { /* ... */ },   // existing: JWT via studio
}

impl AuthProvider {
    /// Load from lfd.yaml config
    pub fn from_config(config: &LfdConfig) -> Result<Self> {
        match config.auth.provider.as_str() {
            "local" => Ok(Self::Local),
            "static" => {
                let token = config.auth.token
                    .as_ref()
                    .ok_or_else(|| anyhow!("static auth requires 'token' in config"))?;
                Ok(Self::Static { token: token.clone() })
            }
            "loopflow.studio" => { /* existing */ }
            other => Err(anyhow!("unknown auth provider: {other}")),
        }
    }
}

// In the auth middleware:
fn validate_request(provider: &AuthProvider, req: &Request, is_loopback: bool) -> Result<()> {
    match provider {
        AuthProvider::Local if is_loopback => Ok(()),
        AuthProvider::Local => Err(AuthError::RemoteAccessDisabled),
        AuthProvider::Static { token } => {
            let provided = extract_bearer_token(req)?;
            if provided == token {
                Ok(())
            } else {
                Err(AuthError::InvalidToken)
            }
        }
        AuthProvider::LoopflowStudio { .. } => { /* existing JWT validation */ }
    }
}
```

### Config parsing

```rust
// lfd/config.rs — add auth fields

#[derive(Deserialize, Debug)]
pub struct AuthConfig {
    #[serde(default = "default_provider")]
    pub provider: String,          // "local" | "static" | "loopflow.studio"
    pub token: Option<String>,     // for static provider
    pub allowed_users: Option<Vec<String>>,  // for loopflow.studio
    pub jwks_url: Option<String>,  // for loopflow.studio
}

fn default_provider() -> String {
    "local".to_string()
}
```

### Client-side token

Concerto and the Python client need to send the token:

```swift
// Swift — add token to HTTP requests
var request = URLRequest(url: url)
if let token = connectionToken {
    request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
}
```

```python
# Python client — already supports headers
client = Client(base_url="http://ec2-host:2486", headers={"Authorization": f"Bearer {token}"})
```

## Security notes

- Static tokens are for dev/testing. No expiry, no rotation, no revocation.
- IP-restrict the security group (Phase 04) as defense in depth.
- For production, replace with JWT auth (Phase 07).
- Token should be generated with `openssl rand -hex 32` (256-bit).

## Done when

- lfd starts with `provider: static` and a token in config
- Request without token → 401
- Request with wrong token → 401
- Request with correct token → 200
- Loopback requests still work without token when `provider: local`
- `cargo test` covers all three cases
