---
status: todo
phase: 2
---

# ConcertoMobile: v0

iOS app for wave management. Non-interactive—status and actions only.

---

## Overview

Mobile as remote control for lfd. Conductor persona: check in, trigger actions, move on.

**What mobile can do:**
- See wave status (running, waiting, idle)
- Trigger non-interactive steps/flows
- Land PRs
- Create waves
- See results

**What mobile cannot do:**
- Run interactive steps
- Chat with LLM (Phase B)
- Execute agent tools (Phase C)

---

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                     LoopflowCore                        │
│  noun views + models + protocols (iOS compat needed)    │
└─────────────────────────────────────────────────────────┘
          │                    │
          ▼                    ▼
    ┌──────────┐      ┌───────────────┐
    │ Concerto │      │ ConcertoMobile│
    │ (macOS)  │      │    (iOS)      │
    └──────────┘      └───────────────┘
```

**LoopflowCore:** Noun views ("here is a wave"), models, protocols. Shared across apps.

**ConcertoMobile:** Workflow views ("here is how to triage waves on mobile").

---

## Auth

Sign in via loopflow.studio using ASWebAuthenticationSession.

```swift
let authURL = URL(string: "https://loopflow.studio/auth/mobile")!
let session = ASWebAuthenticationSession(
    url: authURL,
    callbackURLScheme: "concerto"
) { callbackURL, error in
    // Extract JWT from callback
    // Store in Keychain
}
```

**Keychain storage:**
```swift
let service = "studio.loopflow.auth"
// Store JWT
// Sign in separately per device, no iCloud sync
```

---

## Discovery + Connection

After auth, discover lfd and connect.

```swift
public final class RemoteWaveService: WaveServiceProtocol {
    private let tokenProvider: TokenProvider
    private var lfdBaseURL: URL?
    private var webSocket: URLSessionWebSocketTask?

    /// Discover user's lfd via loopflow.studio
    public func connect() async throws {
        // 1. Get JWT
        let jwt = try await tokenProvider.token()

        // 2. Discover lfd URL
        let discoveryURL = URL(string: "https://loopflow.studio/api/v1/daemons/discover")!
        var request = URLRequest(url: discoveryURL)
        request.setValue("Bearer \(jwt)", forHTTPHeaderField: "Authorization")
        let (data, _) = try await URLSession.shared.data(for: request)
        let response = try JSONDecoder().decode(DiscoveryResponse.self, from: data)

        guard let url = response.lfd_url else {
            throw ConnectionError.noLfdRegistered
        }
        self.lfdBaseURL = URL(string: url)

        // 3. Connect WebSocket for live updates
        try await connectWebSocket(jwt: jwt)
    }

    private func connectWebSocket(jwt: String) async throws {
        let wsURL = lfdBaseURL!.appendingPathComponent("ws")
        var request = URLRequest(url: wsURL)
        request.setValue("Bearer \(jwt)", forHTTPHeaderField: "Authorization")
        webSocket = URLSession.shared.webSocketTask(with: request)
        webSocket?.resume()
        listenForEvents()
    }
}
```

---

## Views

### Workflow Views (ConcertoMobile)

| View | Purpose |
|------|---------|
| `WaveListView` | Triage waves, grouped by attention state |
| `WaveDetailView` | Status + actions for single wave |
| `CreateWaveView` | Pick area, flow, direction, create |
| `SignInView` | Auth flow |
| `ConnectionErrorView` | lfd unreachable state |

### Noun Views (LoopflowCore, shared)

| View | Purpose |
|------|---------|
| `WaveCard` | Single wave summary |
| `StatusBadge` | Running/waiting/idle indicator |
| `StepProgress` | Current step + elapsed time |
| `ActionButton` | Land PR, Run Step, etc. |
| `FlowPicker` | Select flow |
| `DirectionPills` | Select direction(s) |

Touch adaptations: larger tap targets (44pt minimum), appropriate spacing.

---

## Live Updates

Subscribe to WebSocket events. Update UI when events arrive.

```swift
func listenForEvents() {
    webSocket?.receive { [weak self] result in
        switch result {
        case .success(.string(let text)):
            if let event = try? JSONDecoder().decode(WaveEvent.self, from: text.data(using: .utf8)!) {
                DispatchQueue.main.async {
                    self?.handleEvent(event)
                }
            }
        case .failure(let error):
            // Reconnect logic
        default:
            break
        }
        self?.listenForEvents() // Continue listening
    }
}

func handleEvent(_ event: WaveEvent) {
    // Update local state
    // UI updates automatically via SwiftUI bindings
}
```

Events are source-agnostic. Client doesn't know if update came from git hook, GitHub poll, or webhook.

---

## Offline / Unreachable

When lfd is unreachable:
- Show clear error state
- Offer retry
- Don't cache stale data (waves require live connection)

```swift
struct ConnectionErrorView: View {
    let error: ConnectionError
    let retry: () async -> Void

    var body: some View {
        VStack {
            Image(systemName: "wifi.slash")
            Text("Can't reach lfd")
            Text(error.localizedDescription)
                .foregroundStyle(.secondary)
            Button("Retry") { Task { await retry() } }
        }
    }
}
```

---

## Scope

**In scope:**
- iOS target in Swift package
- LoopflowCore iOS compatibility work
- Auth via loopflow.studio (ASWebAuthenticationSession)
- RemoteWaveService (HTTP + WebSocket)
- Wave list grouped by attention state
- Wave detail with status + actions
- Land PR, Run Step buttons
- Create wave flow
- Live UI updates via WebSocket

**Out of scope:**
- Terminal / interactive steps
- Chat interface (Phase B)
- Agent tools (Phase C)
- APNS push notifications
- iPad-specific layouts
- Offline mode

---

## Done When

1. iOS app builds and runs on iPhone simulator
2. User can sign in via loopflow.studio
3. App discovers and connects to user's lfd
4. Wave list shows correct status
5. UI updates live when local git operations happen
6. User can tap "Land PR" and it lands
7. User can tap "Run Step" and it triggers

Verification: conductor can check wave status and land PRs from iPhone.
