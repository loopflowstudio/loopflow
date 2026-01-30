# Design Review: Continue Button for Interactive Sessions

## What was implemented

Added Cancel/Continue footer bar to `InteractiveSessionView` that provides clear session control semantics:
- **Continue** (⌘Return): Sends EOF to terminal, allowing graceful agent exit and flow advancement
- **Cancel** (Escape): Terminates session immediately via SIGTERM, wave stays in WAITING state

Also cleaned up unused code in daemon (removed `_parse_directions`, `_is_area` functions) and consolidated duplicate `StimulusV1`/`StimulusUpdate` models into single `StimulusRequest`.

## Key choices

| Decision | Why |
|----------|-----|
| Footer bar, not header | Header has status metadata; footer follows iOS dialog conventions for action confirmation |
| EOF signal via Ghostty API | No new daemon API needed—existing exit code handling already advances flow |
| Always-enabled Continue | Users control timing; clicking mid-work sends interrupt (exit 130), recoverable via reconnect |
| Consolidated stimulus models | Three identical Pydantic models served the same purpose |

## How it fits together

```
InteractiveSessionView
├── sessionHeader (status, wave name, step, badge)
├── terminalContent (GhosttyTerminalView)
└── sessionFooter (Cancel, Continue)
         │
         ├── Cancel → destroyActiveSession() → SIGTERM → wave stays WAITING
         └── Continue → sendText("\u{04}") → EOF → graceful exit → daemon advances flow
```

The GhosttyManager exposes `sendText(_:)` which wraps `ghostty_surface_text()`. Sending ASCII 4 (Ctrl+D) triggers EOF, and the process exits cleanly. The existing `onSessionClosed` callback fires, which clears session state. The daemon sees exit code 0 and advances the flow.

## Risks and bottlenecks

**Low risk:**
- If user clicks Continue while agent is mid-response, process receives interrupt. Exit code will be 130 (SIGINT), not 0. Daemon treats this as incomplete, wave stays in WAITING. User can reconnect. This is documented behavior, not a bug.

**No new daemon coupling:**
- All flow advancement logic already existed. This change only adds a UI affordance for sending EOF.

## What's not included

- Agent completion detection (polling output for "ready" patterns)—deliberately skipped per design doc
- Mobile-specific layout—Phase 3 work
- Visual indicator for agent working vs. waiting—fragile, not worth it
