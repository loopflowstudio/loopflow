# Local Notifications for macOS

Surface wave state changes so users don't need to watch the app constantly.

## What was built

**NotificationService** — Singleton in LoopflowCore wrapping `UNUserNotificationCenter`

Three notification types:
- **Needs Interactive** — Wave waiting for user input
- **Error** — Wave failed
- **PR Ready** — Wave completed, PR awaiting review/land

Tap notification → focus Concerto → select that wave.

**Interactive Session Footer** — Cancel and Continue buttons for `InteractiveSessionView`
- Continue sends EOF (Ctrl+D) for graceful process exit
- Cancel destroys terminal surface without advancing flow
- Keyboard shortcuts: Escape (Cancel), Cmd+Return (Continue)

## Key decisions

| Decision | Why |
|----------|-----|
| Hook into existing event flow | `RepoState` already receives wave events—detect status transitions there |
| Compare old vs new status | Only notify on state *change*, not every refresh |
| Singleton NotificationService | Single delegate for `UNUserNotificationCenter` |
| Group by wave ID | `threadIdentifier = wave.id` replaces older notifications for same wave |
| EOF for Continue | Process exits with code 0, daemon advances to next step |

## Event flow

```
Wave status changes on server
         │
         ▼
RepoState.refreshWaves() polls via WaveService
         │
         ▼
handleWaveStatusChange() compares old/new status
         │
         ▼
NotificationService.shared.notify*() → UNUserNotificationCenter
         │
         ▼
User taps notification
         │
         ▼
didReceive → posts .selectWave → WaveSidebar selects wave
```

## Out of scope (Phase 1)

- Remote push notifications (Phase 2, requires Loopflow account + APNS)
- Fine-grained notification preferences
- Action buttons on notifications
- Rich notifications with images

## Tests

- Python: 678 tests pass
- Swift package: 61 tests pass
- Concerto build: succeeds
