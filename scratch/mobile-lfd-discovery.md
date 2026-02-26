# lfd Discovery via Auth

## Problem

Mobile requires manual entry of host, port, and token to connect to lfd. This is the dominant friction in the mobile experience — it turns a "check on your wave from the couch" moment into a "find the IP address, copy the token" chore. Users who run lfd with loopflow.studio auth already have an identity relationship with their daemons. Discovery should exploit that relationship to eliminate manual configuration entirely.

## Approach

Three layers, all additive to the existing manual connection path:

### 1. lfd registers its address and repos with studio (Rust)

Extend the existing `RegistrationClient` payload. `register()` and `send_heartbeat()` already fire every 30s — add `address` and `repos` to both.

**Address detection** (new module `rust/loopflow/src/lfd/address.rs`):
1. Shell out to `tailscale status --json`, parse `TailscaleIPs[0]` if available
2. Fall back to primary non-loopback network interface IP via `nix::ifaddrs`
3. Fall back to the configured bind address from `LFD_HTTP_ADDR`

Combine detected IP with the configured port for the final `address` string.

**Repo summary** — call `store.list_waves(None)` at registration time and on each heartbeat, group by repo, emit `[{name, wave_count}]`. The store is already available in the lfd startup path; thread a reference into the registration call sites.

Registration payload becomes:
```json
{
  "machine_id": "uuid",
  "machine_name": "hostname",
  "capabilities": ["waves", "terminal"],
  "address": "100.64.1.5:2486",
  "repos": [
    {"name": "loopflow", "wave_count": 3}
  ]
}
```

Heartbeat payload gets `address` and `repos` too (addresses change — DHCP, Tailscale reconnect).

### 2. DiscoveryService calls studio and returns typed results (Swift, LoopflowCore)

New `DiscoveryService` in LoopflowCore. Calls `GET /api/v1/daemons` with the JWT from `AuthService.currentToken()`. Returns `[DiscoveredDaemon]`.

```swift
struct DiscoveredDaemon: Codable, Identifiable {
    let machineId: String
    let machineName: String
    let address: String          // "100.64.1.5:2486"
    let status: String           // "online"
    let repos: [DaemonRepo]
    let connectionToken: String  // studio-issued, valid for lfd's validate-connection
    let lastHeartbeat: Date

    var id: String { machineId }
}

struct DaemonRepo: Codable {
    let name: String
    let waveCount: Int
}
```

`DiscoveryService` uses `AuthService` (existing) for JWT and the same `https://loopflow.studio` base URL. Token refresh is handled by `AuthService.refreshToken()` before the discovery call if the JWT is near expiry.

### 3. Discovery view as the new iOS entry point (Swift, Concerto/Platform/iOS)

New `DiscoveryView` replaces `ConnectionSetupView` as the initial screen when `repoTarget == nil`.

**State machine:**
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

**Reachability probing:** After loading the daemon list from studio, probe each daemon's address with a lightweight TCP connect (or HTTP GET to `/health` if the lfd exposes it). Update the UI progressively. Don't block the list on probing — show daemons immediately, then update reachability as results arrive.

**Connecting:** Tapping a daemon constructs a `ServerConnection` from the daemon's `address` (parsed into host + port) and `connectionToken`, then calls the existing `repoState.connect(to:outputBuffer:)`. Same handshake flow as manual connection. If the daemon has one repo, auto-select it. If multiple, the existing repo picker appears post-connect.

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

**No new lfd HTTP endpoints.** The existing `register`, `heartbeat`, and `validate-connection` endpoints on studio are sufficient. lfd's existing APIs are unchanged. The only Rust change is enriching the registration payload.

**AuthService owns JWT lifecycle.** `DiscoveryService` doesn't manage tokens — it calls `AuthService.currentToken()` and `AuthService.refreshToken()`. One owner for JWT storage, refresh, and expiry.

**StudioAuthStore wraps auth + discovery state.** Rather than scattering auth checks across views, a single `@Observable` store manages the state machine (signed out → signing in → signed in, daemon list loading → loaded → error). Views observe this store.

**Connection tokens are studio-issued, not stored.** Each `GET /api/v1/daemons` returns fresh `connection_token` values. The mobile client uses them immediately to connect and doesn't persist them — the existing `ConnectionSecretStore` stores the token per `connectionKey` (host:port) as it does today for manual connections. If the user force-quits and reopens, they re-discover and get fresh tokens.

## Scope

**In scope:**
- Rust: address detection module (Tailscale → network interface → bind address fallback)
- Rust: registration + heartbeat payload enrichment (address, repos)
- Rust: tests for address detection and payload changes
- Swift/LoopflowCore: `DiscoveredDaemon` model + `DiscoveryService`
- Swift/LoopflowCore: `StudioAuthStore` (auth + discovery state)
- Swift/iOS: `DiscoveryView` (sign in, daemon list, reachability, connect)
- Swift/iOS: `MobileRootView` wiring to show `DiscoveryView` on first launch
- Swift: tests for `DiscoveryService`, `StudioAuthStore`

**Out of scope:**
- Studio-side `GET /api/v1/daemons` implementation (separate repo, wave plan says "separate concern")
- Tailscale installation detection/prompting on iOS (future enhancement)
- macOS discovery (macOS uses bundled daemon — discovery is an iOS concern)
- Saved connections / favorites (if users want to bookmark a daemon)
- Multi-account support (one JWT = one user = one daemon list)
- Offline/cached daemon list (always fetch fresh)

## Done when

```bash
# Rust: address detection works
cargo test -p loopflow address

# Rust: registration payload includes address and repos
cargo test -p loopflow registration

# Swift: DiscoveryService decodes daemon list
swift test --package-path swift --filter DiscoveryServiceTests

# Swift: StudioAuthStore state machine
swift test --package-path swift --filter StudioAuthStoreTests

# Full suite green
cargo test --all && swift test --package-path swift
```

Observable outcomes:
- lfd registration with `loopflow.studio` auth sends `address` and `repos` in register + heartbeat payloads
- Opening Concerto iOS with no prior connection shows sign-in screen
- Signing in calls `GET /api/v1/daemons` and shows daemon list
- Tapping a daemon connects using studio-issued `connection_token`
- Wave list, live output, and chat all work over a discovered connection
- "Manual connection" link is always accessible from the discovery view
- Disconnecting lfd on the server side removes it from discovery within 30s (studio heartbeat timeout)

Advances wave goals: *"Zero-config discovery: lfd publishes to studio, mobile auto-discovers on login, connects via Tailscale"* and metric *"Login on mobile → see running lfds → tap to connect (no manual IP/port entry)"*.
