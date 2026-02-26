# lfd Discovery via Auth

## Problem

Mobile requires manual entry of host, port, and token to connect to lfd. This is the dominant friction in the mobile experience — it turns a "check on your wave from the couch" moment into a "find the IP address, copy the token" chore. Users who run lfd with loopflow.studio auth already have an identity relationship with their daemons. Discovery should exploit that relationship to eliminate manual configuration entirely.

## Delivery split

Three PRs, each independently shippable:

| PR | Repo | What ships | Depends on |
|----|------|-----------|------------|
| **1. Rust payload enrichment** | loopflow.mobile | Address detection, `url` + `repos` in register/heartbeat | Nothing |
| **2. Studio discovery API** | studio | `repos` on Daemon model, `connection_token` minting in discover, `validate-connection` endpoint | Nothing (but PR 1 sends the data it stores) |
| **3. Swift discovery UI** | loopflow.mobile | DiscoveryService, DiscoveryView, MobileRootView wiring | PR 2 (needs the endpoint) |

Studio draft lives in worktree `studio/.claude/worktrees/daemon-discovery/` on branch `daemon-discovery-api`.

## Approach

Three layers, all additive to the existing manual connection path:

### 1. lfd registers its url and repos with studio (Rust) — PR 1

Extend the existing `RegistrationClient` payload. `register()` and `send_heartbeat()` already fire every 30s — add `url` and `repos` to both.

**Address detection** (new module `rust/loopflow/src/lfd/address.rs`):
1. Query Tailscale local API (`/localapi/v0/status`) via Unix socket (Linux) or TCP+password (macOS) — parse `TailscaleIPs[0]`. Use `tailscale-localapi` crate or minimal direct hyper request (lfd already depends on hyper). Avoids shelling out to `tailscale` CLI which is fragile (PATH, version skew, subprocess overhead).
2. Fall back to primary non-loopback network interface IP via `nix::ifaddrs`
3. Fall back to the configured bind address from `LFD_HTTP_ADDR`

Combine detected IP with the configured port for the final `url` string (e.g. `http://100.64.1.5:2486`). Note: Tailscale LocalAPI is undocumented and may change — the fallback chain ensures this isn't a hard dependency.

**Repo summary** — call `store.list_waves(None)` at registration time and on each heartbeat, group by repo, emit `[{name, wave_count}]`. The store is already available in the lfd startup path; thread a reference into the registration call sites.

Registration payload becomes:
```json
{
  "machine_id": "uuid",
  "machine_name": "hostname",
  "capabilities": ["waves", "terminal"],
  "url": "http://100.64.1.5:2486",
  "repos": [
    {"name": "loopflow", "wave_count": 3}
  ]
}
```

Heartbeat payload gets `url` and `repos` too (addresses change — DHCP, Tailscale reconnect).

### 2. Studio discovery API — PR 2

Studio already has `POST /api/v1/daemons/register`, `POST /api/v1/daemons/heartbeat`, `POST /api/v1/daemons/deregister`, and `GET /api/v1/daemons/discover`. Changes needed:

- **Daemon model**: add `repos` JSON column
- **Register**: accept `repos` field (list of `{name, wave_count}`)
- **Heartbeat**: accept optional `url` and `repos` (so address/repo changes propagate without re-registering)
- **Discover**: return `capabilities`, `repos`, and a freshly-minted `connection_token` per daemon
- **New endpoint**: `POST /api/v1/daemons/validate-connection` — lfd calls this to validate a connection token presented by mobile. No JWT required (the token itself is the credential). Returns `{valid, user_id, email}`.

Connection tokens are HMAC-signed with the same `STATE_SIGNING_SECRET`, encoding `{uid, mid, exp}` with 10-minute expiry. Short-lived, never stored.

Draft implementation with passing tests in `studio/.claude/worktrees/daemon-discovery/`.

### 3. DiscoveryService + DiscoveryView (Swift) — PR 3

New `DiscoveryService` in LoopflowCore. Calls `GET /api/v1/daemons/discover` with the JWT from `AuthService.currentToken()`. Returns `[DiscoveredDaemon]`.

```swift
struct DiscoveredDaemon: Codable, Identifiable {
    let machineId: String
    let machineName: String?
    let url: String?              // "http://100.64.1.5:2486"
    let capabilities: [String]
    let repos: [DaemonRepo]
    let connectionToken: String   // studio-issued, 10min expiry
    let lastHeartbeat: Date?

    var id: String { machineId }
}

struct DaemonRepo: Codable {
    let name: String
    let waveCount: Int
}
```

`DiscoveryService` uses `AuthService` (existing) for JWT and the same `https://loopflow.studio` base URL. Token refresh is handled by `AuthService.refreshToken()` before the discovery call if the JWT is near expiry.

New `DiscoveryView` replaces `ConnectionSetupView` as the initial screen when `repoTarget == nil`.

**State machine (view-local, not app state):**
```
signedOut → signingIn → discovering → daemonList
                ↓                        ↓
             signedOut              connecting → connected
                                        ↓
                                      error → daemonList
```

**Unauthenticated state:**
- Loopflow logo
- "Sign in to discover your running lfds"
- [Sign in] button → `AuthService.signIn()` via ASWebAuthenticationSession
- "Manual connection ›" link → `ConnectionSetupView` (pushed in nav stack)

**Authenticated, loading:**
- Spinner + "Looking for your lfds…"

**Authenticated, daemons found:**
- List of daemon cards. Each card shows:
  - Machine name (e.g. "jacks-macbook")
  - Repo names with wave counts (e.g. "loopflow (3), concerto (1)")
  - Status indicator: green dot (reachable), gray dot (unreachable), spinner (checking)
  - Tap to connect
- Pull-to-refresh
- "Manual connection ›" link at bottom
- Sign out in toolbar

**Authenticated, no daemons:**
- "No lfds found"
- Instructions: "Start lfd on your Mac with loopflow.studio auth to see it here"
- "Manual connection ›"

**Auto-connect:** If exactly one daemon is online and reachable, connect automatically. Skip the picker.

**Reachability probing:** After loading the daemon list from studio, probe each daemon's url with HTTP GET to `/health`. Update the UI progressively. Don't block the list on probing — show daemons immediately, then update reachability as results arrive.

**Connecting:** Tapping a daemon parses `url` into host + port, constructs a `ServerConnection` with the `connectionToken`, then calls the existing `repoState.connect(to:outputBuffer:)`. Same handshake flow as manual connection. If the daemon has one repo, auto-select it. If multiple, the existing repo picker appears post-connect.

### Navigation changes

`MobileRootView.needsInitialSetup` remains the gate. When true, show `DiscoveryView` instead of `ConnectionSetupView`. The Settings tab (tab 1) keeps `ConnectionSetupView` for reconfiguring connections. `DiscoveryView` pushes `ConnectionSetupView` via navigation for manual fallback.

No changes to iPad layout logic — same view swap applies.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Studio as relay (proxy all traffic) | No Tailscale needed, but all data flows through studio | Privacy, latency, cost, operational burden. Wave plan explicitly rejects this. |
| mDNS/Bonjour local discovery | Works on LAN without Tailscale | Doesn't solve the primary use case (remote access). Could add later as a complementary feature. |
| Push notifications for daemon online/offline | Real-time updates without polling | Over-engineered for v1. APNs infrastructure is complex. Pull model is simpler. |
| QR code pairing (lfd shows QR, phone scans) | No studio dependency | Requires physical proximity and a second screen. Doesn't survive across sessions. |

## Key decisions

**Discovery is additive, never replacing manual.** Both paths stay first-class. If studio is down, manual host:port works. If Tailscale isn't installed, manual works. Discovery is the happy path, not the only path.

**Reachability probing is async and non-blocking.** Show the daemon list immediately from studio's response. Probe reachability in the background. A daemon that's "online" per studio but unreachable (no Tailscale, wrong network) shows a gray dot with a hint, not a disabled row — the user can still tap and see the connection error.

**`url` not `address`.** Studio already uses `url` on the Daemon model. Keeps the field name consistent across Rust registration, studio storage, and Swift models. Carries scheme info for TLS detection.

**AuthService owns JWT lifecycle.** `DiscoveryService` doesn't manage tokens — it calls `AuthService.currentToken()` and `AuthService.refreshToken()`. One owner for JWT storage, refresh, and expiry.

**View-level state machine, not an app store.** The discovery state machine (signed out → discovering → daemonList → connecting) is view state owned by `DiscoveryView` via `@State`/local `@Observable`. No new app-level store — `AuthService` owns JWT lifecycle, `DiscoveryService` is a stateless network service, and the view composes them.

**Connection tokens are studio-issued, short-lived, not stored.** Each `GET /api/v1/daemons/discover` returns fresh `connection_token` values (10min HMAC-signed). The mobile client uses them immediately to connect and doesn't persist them. lfd validates them by calling studio's `validate-connection` endpoint. If the user force-quits and reopens, they re-discover and get fresh tokens.

**Heartbeat carries url and repos.** Addresses change (DHCP, Tailscale reconnect) and repo counts change. Heartbeat already fires every 30s — piggyback updated data rather than requiring re-registration.

## Scope per PR

### PR 1: Rust payload enrichment (this repo)
- Address detection module (Tailscale local API → network interface → bind address fallback)
- Registration + heartbeat payload enrichment (`url`, `repos`)
- Tests for address detection and payload changes

### PR 2: Studio discovery API (studio repo)
- `repos` JSON column on Daemon model
- Register accepts `repos`; heartbeat accepts `url` + `repos`
- Discover returns `capabilities`, `repos`, `connection_token`
- `validate-connection` endpoint
- Tests (draft already passing in worktree)

### PR 3: Swift discovery UI (this repo)
- `DiscoveredDaemon` model + `DiscoveryService` in LoopflowCore
- `DiscoveryView` (sign in, daemon list, reachability, connect)
- `MobileRootView` wiring to show `DiscoveryView` on first launch
- Tests for `DiscoveryService`

## Done when

### PR 1
```bash
cargo test -p loopflow address
cargo test -p loopflow registration
cargo test --all
```

### PR 2
```bash
cd studio && uv run pytest auth/tests/ -v
```

### PR 3
```bash
swift test --package-path swift --filter DiscoveryServiceTests
swift test --package-path swift
```

### End-to-end (all three merged)
- lfd registration sends `url` and `repos` in register + heartbeat payloads
- Opening Concerto iOS with no prior connection shows sign-in screen
- Signing in calls `GET /api/v1/daemons/discover` and shows daemon list
- Tapping a daemon connects using studio-issued `connection_token`
- Wave list, live output, and chat all work over a discovered connection
- "Manual connection" link is always accessible from the discovery view
- Disconnecting lfd removes it from discovery within 30s (studio heartbeat timeout)

Advances wave goals: *"Zero-config discovery: lfd publishes to studio, mobile auto-discovers on login, connects via Tailscale"* and metric *"Login on mobile → see running lfds → tap to connect (no manual IP/port entry)"*.
