# Design Review: Local Notifications + Interactive Session UX

This branch implements local macOS notifications for wave state changes and improves the interactive session experience with a proper Continue/Cancel footer.

## What was implemented

**Local Notifications (NotificationService)**
- New `NotificationService` singleton in LoopflowCore that wraps `UNUserNotificationCenter`
- Three notification types: needs-interactive, error, PR-ready
- Notifications grouped by wave ID (newer replaces older for same wave)
- Deep link handling: tap notification → select wave → bring app to front
- Notifications display even when app is in foreground

**Interactive Session Footer**
- Added footer bar with Cancel and Continue buttons to `InteractiveSessionView`
- Continue sends EOF (Ctrl+D) to terminal, allowing graceful process exit
- Cancel destroys terminal surface (SIGTERM) without advancing flow
- Keyboard shortcuts: Escape for Cancel, Cmd+Return for Continue

**Supporting Changes**
- `GhosttyManager.sendText()` method to inject text into terminal
- `RepoState.handleWaveStatusChange()` detects transitions and fires notifications
- `WaveSidebar` handles `.selectWave` notification to navigate on tap
- `ConcertoApp` requests notification authorization on launch

**Python Cleanups**
- Removed unused `_parse_directions` and `_is_area` from `cli.py`
- Consolidated `StimulusV1` and `StimulusUpdate` to single `StimulusRequest` model
- Extended `parse_branch_base` to handle nested timestamps and trailing timestamps

## Key choices

| Decision | Rationale |
|----------|-----------|
| Singleton NotificationService | Single delegate for `UNUserNotificationCenter`, avoids ownership issues |
| Pass waveId/waveName separately (not Wave) | API signatures cleaner, avoids importing Wave into LoopflowCore inappropriately |
| Compare oldStatus vs newStatus in RepoState | Only notify on transitions, not every refresh |
| EOF (Ctrl+D) for Continue | Process exits gracefully with exit code 0, daemon sees success and advances |
| Keyboard shortcuts on footer | Cmd+Return for continue aligns with common "submit" convention |

## How it fits together

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
didReceive → posts .selectWave → WaveSidebar.onReceive selects wave
```

For interactive sessions:
```
User clicks Continue
         │
         ▼
GhosttyManager.sendText("\u{04}") sends EOF
         │
         ▼
Shell process receives EOF, exits cleanly
         │
         ▼
ghostty close_surface_cb fires
         │
         ▼
handleSessionClosed() → sessionState.endInteractiveSession()
         │
         ▼
Daemon sees exit code 0, advances to next step
```

## Risks and bottlenecks

- **No notification preferences** — users can't disable/filter notifications yet (Phase 1 scope)
- **Notification permission denial** — if user denies, notifications silently fail (acceptable for v1)
- **Multiple waves changing simultaneously** — each fires its own notification, could be noisy
- **App must be running** — local notifications require app to be running (remote push comes in Phase 2)

## What's not included

- Remote push notifications (Phase 2, requires Loopflow account + APNS)
- Fine-grained notification preferences (quiet hours, per-wave settings)
- Action buttons on notifications (approve/reject from notification itself)
- Rich notifications with images or detailed content
- Test coverage for notification logic (would require mocking UNUserNotificationCenter)

## Tests

- **Python**: 678 tests pass
- **Swift package**: 61 tests pass
- **Concerto build**: succeeds

The notification logic and status transition handling are not directly tested due to the difficulty of mocking system notification APIs. The code paths are exercised through manual testing with running waves.
