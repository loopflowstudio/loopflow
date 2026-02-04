# lfd Registration

lfd registers with loopflow.studio to enable remote connections from mobile and other devices.

## Problem

lfd runs locally with no identity. For Phase 3 mobile access, Concerto on a phone needs to find and connect to the user's Mac. Without registration, there's no discovery—the mobile client would need to know the Mac's IP address, which changes and may be behind NAT.

loopflow.studio acts as the rendezvous point. lfd registers itself, mobile clients query the registration, and connections can be established (either direct or relayed).

## Approach

lfd calls loopflow.studio on startup with a machine identifier. loopflow.studio issues a connection token that mobile clients use to prove they're authorized to connect. lfd maintains registration with periodic heartbeats.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  lfd (user's Mac)                                                           │
│                                                                             │
│  1. Generate machine_id (persistent UUID in ~/.lf/machine_id)               │
│  2. POST /api/v1/daemons/register with JWT + machine_id                     │
│  3. Receive connection_token for validating mobile clients                  │
│  4. Heartbeat every 30s to maintain registration                            │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
         │                         ▲
         │ Register                │ Heartbeat
         ▼                         │
┌─────────────────────────────────────────────────────────────────────────────┐
│  loopflow.studio                                                            │
│                                                                             │
│  Tracks registered daemons:                                                 │
│  - user_id (from JWT)                                                       │
│  - machine_id (stable across restarts)                                      │
│  - machine_name (human-readable, from hostname)                             │
│  - last_seen (for stale detection)                                          │
│  - connection_info (IP, port, capabilities)                                 │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
         │                         ▲
         │ List my daemons         │ Connect request
         ▼                         │
┌─────────────────────────────────────────────────────────────────────────────┐
│  Concerto (mobile)                                                          │
│                                                                             │
│  1. Authenticate to loopflow.studio (get JWT)                               │
│  2. GET /api/v1/daemons → list of user's registered daemons                 │
│  3. Select daemon, get connection_token                                     │
│  4. Connect to lfd with connection_token                                    │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Direct IP connection | Simpler, no central service | NAT traversal, dynamic IPs, no discovery |
| Tailscale/ZeroTier | Proven solution | Requires separate account, extra software |
| mDNS/Bonjour | Works on LAN | No remote access outside local network |
| Manual config | No dependencies | Terrible UX, error-prone |

Central registration wins: handles discovery, works across networks, integrates with existing auth, no additional software for users.

## Key decisions

### 1. Machine ID: persistent UUID, not hardware ID

Generate a random UUID on first run, store in `~/.lf/machine_id`. Hardware IDs (MAC address, serial number) are:
- Platform-specific to extract
- May require elevated permissions
- Can conflict in VMs or containers
- Privacy-sensitive

A persistent UUID is simple, portable, and sufficient. If the file is deleted, the daemon gets a new identity—acceptable tradeoff.

### 2. Registration optional until remote access is configured

Local-only usage (Phase 1/2) doesn't require registration. lfd should work without loopflow.studio account. Registration activates when:
- User runs `lf auth login` (gets JWT)
- User configures `auth.provider: loopflow.studio` in `~/.lf/lfd.yaml`

Default behavior: no registration, no outbound calls, full local functionality.

### 3. Heartbeat at 30s, stale after 90s

Aggressive heartbeat because mobile users expect fast feedback. If a daemon dies without deregistering, mobile sees "offline" within 90 seconds. Tradeoff: more API calls, but registration is cheap (small payload, idempotent).

### 4. Connection token is short-lived, per-session

When mobile requests to connect, loopflow.studio issues a connection token valid for 5 minutes. Mobile presents this token to lfd. lfd validates by calling loopflow.studio (with caching). This prevents:
- Stolen tokens being used indefinitely
- Replay attacks across sessions
- Need for lfd to maintain user database

### 5. gRPC for remote, not HTTP

Remote connections use gRPC (port 50051) with TLS, not the HTTP API (port 2486). gRPC provides:
- Bidirectional streaming for terminal I/O
- Strong typing via protobuf
- Built-in TLS support
- Efficient binary protocol

HTTP API remains localhost-only. This is the existing architecture—remote just adds auth.

## Implementation

Implemented in Rust lfd (`rust/lfd/`), not Python.

### Machine ID

```rust
// rust/lfd/src/machine_id.rs

pub fn get_machine_id() -> String {
    let machine_id_path = machine_id_path();

    if let Ok(id) = std::fs::read_to_string(&machine_id_path) {
        let id = id.trim();
        if !id.is_empty() {
            return id.to_string();
        }
    }

    let id = Uuid::new_v4().to_string();
    if let Some(parent) = machine_id_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&machine_id_path, &id);
    id
}

pub fn get_machine_name() -> String {
    gethostname::gethostname().to_string_lossy().into_owned()
}
```

### Registration client

```rust
// rust/lfd/src/registration.rs

pub struct RegistrationClient {
    base_url: String,
    state: Arc<RwLock<RegistrationState>>,
    connection_token: Arc<RwLock<Option<String>>>,
}

impl RegistrationClient {
    pub async fn register(
        &self,
        jwt: &str,
        machine_id: &str,
        machine_name: &str,
    ) -> Result<String, RegistrationError> {
        let client = reqwest::Client::new();
        let url = format!("{}/api/v1/daemons/register", self.base_url);

        let payload = serde_json::json!({
            "machine_id": machine_id,
            "machine_name": machine_name,
            "capabilities": ["waves", "terminal", "grpc"],
            "grpc_port": 50051,
        });

        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {jwt}"))
            .json(&payload)
            .send()
            .await?;

        // ... parse response, update state
        Ok(data.connection_token)
    }

    pub fn start_heartbeat(
        &self,
        jwt: String,
        machine_id: String,
        cancel: CancellationToken,
    ) -> JoinHandle<()> {
        // Spawns background task that heartbeats every 30s
    }

        // Best effort deregister on shutdown
    }
}
```

### Connection validation

```rust
// rust/lfd/src/registration.rs

pub struct ConnectionValidator {
    base_url: String,
    cache: Arc<RwLock<HashMap<String, (bool, Instant)>>>,
}

impl ConnectionValidator {
    pub async fn validate(&self, token: &str) -> bool {
        // Check cache first (60s TTL)
        // If not cached, POST to /api/v1/daemons/validate-connection
        // Cache result and return
    }
}
```

### Auth context for gRPC

```rust
// rust/lfd/src/auth.rs

pub struct AuthContext {
    pub enabled: Arc<RwLock<bool>>,
    pub registered: Arc<RwLock<bool>>,
    pub validator: Option<ConnectionValidator>,
}

impl AuthContext {
    /// Check if a request should be authenticated.
    /// Returns Ok(()) if allowed, Err(Status) if denied.
    pub async fn check_request<T>(&self, request: &Request<T>) -> Result<(), Status> {
        // If registration not enabled/registered, allow all
        // Otherwise, extract and validate connection token
    }
}
```

### Integration with daemon startup

```rust
// rust/lfd/src/main.rs

async fn setup_registration(
    config: &LfdConfig,
    cancel: CancellationToken,
) -> (Option<RegistrationClient>, AuthContext, Option<(String, String)>) {
    if config.auth.provider != Some("loopflow.studio") {
        return (None, AuthContext::disabled(), None);
    }

    let jwt = credentials::load_jwt()?;
    let machine_id = machine_id::get_machine_id();
    let machine_name = machine_id::get_machine_name();

    let client = RegistrationClient::new(&config.auth.base_url);
    match client.register(&jwt, &machine_id, &machine_name).await {
        Ok(_) => {
            tracing::info!(machine_name = %machine_name, "registered with loopflow.studio");
            client.start_heartbeat(jwt.clone(), machine_id.clone(), cancel);
            (Some(client), auth_context, Some((jwt, machine_id)))
        }
        Err(e) => {
            tracing::warn!(error = %e, "registration failed");
            (Some(client), auth_context, None)
        }
    }
}

// On shutdown: client.deregister(&jwt, &machine_id).await;
```

### loopflow.studio API (server-side reference)

```typescript
// For context—this runs on loopflow.studio, not in lfd

interface Daemon {
  user_id: string;
  machine_id: string;
  machine_name: string;
  last_seen: Date;
  connection_info: {
    grpc_port: number;
    capabilities: string[];
  };
}

// POST /api/v1/daemons/register
// - Validates JWT, extracts user_id
// - Upserts daemon record
// - Issues connection_token (signed, 5min expiry)

// POST /api/v1/daemons/heartbeat
// - Updates last_seen

// GET /api/v1/daemons
// - Returns user's registered daemons (filtered by last_seen > now - 90s)

// POST /api/v1/daemons/validate-connection
// - Validates connection_token signature and expiry
// - Returns user_id and machine_id if valid
```

## Scope

**In scope:**
- Machine ID generation and persistence
- Registration client with heartbeat
- Connection token validation
- Integration with daemon startup/shutdown
- Config option to enable/disable registration

**Out of scope:**
- loopflow.studio server implementation (separate service)
- NAT traversal / relay (future phase)
- Multiple daemons per machine (single daemon design)
- Offline mode caching (requires network for remote access anyway)

## Done when

- [x] `~/.lf/machine_id` created on first run with persistent UUID
- [x] `lfd` registers on startup when `auth.provider: loopflow.studio` and JWT present
- [x] Heartbeat runs every 30s while registered
- [x] Clean deregister on graceful shutdown
- [x] Connection token validation works for incoming gRPC connections
- [x] Registration failure doesn't break local functionality
- [x] `/status` and `/health` HTTP endpoints show registration state

**Note:** Implemented in Rust lfd (`rust/lfd/`), not Python lfd. Python lfd is deprecated.

## Open questions

- **Connection routing**: Direct connection requires knowing public IP. Should loopflow.studio track public IP from registration request, or do we need STUN/TURN for NAT traversal?
- **Multiple networks**: If Mac has multiple IPs (WiFi + Ethernet), which to register? Register all and let client try each?
- **Relay fallback**: When direct connection fails, should loopflow.studio relay traffic? Adds complexity and cost but improves reliability.

These are deferred to Phase 3 implementation. For now, registration provides the identity and discovery layer.
