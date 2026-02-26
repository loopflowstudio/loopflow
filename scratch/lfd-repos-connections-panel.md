# Concerto Connections Panel

## Problem

Users can connect GitHub, Claude, and Codex through `lfq auth` in the terminal, but not from Concerto. Provider auth should be browser-first and identical across clients — click connect, finish auth, continue working. The API contract is already shipped (steps 1–2 of the lfd-repos wave). This is purely Swift client + UI work.

## Approach

Add auth provider cards to Concerto's existing connection settings views. Five layers, bottom-up:

1. **Models** — `AuthProvider`, `AuthProviderStatus`, `AuthFlow` in `LoopflowCore/Models/`
2. **Events** — `LFDEvent.auth(AuthEvent)` in `LocalEventService`
3. **Service** — auth HTTP methods on `LocalWaveService`
4. **State** — `AuthProviderStore` in `LoopflowCore/State/`
5. **Views** — `AuthProviderCard` component, integrated into `ConnectionSettingsView` (macOS) and `ConnectionSetupView` (iOS)

### Layer 1: Models

New file: `swift/LoopflowCore/Models/AuthProvider.swift`

```swift
public enum AuthProvider: String, Codable, Sendable, CaseIterable {
    case github
    case claude
    case codex

    public var displayName: String {
        switch self {
        case .github: "GitHub"
        case .claude: "Claude"
        case .codex: "Codex"
        }
    }

    public var icon: String {
        switch self {
        case .github: "terminal"         // SF Symbol
        case .claude: "brain.head.profile"
        case .codex: "cpu"
        }
    }
}

public enum AuthStatus: String, Codable, Sendable {
    case active
    case pending
    case none
    case expired
}

public struct AuthProviderStatus: Codable, Sendable, Identifiable {
    public let provider: AuthProvider
    public let status: AuthStatus
    public let login: String?

    public var id: AuthProvider { provider }
}

public struct AuthFlow: Codable, Sendable {
    public let provider: AuthProvider
    public let verification_uri: String
    public let verification_uri_complete: String?
    public let user_code: String?
    public let expires_in: UInt64?
}
```

JSON field names match the Rust DTOs exactly (snake_case) — `Codable` handles this with `keyDecodingStrategy: .convertFromSnakeCase` or matching field names directly.

### Layer 2: Events

Extend `LFDEvent` enum in `LocalEventService.swift`:

```swift
case auth(AuthEvent)

public struct AuthEvent: Sendable {
    public enum EventType: String, Sendable {
        case flowStarted = "auth.flow_started"
        case connected = "auth.connected"
        case failed = "auth.failed"
        case disconnected = "auth.disconnected"
    }

    public let type: EventType
    public let provider: AuthProvider
    public let login: String?       // only on .connected
    public let error: String?       // only on .failed
    public let timestamp: Date
}
```

Add parsing in `parseEvent(text:)` — the existing pattern switches on the event type string. Auth events all start with `"auth."`.

### Layer 3: Service

Add three methods to `LocalWaveService`:

```swift
// GET /v0/auth
public func listAuthProviders() async throws -> [AuthProviderStatus]

// POST /v0/auth/{provider}
public func startAuthFlow(provider: AuthProvider) async throws -> AuthFlow

// DELETE /v0/auth/{provider}
public func disconnectProvider(provider: AuthProvider) async throws -> AuthProviderStatus
```

These follow the existing `performRequest`/`makeRequest` pattern. Standard timeouts for list/disconnect, long timeout for start (the POST may wait for the provider CLI to launch).

### Layer 4: State

New file: `swift/LoopflowCore/State/AuthProviderStore.swift`

```swift
@MainActor
@Observable
public final class AuthProviderStore {
    public private(set) var providers: [AuthProvider: AuthProviderStatus]
    public private(set) var pendingFlow: AuthFlow?  // active flow in progress
    public private(set) var error: String?

    public var ordered: [AuthProviderStatus]  // computed, stable order

    public func refresh()           // GET /v0/auth, reconcile state
    public func connect(_ provider: AuthProvider)   // POST, open browser, set pending
    public func disconnect(_ provider: AuthProvider) // DELETE, update card
    public func handleEvent(_ event: AuthEvent)      // websocket reconciliation
}
```

Key behaviors:

- **Optimistic pending**: `connect()` sets local status to `.pending` *before* the POST returns. This prevents the race where the websocket `auth.connected` event arrives before the UI shows pending state.
- **Browser launch**: After POST returns, open `verification_uri_complete` (fallback `verification_uri`) via `NSWorkspace.shared.open()` (macOS) or `openURL` environment (iOS).
- **Event reconciliation**: `handleEvent` updates provider status from websocket events. `auth.connected` → `.active` with login. `auth.failed` → `.none` with error message. `auth.disconnected` → `.none`.
- **Refresh on reconnect**: When `LocalEventService` reconnects, call `refresh()` to reconcile any missed events.

Wire into `RepoState`:

```swift
public final class RepoState {
    public let authProviderStore = AuthProviderStore()
    // ...
}
```

In `RepoState.startEventSubscription`, add auth event handling alongside existing wave event handling.

### Layer 5: Views

New shared file: `swift/LoopflowCore/Views/AuthProviderCard.swift`

Each card shows one of three states:

**Disconnected (status: `.none` or `.expired`)**
```
┌─────────────────────────────────────┐
│  ⌘  GitHub                         │
│  Not connected                      │
│                        [Connect]    │
└─────────────────────────────────────┘
```

**Pending (status: `.pending`, flow active)**
```
┌─────────────────────────────────────┐
│  ⌘  GitHub              ● Pending  │
│  Enter code: ABCD-1234    [Copy]    │
│  Waiting for browser...             │
│                        [Cancel]     │
└─────────────────────────────────────┘
```

**Connected (status: `.active`)**
```
┌─────────────────────────────────────┐
│  ⌘  GitHub            ● Connected  │
│  octocat                            │
│                     [Disconnect]    │
└─────────────────────────────────────┘
```

Design token usage:
- Card: `palette.surface` background, `CornerRadius.lg`, `palette.border` stroke
- Status dot: `statusSuccess` (active), `statusWarning` (pending), `statusNeutral` (none)
- Provider name: `Typography.sectionTitle()`, `palette.text`
- Login/status text: `Typography.body()`, `palette.textSecondary`
- Connect button: `DarkButtonStyle`
- Disconnect button: `DestructiveButtonStyle`
- User code: `Typography.code()`, monospaced, with copy-to-clipboard button
- Hit targets: `HitTarget.comfortable` minimum (44pt on iOS)

**macOS integration**: Add a "Provider Connections" section below the existing daemon connection section in `ConnectionSettingsView`. Three cards stacked vertically.

**iOS integration**: Add a "Provider Connections" `Section` to the `ConnectionSetupView` `Form`. Same three cards adapted to form styling.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Separate "Providers" settings tab | Cleaner separation of concerns | Over-navigated — auth is part of connection setup, not a separate concept. Users shouldn't hunt for it. |
| Provider cards on the main dashboard | More visible, first-class | Dashboard real estate is for waves. Auth is setup, not ongoing work. Once connected, you don't look at it again. |
| AuthProviderStore merged into ConnectionStore | Fewer store objects | Provider auth state is independent of daemon connection. Mixing them couples unrelated lifecycles. |
| Polling instead of websocket events | Simpler implementation | Auth flow takes 5–30 seconds. Polling would either waste resources (fast interval) or feel sluggish (slow interval). Events give instant feedback. |

## Key decisions

1. **AuthProviderStore is separate from ConnectionStore.** Provider auth (GitHub/Claude/Codex tokens) and daemon connection (bundled/remote lfd) are independent concerns. A user can be connected to lfd but have no provider auth, or vice versa. Separate stores prevent coupling.

2. **Optimistic pending state.** The instant a user clicks Connect, the card flips to pending *before* the POST returns. This eliminates the race where `auth.connected` arrives via websocket before the HTTP response. The POST response only matters for extracting the `user_code` and `verification_uri`.

3. **User code is prominent with copy button.** Device auth flows (GitHub, Codex) require entering a code in the browser. The code gets `Typography.code()` styling and a dedicated copy-to-clipboard button. Users shouldn't have to squint or manually select text.

4. **Refresh on every reconnect.** When the websocket reconnects (after network loss, sleep, lfd restart), immediately `GET /v0/auth` to reconcile. Events missed during disconnection could leave the UI stale. This is the same pattern used for wave state reconciliation.

5. **Extend existing views, don't create new ones.** Provider cards go in the existing ConnectionSettingsView/ConnectionSetupView. Auth is part of "connecting Concerto to services" — the same mental model as connecting to lfd. No new navigation, no new screens.

6. **AuthProviderCard is a shared component in LoopflowCore.** One implementation, used in both macOS and iOS views. Platform-specific URL opening handled via environment (`openURL`).

7. **409 (already pending) shows the existing pending state.** If a user clicks Connect while a flow is already pending, the service returns 409. The store catches this and shows the existing pending card rather than an error. The user sees continuity, not failure.

## Failure modes

| Failure | Handling |
|---------|----------|
| Provider binary missing (`gh`/`claude`/`codex` not installed) | POST returns an error with "command not available" — display in card as descriptive message with install hint |
| Flow already pending (409) | Catch 409, treat as no-op, keep showing pending state |
| Browser launch failure | Show copyable URL as fallback — `verification_uri_complete` or `verification_uri` displayed inline |
| lfd reconnect mid-flow | `refresh()` on reconnect reconciles state from `GET /v0/auth` |
| Auth flow timeout (user never completes browser auth) | `auth.failed` event arrives when `expires_in` elapses — card returns to disconnected with "Flow expired" message |
| Provider returns expired status | Show "Expired" state with "Reconnect" button (same as Connect) |

## Scope

**In scope:**
- Shared auth models in `LoopflowCore`
- Auth event parsing in `LocalEventService`
- Auth HTTP methods on `LocalWaveService`
- `AuthProviderStore` state container
- `AuthProviderCard` shared view component
- Integration into macOS `ConnectionSettingsView`
- Integration into iOS `ConnectionSetupView`
- Tests for models, event parsing, and store logic

**Out of scope:**
- Server-side auth API changes (already shipped)
- Repo onboarding (`POST /v0/repos`) — that's step 4
- Provider auto-detection (checking if `gh`/`claude`/`codex` are installed proactively)
- Token display or management in Concerto (tokens are lfd's concern)
- Custom provider support (only GitHub, Claude, Codex)

## Implementation order

1. Models (`AuthProvider.swift`) — no dependencies, testable immediately
2. Event parsing (`LocalEventService` extension) — depends on models
3. Service methods (`LocalWaveService` extension) — depends on models
4. Store (`AuthProviderStore`) — depends on models + service
5. Views (`AuthProviderCard` + view integration) — depends on store
6. Wire into `RepoState` event loop — depends on store + events

Each layer is independently testable. Layers 1–3 can be implemented in parallel.

## Done when

- Users can connect/disconnect GitHub, Claude, and Codex from Concerto with no terminal steps.
- Card state tracks `active` / `pending` / `none` from HTTP + websocket events.
- Pending state shows user code with copy button when applicable.
- Auth state refreshes on websocket reconnect.
- `cargo test --all` and `swift test --package-path swift` pass.

Wave goals advanced: *"Concerto Connections panel wired to `/v0/auth` + auth events"* (step 3, lfd-repos wave).
