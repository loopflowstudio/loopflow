# Concerto Mobile Direction

Mobile access to loopflow agents. Non-interactive first, then chat, then full agent.

## Problem

Users want to manage waves from their phone. Mobile users (conductor persona) want status and actions, not terminal sessions. Current macOS app requires local lfd connection—mobile needs remote access.

## Approach

Build Phase A (non-interactive mobile) as a remote control for lfd. No conversation, no terminal. Just status and actions.

**What mobile can do:**
- See wave status (running, waiting, idle)
- Trigger non-interactive steps/flows
- Land PRs
- Create waves
- See results (what changed, PR link)

**What mobile explicitly cannot do:**
- Run interactive steps (those require terminal)
- Chat with LLM (Phase B)
- Execute agent tools (Phase C)

---

## Architecture

```
┌─────────────┐     ┌─────────────────┐     ┌─────────────┐
│  iOS App    │────►│  loopflow.studio │────►│  Your lfd   │
│             │     │  (JWT auth +     │     │  (macOS)    │
│  Concerto   │     │   lfd discovery) │     │             │
└─────────────┘     └─────────────────┘     └─────────────┘
```

**Discovery flow:**
1. User signs in via loopflow.studio (same as macOS: GitHub/Google/Apple)
2. Their lfd registers with loopflow.studio on startup (already implemented in `registration.rs`)
3. iOS app calls loopflow.studio to discover registered lfd endpoints
4. iOS app connects directly to lfd via HTTP API

**Why this architecture:**
- Single auth system (loopflow.studio JWT works for both apps)
- lfd already has registration/heartbeat infrastructure
- No NAT traversal complexity—lfd is on a network-reachable machine
- Fallback: user can manually enter lfd URL if discovery fails

---

## Key Decisions

### 1. HTTP API only (no gRPC on mobile initially)

The lfd already exposes HTTP at `:2486` for status/metrics. Extend this to cover wave operations.

**Why HTTP over gRPC:**
- URLSession works great on iOS, grpc-swift adds complexity
- Wave operations are request/response, not streaming
- Can add gRPC later for real-time event subscriptions if needed

**What needs to be added to HTTP API:**
- POST `/waves` - create wave (already via LocalWaveService JSON body)
- GET `/waves` - list waves with worktree state
- PATCH `/waves/:id` - update wave config
- DELETE `/waves/:id` - delete wave
- POST `/waves/:id/run` - run wave
- POST `/waves/:id/stop` - stop wave
- POST `/waves/:id/collapse` - collapse PRs

The LocalWaveService already uses all these endpoints. The HTTP layer exists.

### 2. RemoteWaveService uses loopflow.studio for discovery

```swift
public final class RemoteWaveService: WaveServiceProtocol {
    private let tokenProvider: TokenProvider
    private var lfdBaseURL: URL?

    /// Discover user's lfd via loopflow.studio
    public func discoverLfd() async throws {
        let jwt = try await tokenProvider.token()
        let discoveryURL = URL(string: "https://loopflow.studio/api/v1/daemons/discover")!
        var request = URLRequest(url: discoveryURL)
        request.setValue("Bearer \(jwt)", forHTTPHeaderField: "Authorization")

        let (data, _) = try await URLSession.shared.data(for: request)
        let response = try JSONDecoder().decode(DiscoveryResponse.self, from: data)
        self.lfdBaseURL = URL(string: response.lfd_url)
    }

    // Same methods as LocalWaveService, but uses discovered lfdBaseURL
    // and adds Authorization header with JWT
}
```

### 3. Shared SwiftUI components between macOS and iOS

Add iOS platform to Package.swift:

```swift
platforms: [
    .macOS(.v15),
    .iOS(.v17)
]
```

Views that work cross-platform:
- `WaveSidebar` (NavigationSplitView works on both)
- `WaveRow`
- `WaveDetailPanel`
- `DirectionPills`
- `FlowPicker`

Views that are macOS-only:
- `GhosttyTerminalView` (no Ghostty on iOS)
- `EmbeddedTerminalPanel`
- `InteractiveSessionView`

### 4. Auth stays in Keychain, shared via app group

```swift
// Use app group for shared Keychain access
let keychainService = "studio.loopflow.auth"
let accessGroup = "group.studio.loopflow"  // Share between macOS and iOS
```

The AuthService already stores JWT in Keychain. Adding an access group makes it work across the app family.

### 5. Polling initially, push notifications later

Start with 30-second polling for wave status:

```swift
Timer.publish(every: 30, on: .main, in: .common)
    .autoconnect()
    .sink { _ in await refreshWaves() }
```

Push notifications require:
- APNS setup in loopflow.studio
- Device token registration on iOS app launch
- Server-side logic to push when wave needs attention

Defer push to after MVP works with polling.

---

## Scope

### In scope (Phase A)

- iOS target in Swift package
- RemoteWaveService with loopflow.studio discovery
- Shared views adapted for touch (larger tap targets)
- Wave list grouped by attention state
- Wave detail with status and actions
- Land PR, Run Step buttons
- Create wave flow

### Out of scope

- Terminal streaming (Phase C, if ever)
- Chat interface (Phase B)
- Interactive step execution (requires terminal)
- Push notifications (post-MVP)
- iPad-specific layouts (follow-on work)
- Offline mode (waves require live lfd connection)

---

## Alternatives Considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| gRPC on mobile | Lower latency for real-time updates | Adds grpc-swift complexity; HTTP adequate for Phase A operations |
| Direct lfd connection without discovery | Simpler if lfd is on same network | Doesn't work when mobile is on cellular; discovery enables remote access |
| Tunnel via loopflow.studio (relay) | Works through NAT | Adds latency and server load; direct connection is faster |
| React Native for cross-platform | Single codebase for Android | Loses SwiftUI polish; Android can wait |

---

## Done When

1. iOS app builds and runs on iPhone simulator
2. User can sign in via loopflow.studio
3. App discovers and connects to user's lfd
4. Wave list shows correct status (running/waiting/idle)
5. User can tap "Land PR" and it lands
6. User can tap "Run Step" and it triggers

Verification: conductor persona can check wave status and land PRs from iPhone without opening laptop.
