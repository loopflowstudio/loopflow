# Design Review: Local Notifications for Concerto

## What was implemented

**Local notifications for wave state changes** — macOS notifications surface when waves need attention, fail, or complete with PRs. Users tap a notification to focus Concerto and select that wave.

**Interactive session footer** — Cancel and Continue buttons for interactive sessions, with keyboard shortcuts (Escape, Cmd+Return). Continue sends EOF to gracefully advance the flow.

**Branch naming fix** — `parse_branch_base()` now handles nested timestamps recursively, fixing cases like `foo.20260129_2255.20260129_2318.aurora-rondo`.

**Code cleanup** — Removed unused `_parse_directions()` and `_is_area()` from `lfd/cli.py`. Consolidated duplicate `StimulusUpdate`/`StimulusV1` classes into single `StimulusRequest` in HTTP server.

## Key choices

| Decision | Why |
|----------|-----|
| `NotificationService` singleton | Single delegate for `UNUserNotificationCenter` required by Apple |
| Status change detection in `RepoState.refreshWaves()` | Already polls wave state; comparing old vs new avoids separate event stream |
| `threadIdentifier = wave.id` | Groups notifications per wave, newer replaces older |
| EOF (Ctrl+D) for Continue | Standard Unix signal for graceful stdin close; daemon sees exit code 0 |
| Cancel destroys terminal surface | SIGTERM lets process clean up; wave stays in WAITING for reconnect |
| Recursive timestamp stripping | Wave branches can accumulate multiple iteration suffixes |

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
User taps notification → .selectWave posted → WaveSidebar selects wave
```

Interactive session flow:
- User clicks Cancel → SIGTERM → session cleared → wave stays WAITING
- User clicks Continue → EOF sent → process exits 0 → daemon advances flow

## Risks and bottlenecks

**Notification permission** — Requested on app launch. If denied, notifications silently fail. No UI to prompt again (standard macOS behavior).

**Polling-based detection** — Status changes detected on next `refreshWaves()` poll (default 5s). Near-instant for practical use, but not real-time.

**EOF assumption** — Continue assumes the running process (Claude Code/Codex) interprets EOF correctly. If the process ignores EOF, the terminal stays open.

## What's not included

- Remote push notifications (Phase 2, requires Loopflow account + APNS)
- Fine-grained notification preferences (quiet hours, per-wave toggles)
- Action buttons on notifications (approve/dismiss from notification)
- Rich notifications with images

## Test coverage

| Suite | Result |
|-------|--------|
| Python | 678 tests pass |
| Swift package | 61 tests pass |
| Concerto build | succeeds |

New tests added:
- `test_parse_branch_base_strips_trailing_timestamp`
- `test_parse_branch_base_nested_timestamps`
