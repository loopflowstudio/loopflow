# 04: lfd Discovery via Auth

Login on mobile, see your running lfds. Tailscale handles connectivity. Zero config.

## What to build

lfd registers with studio auth servers automatically when using the `loopflow.studio` auth provider. Registration = discoverable — no `--publish` flag needed. Studio is a discovery service that tracks which lfds are online and their addresses. When you login on mobile, Concerto discovers your lfds via studio and connects directly.

No relay service. Tailscale handles NAT traversal for remote access. Studio just answers "where are my lfds?"

Stage 04 is additive on top of direct/manual connection from Stage 01/03. If discovery is unavailable, manual host:port must still work.

## How it works

### Registration (lfd side)

lfd already registers with `auth.loopflow.studio`, heartbeats every 30s, deregisters on shutdown. Two additions to the existing payload:

1. **`address`** — detected host:port for direct connections
2. **`repos`** — list of `{ name, wave_count }` so discovery can show repo names without connecting

Address detection order:
1. `tailscale status --json` → extract `TailscaleIPs[0]` if Tailscale is running
2. Fall back to primary non-loopback network interface IP
3. Fall back to bind address from config

Both `address` and `repos` are included in heartbeats too (addresses can change — DHCP, Tailscale reconnect).

Tailscale is the expected connectivity layer for remote access. Without Tailscale, LAN address works on the same network but not remotely.

lfd's existing HTTP + WebSocket APIs work unchanged. No new lfd API surface. No new CLI flags.

### Discovery service (studio side)

Studio needs one new endpoint. Studio-side implementation is a separate concern — this is the contract.

**`GET /api/v1/daemons`** — returns registered lfds for the authenticated user.

```
Authorization: Bearer <user JWT>
```

```json
{
  "daemons": [
    {
      "machine_id": "abc-123",
      "machine_name": "jacks-macbook",
      "address": "100.64.1.5:2486",
      "status": "online",
      "repos": [
        { "name": "loopflow", "wave_count": 3 },
        { "name": "concerto", "wave_count": 1 }
      ],
      "connection_token": "ct_xxxxxxxxxxxx",
      "last_heartbeat": "2026-02-26T18:45:00Z"
    }
  ]
}
```

`connection_token` is studio-issued and valid for lfd's `validate-connection` endpoint. Studio generates it per-request for the authenticated user. Same validation path that lfd already implements via `ConnectionValidator`.

This is a thin service — a table of heartbeats behind an auth check. No WebSocket proxying, no connection state, no data flowing through studio.

### Mobile client flow

On login:

1. `ASWebAuthenticationSession` opens `https://auth.loopflow.studio/oauth/authorize?...` in an in-app browser sheet
2. Studio redirects to `loopflow://auth/callback?token=<jwt>` — token stored in Keychain
3. `GET /api/v1/daemons` with JWT — fetch list of registered lfds
4. If one lfd → auto-connect; if multiple → show picker; if none → show instructions
5. Connect directly to lfd's address using studio-issued `connection_token`

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
- If lfd doesn't use `loopflow.studio` auth, it's not discoverable — manual connection still works
- Tailscale is the connectivity layer for remote — no studio relay service to build or operate
- No lfd API changes — existing registration/heartbeat payloads get two new fields
- Studio discovery service is stateless (in-memory registry, no database)
- Discovery failure must degrade gracefully to manual connection, not block mobile usage

## Done when

- lfd registration payload includes address (Tailscale-preferred) and repo summaries
- lfd heartbeat updates address and repo summaries
- `StudioAuthStore` handles OAuth sign-in via ASWebAuthenticationSession
- `DiscoveryService` calls `GET /api/v1/daemons` and returns typed results
- Mobile app: sign in → see list of running lfds → tap to connect directly
- Tapping a discovered lfd connects using studio-issued connection token
- Wave list, live output, and chat all work over a discovered connection
- Disconnecting lfd removes it from discovery within 30s
- Direct connection (manual host:port) still works as fallback
