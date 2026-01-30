# Continue Button for Interactive Sessions

Signal "I'm done reviewing" and advance the flow with a single tap.

## Problem

Interactive steps pause the flow until the user signals completion. Currently, you have to know to send Ctrl+D (EOF) to the terminal—an obscure Unix convention that's hostile to mobile users and anyone who hasn't memorized shell lore.

The existing "End" button terminates the session abruptly (SIGTERM), which is cancel semantics, not continue semantics. Users need an obvious, affirmative way to say "I'm satisfied, proceed to the next step."

## Approach

Add a fixed footer bar below the terminal with two actions:

1. **Continue** (primary) — Sends EOF to terminal, waits for graceful exit, triggers daemon to advance flow
2. **Cancel** (secondary) — Same as current "End" button, aborts without advancing

The footer lives outside the terminal scroll area so it's always visible and tappable—critical for mobile where you can't easily reach Ctrl+D.

```
┌─────────────────────────────────────────────────────┐
│ ● swift-falcon   design   [interactive]             │
├─────────────────────────────────────────────────────┤
│                                                     │
│  Terminal content scrolls here...                   │
│                                                     │
│  > claude: I've completed the design. Ready to      │
│    proceed when you are.                            │
│                                                     │
├─────────────────────────────────────────────────────┤
│                              [Cancel]  [✓ Continue] │
└─────────────────────────────────────────────────────┘
```

## How it works

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

## Key decisions

| Decision | Why |
|----------|-----|
| Footer bar, not header | Header has status metadata; footer follows iOS dialog conventions for action confirmation |
| EOF signal via Ghostty API | No new daemon API needed—existing exit code handling already advances flow |
| Always-enabled Continue | Users control timing; clicking mid-work sends interrupt (exit 130), recoverable via reconnect |

If user clicks Continue while agent is mid-response, process receives interrupt. Exit code will be 130 (SIGINT), not 0. Daemon treats this as incomplete, wave stays in WAITING. User can reconnect.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep Ctrl+D only | No UI changes | Hostile to mobile users, requires Unix knowledge |
| Single "Done" button | Simpler UI | Conflates cancel and continue semantics |
| Floating action button | More prominent | Obscures terminal content, feels out of place |
| Add daemon polling for "agent done" state | Auto-enable Continue | Over-engineered; agent text parsing is fragile |

## Out of scope

- Agent completion detection (polling output for "ready" patterns)—fragile, not worth it
- Mobile-specific layout—Phase 3 work
- Visual indicator for agent working vs. waiting—users can see the terminal

## Done when

- Footer renders with Cancel and Continue buttons below terminal
- Continue sends EOF (⌘Return), terminal exits cleanly, daemon advances flow
- Cancel aborts (Escape), wave stays in WAITING state
- "End" button removed from header
