# Studio Relay

**Finish line:** A phone on cellular can reach lfd on a home workstation through loopflow.studio acting as a WebSocket relay.

## Context

lfd listens on localhost. The phone can't reach it. Studio sits in the middle as a dumb byte-forwarding pipe. Cloudflare's existing proxy (already used for the website) gives edge TLS termination — the phone's handshake happens at the nearest PoP.

```
Phone → CF PoP (edge TLS) → CF backbone → Studio (relay) → lfd
```

## What to build

### Studio side (~400 lines, Studio codebase)

**Relay endpoint:** `wss://loopflow.studio/relay/{daemon_id}`

Two roles connect to the same endpoint:
- **Daemon** connects with `role=daemon` query param. Studio holds this socket
- **Client** connects with `role=client`. Studio pairs it with the daemon socket for this `daemon_id`

Studio forwards bytes transparently between paired sockets. It does not inspect, authenticate, or modify traffic. Auth is between the phone and lfd — Studio just pipes.

One daemon socket per `daemon_id`. Multiple client sockets per daemon (one per device). Each client gets its own pairing with the daemon socket.

When the daemon disconnects, Studio closes all paired client sockets. When a client disconnects, Studio notifies the daemon (optional — lfd can detect via its own ping/pong).

### lfd side (~800 lines Rust)

**Relay client.** On startup (if configured), lfd opens `wss://loopflow.studio/relay/{daemon_id}` with `role=daemon`. Keeps alive with 30s pings. Auto-reconnects with exponential backoff on drop.

**QR pairing.** `lf ops relay pair` mints a connection token via the existing token ledger, encodes a QR code to terminal:

```json
{ "relay": "wss://loopflow.studio/relay", "daemon_id": "abc", "token": "a4f8..." }
```

Same QR available in Concerto desktop menu bar.

**Token handling.** When a client connects through the relay, its first message includes `Bearer {token}`. lfd validates against its local token ledger (constant-time comparison, same as direct connections). The relay is transparent — lfd sees the same auth flow as a local client.

**Token refresh.** Before a mobile token expires, the phone sends a refresh request through the relay. lfd validates the current token, mints a replacement with fresh TTL, responds inline. Phone stores the new token. Old token is revoked.

**Configuration:**

```yaml
# ~/.lf/lfd.yaml
relay:
  enabled: true
  url: wss://loopflow.studio/relay
```

`daemon_id` derived from the existing machine ID used for Studio registration.

## Constraints

- Studio relay is a dumb pipe. Zero knowledge of lfd's protocol, auth tokens, or message content
- The phone speaks the exact same WebSocket protocol as local Concerto — no relay-awareness in LoopflowCore's networking layer
- Token TTL for mobile pairing: 24 hours default, auto-refreshed at 50% lifetime
- Relay connection is always-on when enabled. No on-demand connection setup

## Done when

- lfd connects to Studio relay on startup
- Phone connects to the same relay endpoint and reaches lfd
- QR code pairing works from terminal and Concerto desktop
- Token auto-refresh keeps the phone connected across days
- `lf ops token revoke` immediately disconnects a paired phone
- Connection survives wifi→cellular transitions (reconnect within seconds)
