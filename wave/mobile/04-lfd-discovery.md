# 04: lfd Discovery via Auth

Login on mobile, see your running lfds. Tailscale handles connectivity. Zero config.

## What to build

lfd registers with studio auth servers when started in publish mode. Studio is a discovery service — it tracks which lfds are online and their Tailscale addresses. When you login on mobile, Concerto discovers your lfds via studio and connects directly over Tailscale.

No relay service. Tailscale handles NAT traversal. Studio just answers "where are my lfds?"

Stage 04 is additive on top of direct/manual connection from Stage 01/03. If discovery is unavailable, manual host:port must still work.

## How it works

### Publish mode (lfd side)

`lfd dev --publish` (or `publish = true` in config):

1. Authenticates with studio using loopflow account token
2. Detects Tailscale IP via `tailscale status --json`
3. Reports address + port to studio's discovery registry
4. Sends periodic heartbeats: repo name, address, online status
5. Deregisters on graceful shutdown, times out on crash (~30s)

```
lfd dev --publish
# → authenticates with studio.loopflow.dev
# → Tailscale: 100.64.1.5
# → "Publishing as jack/loopflow (lfd-abc123)"
# → ready for remote connections
```

Tailscale is the expected connectivity layer. If Tailscale isn't running, `--publish` warns and publishes LAN address only (works on same network but not remotely).

lfd's existing HTTP + WebSocket APIs work unchanged. No new lfd API surface.

### Discovery service (studio side)

Studio runs a lightweight discovery registry — no relay, no proxying, no data plane:

- **Registry**: maps user → list of published lfds (id, repo name, address, online status, last heartbeat)
- **Discovery endpoint**: `GET /v0/lfds` — returns the user's published lfds (authenticated)
- **Health**: marks lfds offline when heartbeats stop

```json
GET /v0/lfds
Authorization: Bearer <token>

{
  "lfds": [
    {
      "id": "lfd-abc123",
      "name": "loopflow",
      "repo": "loopflow",
      "status": "online",
      "address": "100.64.1.5:4242",
      "last_heartbeat": "2026-02-24T18:45:00Z"
    },
    {
      "id": "lfd-def456",
      "name": "concerto",
      "repo": "loopflow.mobile",
      "status": "online",
      "address": "100.64.1.5:4243",
      "last_heartbeat": "2026-02-24T18:44:55Z"
    }
  ]
}
```

This is a thin service — a table of heartbeats behind an auth check. No WebSocket proxying, no connection state, no data flowing through studio.

### Mobile client flow

On login:

1. Authenticate with studio (OAuth / token)
2. `GET /v0/lfds` — fetch list of published lfds
3. If one lfd → auto-connect; if multiple → show picker; if none → show instructions
4. Connect directly to lfd's Tailscale address

Discovery resolves to a concrete `ServerConnection` (host, port, useTLS, authMode, token) — the same struct used for manual connections. No new enum cases or connection types. The distinction is in how the connection parameters are obtained (user-entered vs. studio-discovered), not how they're used.

### Prerequisites

Tailscale on both devices:
- Mac running lfd: Tailscale installed, logged in
- iPhone/iPad: Tailscale app installed, same tailnet

This is a real prerequisite — without Tailscale on both ends, remote connectivity doesn't work. The setup flow should detect this and guide the user:

```
┌─────────────────────────┐
│                         │
│   Your lfds             │
│                         │
│  ┌───────────────────┐  │
│  │  loopflow      ●  │  │  ← online, reachable
│  └───────────────────┘  │
│  ┌───────────────────┐  │
│  │  concerto      ○  │  │  ← online, unreachable
│  │  Not on Tailscale  │  │
│  └───────────────────┘  │
│                         │
│  Manual connection ›    │
│                         │
└─────────────────────────┘
```

### Auth flow on mobile

First launch:

```
┌─────────────────────────┐
│                         │
│     [loopflow logo]     │
│                         │
│   Sign in to discover   │
│   your running lfds     │
│                         │
│  ┌───────────────────┐  │
│  │  Sign in           │  │
│  └───────────────────┘  │
│                         │
│  Manual connection ›    │
│                         │
└─────────────────────────┘
```

After sign-in, auto-discovery replaces manual connection setup as the default path. Manual `host:port` entry stays for users not using studio or Tailscale.

## Security

- Discovery authenticates both sides: lfd proves ownership via account token, client proves identity via login
- Studio never stores lfd data — it's a live registry that empties when lfds disconnect
- Studio never sees lfd traffic — connections are direct, device to device
- Tailscale provides encryption and identity at the network layer
- lfd can reject connections (future: allowlist, approval flow)

## Constraints

- Direct connection (03-multi-client) must still work — discovery is additive
- lfd without `--publish` behaves exactly as before
- Tailscale is the connectivity layer — no studio relay service to build or operate
- No lfd API changes
- Studio discovery service is stateless (in-memory registry, no database)
- Discovery failure must degrade gracefully to manual connection, not block mobile usage

## Done when

- `lfd dev --publish` registers with studio, reports Tailscale address
- Mobile app: sign in → see list of running lfds → tap to connect directly
- Wave list, live output, and chat all work over Tailscale connection
- Disconnecting lfd removes it from discovery within 30s
- Mobile app shows clear guidance when Tailscale isn't set up
- Direct connection (manual host:port) still works as fallback
