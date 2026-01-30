# Local Notifications for macOS

Surface wave state changes so users don't need to watch the app constantly.

## Problem

The Conduct experience is "notification-driven"—the app should be quiet until something needs you. Currently, users must watch Concerto to know when:
- A wave enters WAITING state (interactive step or PR limit)
- A wave fails (ERROR state)
- A wave completes a PR (ready to land)

Without notifications, Conduct mode requires constant attention, defeating its purpose.

## Approach

Use macOS User Notifications (`UNUserNotificationCenter`) to alert users when waves need attention. Hook into the existing `LFDEventService` event stream—when wave events arrive and status transitions to a notification-worthy state, fire a local notification.

Three notification types:

1. **Needs Interactive** — Wave waiting for user input
2. **Error** — Wave failed (or hit circuit breaker)
3. **PR Ready** — Wave completed, PR awaiting review/land

```
┌─────────────────────────────────────────┐
│ Concerto                                │
│ swift-falcon waiting: design            │
│ Interactive step needs your input       │
└─────────────────────────────────────────┘
```

Tap notification → focus Concerto → select that wave.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Polling waves for status changes | Simple, but redundant | Events already arrive via socket—no need to poll |
| Daemon sends push-like notifications | Would bypass the app entirely | macOS local notifications require running app; Phase 2 handles remote |
| Menu bar alerts only | Less intrusive | Notifications reach users even when Concerto is hidden/minimized |

## Key decisions

| Decision | Why |
|----------|-----|
| **Hook into existing event flow** | `RepoState.startEventSubscription` already receives `wave.*` events. Detect status transitions there—no new infrastructure. |
| **Compare old vs new status** | Store `previousWaveStatus: [String: WaveStatus]` to detect transitions. Only notify on state *change*, not every refresh. |
| **UNUserNotificationCenter** | Modern macOS API (10.14+). Required for local notifications. No entitlement needed for local notifications. |
| **Singleton NotificationService** | Single point of authorization request and notification delivery. Simplifies delegate handling. |
| **Default system sound** | Simplicity. Custom sounds add complexity without clear user benefit. |
| **Group by wave ID** | `threadIdentifier = wave.id` groups related notifications. Replaces older notifications for same wave. |
| **No badge count** | Low value for desktop app. Notifications banner is sufficient. |
| **App-scoped deep link** | Use Foundation `Notification.Name.selectWave` (already pattern in codebase) to select wave on tap. |

## Scope

- In scope: Local macOS notifications when app is running, permission request on first launch, deep link to select wave on tap
- Out of scope: Remote push notifications (Phase 2), fine-grained notification preferences, action buttons on notifications, rich notifications with images

## Implementation

### NotificationService

New singleton service in `swift/LoopflowCore/Services/`:

```swift
import UserNotifications

public final class NotificationService: NSObject, UNUserNotificationCenterDelegate, @unchecked Sendable {
    public static let shared = NotificationService()

    private override init() {
        super.init()
        UNUserNotificationCenter.current().delegate = self
    }

    public func requestAuthorization() async throws {
        let center = UNUserNotificationCenter.current()
        try await center.requestAuthorization(options: [.alert, .sound])
    }

    public func notifyNeedsInteractive(wave: Wave, step: String) {
        post(
            id: "interactive-\(wave.id)",
            title: "\(wave.displayName) waiting: \(step)",
            body: "Interactive step needs your input",
            waveId: wave.id
        )
    }

    public func notifyError(wave: Wave, message: String) {
        let truncated = String(message.prefix(100))
        post(
            id: "error-\(wave.id)",
            title: "\(wave.displayName) failed",
            body: truncated,
            waveId: wave.id
        )
    }

    public func notifyPRReady(wave: Wave, prNumber: Int) {
        post(
            id: "pr-\(wave.id)",
            title: "\(wave.displayName) PR #\(prNumber)",
            body: "Ready for review",
            waveId: wave.id
        )
    }

    private func post(id: String, title: String, body: String, waveId: String) {
        let content = UNMutableNotificationContent()
        content.title = title
        content.body = body
        content.sound = .default
        content.threadIdentifier = waveId
        content.userInfo = ["waveId": waveId]

        let request = UNNotificationRequest(
            identifier: id,
            content: content,
            trigger: nil  // deliver immediately
        )

        UNUserNotificationCenter.current().add(request)
    }

    // MARK: - UNUserNotificationCenterDelegate

    public func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        didReceive response: UNNotificationResponse,
        withCompletionHandler completionHandler: @escaping () -> Void
    ) {
        if let waveId = response.notification.request.content.userInfo["waveId"] as? String {
            // Post to internal notification center for app to handle
            NotificationCenter.default.post(
                name: .selectWave,
                object: nil,
                userInfo: ["waveId": waveId]
            )
            // Bring app to front
            NSApplication.shared.activate(ignoringOtherApps: true)
        }
        completionHandler()
    }

    // Show notifications even when app is in foreground
    public func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        willPresent notification: UNNotification,
        withCompletionHandler completionHandler: @escaping (UNNotificationPresentationOptions) -> Void
    ) {
        completionHandler([.banner, .sound])
    }
}
```

### Notification.Name extension

Add to existing `Notification.Name` extensions in `ContentView.swift`:

```swift
extension Notification.Name {
    static let selectWave = Notification.Name("selectWave")
}
```

### RepoState integration

Modify `RepoState` to track status transitions and trigger notifications:

```swift
// Add to RepoState
private var previousWaveStatuses: [String: WaveStatus] = [:]

func refreshWaves() async {
    guard let repo = currentRepo else { return }
    do {
        let newWaves = try await waveService.listWaves(repo: repo)

        // Detect status transitions and notify
        for wave in newWaves {
            let oldStatus = previousWaveStatuses[wave.id]
            let newStatus = wave.status

            if oldStatus != newStatus {
                handleWaveStatusChange(wave: wave, from: oldStatus, to: newStatus)
            }
            previousWaveStatuses[wave.id] = newStatus
        }

        waves = newWaves
        // ... rest of existing logic
    } catch {
        waves = []
    }
}

private func handleWaveStatusChange(wave: Wave, from oldStatus: WaveStatus?, to newStatus: WaveStatus) {
    switch newStatus {
    case .waiting:
        // Get current step from recentSteps if available
        let step = wave.recentSteps.first?.step ?? "step"
        NotificationService.shared.notifyNeedsInteractive(wave: wave, step: step)

    case .error:
        // StepRun doesn't include error message; use step name + status
        let step = wave.recentSteps.first?.step ?? "unknown step"
        let message = "Error in \(step)"
        NotificationService.shared.notifyError(wave: wave, message: message)

    case .idle:
        // Check if PR was just created (prNumber set, wasn't before or status changed)
        if let prNumber = wave.prNumber, wave.prState == .open {
            // Only notify if this is a new PR (not just a refresh)
            if oldStatus == .running {
                NotificationService.shared.notifyPRReady(wave: wave, prNumber: prNumber)
            }
        }

    case .running, .completed:
        break  // No notification needed
    }
}
```

### App startup integration

In `ConcertoApp.swift`, request authorization on first launch:

```swift
@main
struct ConcertoApp: App {
    init() {
        Task {
            try? await NotificationService.shared.requestAuthorization()
        }
    }
    // ... rest of app
}
```

### WaveSidebar deep link handler

Add handler for `selectWave` notification in `WaveSidebar.swift`:

```swift
.onReceive(NotificationCenter.default.publisher(for: .selectWave)) { notification in
    if let waveId = notification.userInfo?["waveId"] as? String,
       let wave = repoState.waves.first(where: { $0.id == waveId }) {
        repoState.selectedWave = wave
    }
}
```

### Wave model check

The `Wave` model already has `recentSteps: [StepRun]` which contains the step name via `step: String`. No model changes needed—the existing `StepRun.step` field provides the step name for notification messages.

## Event flow

```
lfd daemon emits wave.* event
         │
         ▼
LFDEventService receives event
         │
         ▼
RepoState.refreshWaves() called
         │
         ▼
Compare previousWaveStatuses to new statuses
         │
         ▼
If status changed to waiting/error/idle+PR:
  NotificationService.shared.notify*(wave)
         │
         ▼
UNUserNotificationCenter.add(request)
         │
         ▼
User taps notification
         │
         ▼
userNotificationCenter(didReceive:) → post .selectWave
         │
         ▼
WaveSidebar.onReceive → select wave, bring app to front
```

## Done when

- Notification permission requested on first launch
- Notification fires when wave enters WAITING (any reason)
- Notification fires when wave enters ERROR
- Notification fires when wave completes with new open PR
- Tapping notification selects the wave in Concerto and brings app to front
- Notifications grouped by wave ID (newer replaces older for same wave)
- Notifications show even when app is in foreground
