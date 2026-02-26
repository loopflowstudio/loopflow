# Concerto Connections Panel

## Problem

Users can connect GitHub, Claude, and Codex through `lfq auth` in terminal, but not from Concerto. Provider auth should be browser-first and consistent across clients: click connect, finish auth, continue working.

Step 1–2 server/CLI contract is already shipped. This step is Swift client + UI only.

## Human-reviewed scope choice (Feb 26, 2026)

**Selected package: Do it all.**

Ship full scope now, including correctness hardening and UX polish, not just minimal wiring.

## Approach

Add provider auth cards to existing connection settings surfaces using five layers:

1. **Models** — auth provider/status/flow models in `LoopflowCore/Models/`
2. **Events** — `LFDEvent.auth(AuthEvent)` in `LocalEventService`
3. **Service** — `/v0/auth` methods on `LocalWaveService`
4. **State** — `AuthProviderStore` in `LoopflowCore/State/`
5. **Views** — shared `AuthProviderCard` integrated into macOS/iOS connection screens

---

## Layer 1: Models

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
        case .github: "terminal"
        case .claude: "brain.head.profile"
        case .codex: "cpu"
        }
    }
}

public enum ProviderAuthStatus: String, Codable, Sendable {
    case active
    case pending
    case none
    case expired
}

public struct AuthProviderStatus: Codable, Sendable, Identifiable {
    public let provider: AuthProvider
    public let status: ProviderAuthStatus
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

public struct AuthProviderListResponse: Codable, Sendable {
    public let providers: [AuthProviderStatus]
}
```

Notes:
- Keep snake_case field names to match Rust DTOs exactly.
- `/v0/auth` decodes `AuthProviderListResponse` (`{ providers: [...] }`), not raw array.

---

## Layer 2: Events

Extend `LFDEvent` in `LocalEventService.swift`:

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

    // flow_started payload
    public let verificationURI: String?
    public let verificationURIComplete: String?

    // connected payload
    public let login: String?

    // failed payload
    public let error: String?

    public let timestamp: Date
}
```

Parsing requirements:
- Parse dotted event names (`"auth.flow_started"`, etc.)
- Include flow URLs from `auth.flow_started`
- Keep unknown auth payload fields ignored

---

## Layer 3: Service

Add auth methods to `LocalWaveService`:

```swift
// GET /v0/auth -> { providers: [...] }
public func listAuthProviders() async throws -> [AuthProviderStatus]

// GET /v0/auth/{provider}
public func getAuthProvider(provider: AuthProvider) async throws -> AuthProviderStatus

// POST /v0/auth/{provider}
public func startAuthFlow(provider: AuthProvider) async throws -> AuthFlow

// DELETE /v0/auth/{provider}
public func disconnectProvider(provider: AuthProvider) async throws -> AuthProviderStatus
```

Timeout policy:
- list/get/disconnect: standard timeouts
- start: long timeout (provider CLI launch can block)

Error policy:
- Preserve status code from existing `WaveServiceError.serverError(status:message:)`
- Store handles `409` specially (refresh and continue pending)

---

## Layer 4: State

New file: `swift/LoopflowCore/State/AuthProviderStore.swift`

```swift
@MainActor
@Observable
public final class AuthProviderStore {
    public struct BrowserLaunchRequest: Sendable, Equatable {
        public let provider: AuthProvider
        public let url: URL
    }

    public private(set) var providers: [AuthProvider: AuthProviderStatus] = [:]
    public private(set) var pendingFlows: [AuthProvider: AuthFlow] = [:]
    public private(set) var browserLaunchRequest: BrowserLaunchRequest?
    public private(set) var error: String?

    public var ordered: [AuthProviderStatus] { ... } // stable AuthProvider.allCases order

    public func bindService(_ waveService: LocalWaveService)
    public func refresh() async
    public func connect(_ provider: AuthProvider) async
    public func disconnect(_ provider: AuthProvider) async
    public func handleEvent(_ event: AuthEvent)
    public func handleConnectionState(_ state: ConnectionState) async
    public func consumeBrowserLaunchRequest() -> BrowserLaunchRequest?
}
```

Key behaviors:

- **Optimistic pending (per provider):** `connect(provider)` sets that provider to `.pending` immediately.
- **Pending flow map (not singleton):** multiple providers can be pending simultaneously.
- **No platform side effects in store:** store emits `browserLaunchRequest`; view performs `openURL`/`NSWorkspace`.
- **`auth.flow_started` reconciliation:** update provider to `.pending`; cache flow URL in `pendingFlows` even when event originated from another client.
- **`auth.connected`:** set `.active`, set `login`, clear pending flow.
- **`auth.failed`:** set `.none`, clear pending flow, set user-visible error.
- **`auth.disconnected`:** set `.none`, clear pending flow.
- **`POST` 409:** call `refresh()`, keep pending state, no fatal error.
- **Reconnect reconciliation:** when connection state transitions to `.connected`, call `refresh()`.

RepoState wiring:

```swift
public final class RepoState {
    public let authProviderStore = AuthProviderStore()
}
```

- Rebind `authProviderStore` whenever services are rebuilt for a new connection/token.
- Forward `.auth` events in `startEventSubscription`.
- Forward `ConnectionState` updates into `authProviderStore.handleConnectionState`.

---

## Layer 5: Views

New shared file: `swift/LoopflowCore/Views/AuthProviderCard.swift`

States:

1. **Disconnected** (`none`/`expired`) — “Not connected” + Connect/Reconnect button
2. **Pending with flow details** — status dot + code (if present) + copy code + waiting text + Cancel
3. **Pending without flow details** — status dot + “Auth pending (possibly started from another client)” + Cancel
4. **Connected** (`active`) — status dot + login + Disconnect

Design tokens:
- Card: `palette.surface`, `CornerRadius.lg`, `palette.border`
- Status dots: success/warning/neutral
- Typography: section/body/code
- Buttons: `DarkButtonStyle`, `DestructiveButtonStyle`
- Hit targets: desktop `HitTarget.comfortable`, iOS `HitTarget.touch`

Copy behavior:
- macOS: `NSPasteboard`
- iOS: `UIPasteboard`
- Copy button has accessibility label/hint

URL launch behavior:
- View observes store `browserLaunchRequest`
- View opens URL via `openURL` (iOS) / `NSWorkspace.shared.open` (macOS wrapper)
- If open fails, card shows inline copyable URL fallback

Integrations:
- macOS: add “Provider Connections” section below daemon connection in `ConnectionSettingsView`
- iOS: add “Provider Connections” section in `ConnectionSetupView` form and settings tab
- If lfd is disconnected, section remains visible but disabled with “Connect to server first” guidance

---

## Key decisions (updated after review)

1. **Separate store remains correct.** Provider auth lifecycle is independent of daemon connection lifecycle.
2. **Pending is per provider, never global.** Avoid state loss when more than one flow is pending.
3. **`auth.flow_started` is first-class.** Without it, multi-client reconciliation is stale.
4. **Store is state-only.** Browser open is UI/platform work.
5. **Refresh on reconnect is mandatory.** WS “connected” snapshot includes waves, not auth provider state.
6. **409 is continuity, not failure.** Refresh + continue pending UI.

---

## Failure modes

| Failure | Handling |
|---------|----------|
| `gh`/`claude`/`codex` missing | Show provider-specific install hint from server message |
| Flow already pending (409) | Refresh auth providers, preserve pending state |
| Browser open fails | Keep pending state and show copyable URL |
| Reconnect mid-flow | Refresh `/v0/auth` on `.connected` |
| Flow started from another client | Pending card appears from `auth.flow_started` or refresh |
| Flow timeout/deny | `auth.failed` transitions back to disconnected with error |
| Provider status expired | Show expired + reconnect action |

---

## Scope

**In scope now (full package):**
- Shared auth models + DTO wrapper
- Auth event parsing (`auth.*`, including dotted names)
- Auth HTTP methods (`list/get/start/disconnect`)
- `AuthProviderStore` with per-provider pending flows + reconnect reconciliation
- Shared `AuthProviderCard` with copy/open fallback UX
- macOS + iOS connection screen integration
- RepoState wiring for auth events + connection-state reconciliation
- Tests for models, service decoding, event parsing, store reducer, and RepoState event routing

**Out of scope:**
- Server API changes
- Repo onboarding (`POST /v0/repos`) (step 4)
- proactive host binary detection
- token inspection/management UI
- custom providers beyond GitHub/Claude/Codex

---

## Implementation order

1. Models + DTO wrapper
2. Service methods for `/v0/auth`
3. Event parsing for `auth.*`
4. `AuthProviderStore` + connection-state reconciliation
5. `RepoState` wiring (service rebinding + event forwarding)
6. Shared `AuthProviderCard`
7. macOS + iOS integration
8. Tests + validation

---

## Validation plan

- `swift test --package-path swift`
- Targeted tests for:
  - `/v0/auth` wrapper decode
  - `auth.flow_started`/`auth.connected`/`auth.failed`/`auth.disconnected` parse
  - store pending/connected/failed transitions
  - 409 conflict handling path
  - refresh on reconnect path
- Manual smoke:
  - connect/disconnect each provider from macOS settings
  - connect/disconnect from iOS settings
  - start flow in one client, observe pending reconciliation in other client

## Done when

- Users can connect/disconnect GitHub, Claude, and Codex from Concerto with no terminal steps.
- Card states correctly track `active` / `pending` / `none` / `expired` from HTTP + websocket events.
- Pending state shows code and copy action when available; otherwise clear fallback text.
- Auth state reconciles on websocket reconnect and multi-client flow events.
- `swift test --package-path swift` passes.

Wave goal advanced: **“Concerto Connections panel wired to `/v0/auth` + auth events”** (step 3).
