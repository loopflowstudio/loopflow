# 05: Authentication

Enable remote access to lfd with authentication via loopflow.studio.

## Context

Phase 1 lfd runs locally with no auth (Unix socket provides OS-level access control).

For remote access (phone, laptop, different machine), we need authentication.

## Goal

1. loopflow.studio provides auth service (Clerk integration)
2. `lf auth login` performs browser OAuth flow
3. lfd validates JWTs against loopflow.studio
4. Self-hosters control who can access their daemon

## Architecture

```
┌───────────────┐    ┌───────────────────┐    ┌───────────────────────┐
│ User          │    │ loopflow.studio   │    │ Clerk                 │
│               │    │                   │    │                       │
│ lf auth login │───▶│ /auth/login       │───▶│ OAuth (Google/GitHub) │
│               │    │                   │    │                       │
│               │◀───│ JWT               │◀───│ User info             │
│               │    │                   │    │                       │
└───────────────┘    └───────────────────┘    └───────────────────────┘
       │
       │ JWT in Authorization header
       ▼
┌─────────────────────────────────────────────────────────────────────┐
│ lfd (self-hosted)                                                   │
│                                                                     │
│  1. Extract JWT from Authorization header                           │
│  2. Fetch JWKS from loopflow.studio (cached)                       │
│  3. Validate signature                                              │
│  4. Check claims (exp, aud, iss)                                   │
│  5. Check user in allowed_users config                             │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## loopflow.studio Auth Service

Minimal service - just auth, nothing else yet.

### Endpoints

```
GET  /auth/login
  → Redirect to Clerk hosted login

GET  /auth/callback
  → Receive Clerk callback
  → Create session
  → Issue loopflow JWT
  → Redirect to CLI callback

GET  /.well-known/jwks.json
  → Return public keys for JWT verification

GET  /auth/userinfo
  → Return user info from JWT (optional)

POST /auth/device/code
  → Device authorization flow (for headless)

POST /auth/device/token
  → Poll for token (device flow)
```

### Implementation

```typescript
// loopflow-studio/src/auth.ts
import { Clerk } from '@clerk/clerk-sdk-node';
import jwt from 'jsonwebtoken';

const clerk = new Clerk({ secretKey: process.env.CLERK_SECRET_KEY });

// Key pair for signing JWTs
const privateKey = fs.readFileSync('keys/private.pem');
const publicKey = fs.readFileSync('keys/public.pem');

export async function handleLogin(req: Request, res: Response) {
  const state = crypto.randomBytes(16).toString('hex');
  const redirectUrl = `https://loopflow.studio/auth/callback?state=${state}`;

  // Store state for validation
  await redis.set(`auth:state:${state}`, req.query.redirect_uri, 'EX', 300);

  // Redirect to Clerk
  res.redirect(clerk.redirectToSignIn({ redirectUrl }));
}

export async function handleCallback(req: Request, res: Response) {
  const { state, code } = req.query;

  // Validate state
  const redirectUri = await redis.get(`auth:state:${state}`);
  if (!redirectUri) {
    return res.status(400).send('Invalid state');
  }

  // Exchange code for Clerk session
  const session = await clerk.sessions.verifySession(code);
  const user = await clerk.users.getUser(session.userId);

  // Issue loopflow JWT
  const token = jwt.sign(
    {
      sub: user.id,
      email: user.emailAddresses[0].emailAddress,
      name: `${user.firstName} ${user.lastName}`,
    },
    privateKey,
    {
      algorithm: 'RS256',
      issuer: 'https://loopflow.studio',
      audience: 'loopflow-lfd',
      expiresIn: '7d',
    }
  );

  // Redirect back to CLI with token
  res.redirect(`${redirectUri}?token=${token}`);
}

export async function handleJWKS(req: Request, res: Response) {
  const jwk = await jose.exportJWK(publicKey);
  jwk.kid = 'loopflow-1';
  jwk.use = 'sig';
  jwk.alg = 'RS256';

  res.json({
    keys: [jwk],
  });
}
```

### Device Flow (for headless/servers)

```typescript
export async function handleDeviceCode(req: Request, res: Response) {
  const deviceCode = crypto.randomBytes(32).toString('hex');
  const userCode = crypto.randomBytes(4).toString('hex').toUpperCase();

  await redis.set(`device:${deviceCode}`, JSON.stringify({
    userCode,
    status: 'pending',
    createdAt: Date.now(),
  }), 'EX', 600);  // 10 minute expiry

  res.json({
    device_code: deviceCode,
    user_code: userCode,
    verification_uri: `https://loopflow.studio/auth/device?code=${userCode}`,
    expires_in: 600,
    interval: 5,
  });
}

export async function handleDeviceToken(req: Request, res: Response) {
  const { device_code } = req.body;

  const data = await redis.get(`device:${device_code}`);
  if (!data) {
    return res.status(400).json({ error: 'expired_token' });
  }

  const { status, token } = JSON.parse(data);

  if (status === 'pending') {
    return res.status(400).json({ error: 'authorization_pending' });
  }

  if (status === 'authorized') {
    await redis.del(`device:${device_code}`);
    return res.json({ access_token: token, token_type: 'Bearer' });
  }
}
```

## CLI Auth

### Login Command

```rust
// rust/lf/src/commands/auth.rs

pub async fn login() -> Result<()> {
    // Start local callback server
    let (tx, rx) = oneshot::channel();
    let server = start_callback_server(tx).await?;
    let callback_url = format!("http://localhost:{}/callback", server.port());

    // Open browser
    let auth_url = format!(
        "https://loopflow.studio/auth/login?redirect_uri={}",
        urlencoding::encode(&callback_url)
    );

    println!("Opening browser for login...");
    println!("If browser doesn't open, visit: {}", auth_url);

    open::that(&auth_url)?;

    // Wait for callback
    let token = tokio::time::timeout(
        Duration::from_secs(120),
        rx
    ).await??;

    // Save token
    save_token(&token)?;

    println!("Logged in successfully!");

    Ok(())
}

async fn start_callback_server(tx: oneshot::Sender<String>) -> Result<CallbackServer> {
    let app = Router::new()
        .route("/callback", get(move |Query(params): Query<CallbackParams>| async move {
            let _ = tx.send(params.token);
            Html("<h1>Login successful!</h1><p>You can close this window.</p>")
        }));

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();

    tokio::spawn(async move {
        axum::serve(listener, app).await
    });

    Ok(CallbackServer { port })
}

fn save_token(token: &str) -> Result<()> {
    let creds_path = dirs::config_dir()
        .unwrap()
        .join("lf/credentials.json");

    fs::create_dir_all(creds_path.parent().unwrap())?;

    let creds = Credentials {
        version: 1,
        tokens: HashMap::from([(
            "loopflow.studio".to_string(),
            TokenEntry {
                token: token.to_string(),
                expires_at: decode_jwt_exp(token)?,
            }
        )]),
    };

    fs::write(&creds_path, serde_json::to_string_pretty(&creds)?)?;

    Ok(())
}
```

### Device Flow (Headless)

```rust
pub async fn login_device() -> Result<()> {
    // Request device code
    let resp: DeviceCodeResponse = reqwest::Client::new()
        .post("https://loopflow.studio/auth/device/code")
        .send()
        .await?
        .json()
        .await?;

    println!("Visit: {}", resp.verification_uri);
    println!("Enter code: {}", resp.user_code);

    // Poll for token
    let client = reqwest::Client::new();
    loop {
        tokio::time::sleep(Duration::from_secs(resp.interval)).await;

        let token_resp = client
            .post("https://loopflow.studio/auth/device/token")
            .json(&json!({ "device_code": resp.device_code }))
            .send()
            .await?;

        if token_resp.status().is_success() {
            let token: TokenResponse = token_resp.json().await?;
            save_token(&token.access_token)?;
            println!("Logged in successfully!");
            return Ok(());
        }

        let err: ErrorResponse = token_resp.json().await?;
        if err.error != "authorization_pending" {
            return Err(anyhow!("Login failed: {}", err.error));
        }
    }
}
```

## lfd JWT Validation

### Configuration

```yaml
# ~/.lf/lfd.yaml
auth:
  # Local mode (Phase 1 default)
  provider: local

  # Or: loopflow.studio (Phase 2)
  provider: loopflow.studio
  allowed_users:
    - user_abc123           # Clerk user ID
    - user@example.com      # Or by email
  jwks_url: https://loopflow.studio/.well-known/jwks.json
  audience: loopflow-lfd

  # Or: API keys (air-gapped escape hatch)
  provider: api_keys
  keys:
    - name: ci-runner
      key_hash: sha256:abc123...
      scopes: [wave:read, wave:trigger]
```

### JWT Middleware

```rust
// rust/lfd/src/auth/mod.rs

pub struct JwtValidator {
    jwks: Arc<RwLock<JWKS>>,
    config: AuthConfig,
}

impl JwtValidator {
    pub async fn new(config: AuthConfig) -> Result<Self> {
        let jwks = fetch_jwks(&config.jwks_url).await?;

        // Refresh JWKS periodically
        let jwks = Arc::new(RwLock::new(jwks));
        let jwks_clone = jwks.clone();
        let url = config.jwks_url.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(3600));
            loop {
                interval.tick().await;
                if let Ok(new_jwks) = fetch_jwks(&url).await {
                    *jwks_clone.write().await = new_jwks;
                }
            }
        });

        Ok(Self { jwks, config })
    }

    pub async fn validate(&self, token: &str) -> Result<Claims> {
        let jwks = self.jwks.read().await;

        // Decode header to get kid
        let header = jsonwebtoken::decode_header(token)?;
        let kid = header.kid.ok_or_else(|| anyhow!("missing kid"))?;

        // Find key
        let key = jwks.keys.iter()
            .find(|k| k.kid == Some(kid.clone()))
            .ok_or_else(|| anyhow!("unknown key"))?;

        // Validate
        let validation = jsonwebtoken::Validation::new(Algorithm::RS256);
        validation.set_audience(&[&self.config.audience]);
        validation.set_issuer(&["https://loopflow.studio"]);

        let data = jsonwebtoken::decode::<Claims>(token, &key.to_decoding_key()?, &validation)?;

        // Check allowed users
        if !self.config.allowed_users.is_empty() {
            let allowed = self.config.allowed_users.iter().any(|u| {
                u == &data.claims.sub || u == &data.claims.email
            });
            if !allowed {
                return Err(anyhow!("user not allowed"));
            }
        }

        Ok(data.claims)
    }
}
```

### gRPC Interceptor

```rust
// rust/lfd/src/auth/interceptor.rs

pub fn auth_interceptor(
    validator: Arc<JwtValidator>,
) -> impl Fn(Request<()>) -> Result<Request<()>, Status> + Clone {
    move |mut req: Request<()>| {
        // Skip auth for health checks
        let path = req.uri().path();
        if path == "/grpc.health.v1.Health/Check" {
            return Ok(req);
        }

        // Extract token
        let token = req.metadata()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or_else(|| Status::unauthenticated("missing token"))?;

        // Validate
        let claims = validator.validate(token)
            .map_err(|e| Status::unauthenticated(e.to_string()))?;

        // Attach claims to request
        req.extensions_mut().insert(claims);

        Ok(req)
    }
}
```

### TLS Support

```rust
// rust/lfd/src/main.rs

async fn run_grpc_server(config: &Config, store: SharedStore) -> Result<()> {
    let addr = config.grpc_addr.parse()?;

    let mut builder = Server::builder();

    // Add TLS if configured
    if let Some(tls) = &config.tls {
        let cert = tokio::fs::read(&tls.cert_path).await?;
        let key = tokio::fs::read(&tls.key_path).await?;

        let identity = Identity::from_pem(cert, key);
        builder = builder.tls_config(ServerTlsConfig::new().identity(identity))?;
    }

    // Add auth interceptor if configured
    let service = if config.auth.provider != "local" {
        let validator = Arc::new(JwtValidator::new(config.auth.clone()).await?);
        ControlServiceServer::with_interceptor(
            ControlServer::new(store),
            auth_interceptor(validator)
        )
    } else {
        ControlServiceServer::new(ControlServer::new(store))
    };

    builder.add_service(service).serve(addr).await?;

    Ok(())
}
```

## Done When

- [ ] loopflow.studio `/auth/login` redirects to Clerk
- [ ] loopflow.studio `/auth/callback` issues JWTs
- [ ] loopflow.studio `/.well-known/jwks.json` returns public keys
- [ ] `lf auth login` opens browser, receives token
- [ ] `lf auth login --device` works for headless
- [ ] Token saved to `~/.lf/credentials.json`
- [ ] lfd validates JWTs from Authorization header
- [ ] lfd caches JWKS with periodic refresh
- [ ] `allowed_users` config restricts access
- [ ] TLS termination works in lfd
- [ ] API key fallback for air-gapped environments

## Dependencies

- Requires: 01-lf-cli, 02-lfd-primary, 03-service, 04-distribution
- Enables: Remote access to lfd
