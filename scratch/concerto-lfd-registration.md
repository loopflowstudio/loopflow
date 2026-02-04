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

### Machine ID

```python
# loopflow/lfd/daemon/machine_id.py

from pathlib import Path
import uuid

def get_machine_id() -> str:
    """Get or create persistent machine identifier."""
    machine_id_path = Path.home() / ".lf" / "machine_id"

    if machine_id_path.exists():
        return machine_id_path.read_text().strip()

    machine_id = str(uuid.uuid4())
    machine_id_path.parent.mkdir(parents=True, exist_ok=True)
    machine_id_path.write_text(machine_id)
    return machine_id

def get_machine_name() -> str:
    """Get human-readable machine name."""
    import socket
    return socket.gethostname()
```

### Registration client

```python
# loopflow/lfd/daemon/registration.py

import asyncio
import httpx
from dataclasses import dataclass
from typing import Optional

@dataclass
class RegistrationState:
    registered: bool = False
    connection_token: Optional[str] = None
    expires_at: Optional[float] = None

class RegistrationClient:
    def __init__(self, base_url: str = "https://loopflow.studio"):
        self.base_url = base_url
        self.state = RegistrationState()
        self._heartbeat_task: Optional[asyncio.Task] = None

    async def register(self, jwt: str, machine_id: str, machine_name: str) -> str:
        """Register daemon with loopflow.studio. Returns connection token."""
        async with httpx.AsyncClient() as client:
            resp = await client.post(
                f"{self.base_url}/api/v1/daemons/register",
                headers={"Authorization": f"Bearer {jwt}"},
                json={
                    "machine_id": machine_id,
                    "machine_name": machine_name,
                    "capabilities": ["waves", "terminal", "grpc"],
                    "grpc_port": 50051,
                }
            )
            resp.raise_for_status()
            data = resp.json()

        self.state.registered = True
        self.state.connection_token = data["connection_token"]
        self.state.expires_at = data["expires_at"]

        return self.state.connection_token

    async def start_heartbeat(self, jwt: str, machine_id: str, interval: float = 30.0):
        """Start background heartbeat task."""
        async def heartbeat_loop():
            while True:
                await asyncio.sleep(interval)
                try:
                    await self._send_heartbeat(jwt, machine_id)
                except Exception as e:
                    # Log but don't crash—next heartbeat will retry
                    pass

        self._heartbeat_task = asyncio.create_task(heartbeat_loop())

    async def _send_heartbeat(self, jwt: str, machine_id: str):
        async with httpx.AsyncClient() as client:
            resp = await client.post(
                f"{self.base_url}/api/v1/daemons/heartbeat",
                headers={"Authorization": f"Bearer {jwt}"},
                json={"machine_id": machine_id}
            )
            resp.raise_for_status()

    async def deregister(self, jwt: str, machine_id: str):
        """Deregister on shutdown."""
        if self._heartbeat_task:
            self._heartbeat_task.cancel()

        if self.state.registered:
            try:
                async with httpx.AsyncClient() as client:
                    await client.post(
                        f"{self.base_url}/api/v1/daemons/deregister",
                        headers={"Authorization": f"Bearer {jwt}"},
                        json={"machine_id": machine_id}
                    )
            except Exception:
                pass  # Best effort on shutdown

        self.state = RegistrationState()
```

### Connection validation

```python
# loopflow/lfd/daemon/connection_validator.py

import time
from functools import lru_cache
import httpx

class ConnectionValidator:
    def __init__(self, base_url: str = "https://loopflow.studio"):
        self.base_url = base_url
        self._cache: dict[str, tuple[bool, float]] = {}  # token -> (valid, expires)

    async def validate_connection_token(self, token: str) -> bool:
        """Validate a connection token from mobile client."""
        # Check cache first
        if token in self._cache:
            valid, expires = self._cache[token]
            if time.time() < expires:
                return valid
            del self._cache[token]

        # Validate with loopflow.studio
        async with httpx.AsyncClient() as client:
            resp = await client.post(
                f"{self.base_url}/api/v1/daemons/validate-connection",
                json={"connection_token": token}
            )

        if resp.status_code == 200:
            data = resp.json()
            valid = data.get("valid", False)
            expires = data.get("expires_at", time.time() + 60)
            self._cache[token] = (valid, expires)
            return valid

        return False
```

### Integration with daemon startup

```python
# In loopflow/lfd/daemon/server.py run_server()

async def run_server(socket_path: str) -> None:
    # ... existing startup code ...

    # Registration (if auth configured)
    config = load_config()
    registration_client = None

    if config.auth.provider == "loopflow.studio":
        jwt = load_jwt()  # From ~/.lf/credentials.json
        if jwt:
            machine_id = get_machine_id()
            machine_name = get_machine_name()

            registration_client = RegistrationClient()
            try:
                await registration_client.register(jwt, machine_id, machine_name)
                await registration_client.start_heartbeat(jwt, machine_id)
                logging.info(f"Registered with loopflow.studio as {machine_name}")
            except Exception as e:
                logging.warning(f"Registration failed: {e}")
                # Continue without registration—local access still works

    # ... existing server code ...

    # On shutdown
    if registration_client:
        await registration_client.deregister(jwt, machine_id)
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

- [ ] `~/.lf/machine_id` created on first run with persistent UUID
- [ ] `lfd` registers on startup when `auth.provider: loopflow.studio` and JWT present
- [ ] Heartbeat runs every 30s while registered
- [ ] Clean deregister on graceful shutdown
- [ ] Connection token validation works for incoming gRPC connections
- [ ] Registration failure doesn't break local functionality
- [ ] `lfd status` shows registration state

## Open questions

- **Connection routing**: Direct connection requires knowing public IP. Should loopflow.studio track public IP from registration request, or do we need STUN/TURN for NAT traversal?
- **Multiple networks**: If Mac has multiple IPs (WiFi + Ethernet), which to register? Register all and let client try each?
- **Relay fallback**: When direct connection fails, should loopflow.studio relay traffic? Adds complexity and cost but improves reliability.

These are deferred to Phase 3 implementation. For now, registration provides the identity and discovery layer.
