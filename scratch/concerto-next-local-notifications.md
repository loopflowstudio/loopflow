# Local Notifications for macOS

Surface wave state changes so users don't need to watch the app constantly.

## Problem

The Conduct experience is "notification-driven"—the app should be quiet until something needs you. Currently, users must watch Concerto to know when:
- A wave enters WAITING state (interactive step or PR limit)
- A wave fails (ERROR state)
- A wave completes a PR (ready to land)

Without notifications, Conduct mode requires constant attention, defeating its purpose.

## Approach

Use macOS User Notifications to alert users when waves need attention. Three notification types:

1. **Needs Interactive** — Wave waiting for user input
2. **Error** — Wave failed
3. **PR Ready** — Wave completed, PR awaiting review/land

```
┌─────────────────────────────────────────┐
│ Concerto                                │
│ feature-auth waiting: design            │
│ Interactive step needs your input       │
└─────────────────────────────────────────┘
```

Tap notification → focus Concerto → select that wave.

## How it works

```
LFDEventService receives:
  agent.status_changed(waveId, oldStatus, newStatus)
         │
         ▼
NotificationService.notify(type, wave)
         │
         ▼
UNUserNotificationCenter.add(notification)
         │
         ▼
User taps → AppDelegate handles → Select wave in RepoState
```

### Notification triggers

| Event | Trigger |
|-------|---------|
| Needs Interactive | Status → WAITING + waitingReason == .interactive |
| Error | Status → ERROR |
| PR Ready | prNumber set + prState == .open (first time) |

### Notification content

| Type | Title | Body |
|------|-------|------|
| Needs Interactive | `{wave.name} waiting: {step}` | Interactive step needs your input |
| Error | `{wave.name} failed` | Error in {step}: {first line of error} |
| PR Ready | `{wave.name} PR #{number}` | Ready for review |

## Implementation

### NotificationService

```swift
final class NotificationService: NSObject, UNUserNotificationCenterDelegate {
    static let shared = NotificationService()

    func requestAuthorization() async throws
    func notifyNeedsInteractive(wave: Wave, step: String)
    func notifyError(wave: Wave, message: String)
    func notifyPRReady(wave: Wave, prNumber: Int)

    // UNUserNotificationCenterDelegate
    func userNotificationCenter(_:didReceive:withCompletionHandler:)
        // Handle tap → select wave
}
```

### Integration points

1. **App startup**: Request notification authorization
2. **LFDEventService**: Call NotificationService on status changes
3. **AppDelegate**: Register as notification delegate, handle taps

### Deep linking

Notification userInfo carries wave ID:
```swift
content.userInfo = ["waveId": wave.id]
```

On tap:
```swift
func userNotificationCenter(_ center: UNUserNotificationCenter,
                           didReceive response: UNNotificationResponse,
                           withCompletionHandler completionHandler: @escaping () -> Void) {
    if let waveId = response.notification.request.content.userInfo["waveId"] as? String {
        // Select wave in current repo state
        NotificationCenter.default.post(name: .selectWave, object: nil, userInfo: ["waveId": waveId])
    }
    completionHandler()
}
```

## User preferences

Later, add Settings toggle for notification types. For now, all notifications enabled by default—users can disable in System Settings.

## Key decisions

| Decision | Why |
|----------|-----|
| UNUserNotificationCenter | Modern API, required for macOS 10.14+ |
| Notification sounds | Default system sound, not custom (simplicity) |
| Badge count | Not implemented (low value for desktop) |
| Grouping | By wave name (threadIdentifier = wave.id) |

## Out of scope

- Remote notifications (Phase 2 with Loopflow accounts)
- Fine-grained notification preferences (ship minimal first)
- Action buttons on notifications (just tap to open)
- Rich notifications with images

## Done when

- Notification permission requested on first launch
- Notification fires when wave enters WAITING with interactive reason
- Notification fires when wave enters ERROR
- Notification fires when wave gets first open PR
- Tapping notification selects the wave in Concerto
- Notifications grouped by wave
