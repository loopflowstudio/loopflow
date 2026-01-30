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
│ ● swift-falcon   design   [interactive]      [End] │
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

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep Ctrl+D only | No UI changes | Hostile to mobile users, requires Unix knowledge |
| Single "Done" button | Simpler UI | Conflates cancel and continue semantics |
| Floating action button | More prominent | Obscures terminal content, feels out of place |
| Move "End" to footer, rename to "Cancel" | Reuses existing control | Two places to look for session controls |
| Add daemon polling for "agent done" state | Auto-enable Continue | Over-engineered; agent text parsing is fragile |

## Key decisions

1. **Footer bar, not header.** The header already has session metadata and the End button. Adding Continue there crowds it. Footer is natural for action confirmation (iOS patterns, dialog buttons). Follows "Cancel/Continue" left-to-right convention.

2. **EOF signal, not API call.** The daemon already handles process exit correctly—exit code 0 means success, advances flow. Sending EOF (ASCII 4) via `ghostty_surface_text()` is the same as user typing Ctrl+D. No new API needed.

3. **Remove "End" from header.** Move it to footer as "Cancel" for a single, consistent action area. Users won't have to choose between two exit mechanisms.

4. **No "agent is done" detection.** The original design doc asked about disabling Continue while agent works. Skip this. Polling terminal output for "ready to proceed" patterns is fragile. Users can see the terminal—they know when to click Continue.

5. **Continue always visible, always enabled.** User owns the decision. If they click Continue mid-agent-work, the EOF interrupts the process (exit code 130), daemon treats it as incomplete, wave stays in WAITING state. That's recoverable—you can reconnect.

**Wave principles applied:**
- "Connect when needed, land when ready" (03-conduct-ux.md) — Continue is the "ready" signal
- "Continue = Ctrl+D equivalent" (03-conduct-ux.md) — EOF semantics, not terminate
- "Button lives outside terminal view so it's always visible" (03-conduct-ux.md) — Fixed footer

## Scope

**In scope:**
- Footer bar component in `InteractiveSessionView`
- Continue action: send EOF character to terminal via Ghostty API
- Cancel action: existing `endSession()` logic moved from header
- Remove "End" button from header
- Keyboard shortcut: ⌘Return for Continue (macOS convention for "proceed")

**Out of scope:**
- Agent completion detection (polling, output parsing)
- Visual indicator for "agent waiting vs. working"
- Mobile-specific layout (iOS/iPad work comes in Phase 3)
- New daemon APIs (existing EOF → exit → advance flow is sufficient)

## Implementation

### Footer Component

```swift
private var sessionFooter: some View {
    HStack(spacing: Spacing.lg) {
        Spacer()

        Button {
            cancelSession()
        } label: {
            Text("Cancel")
        }
        .buttonStyle(DarkButtonStyle())
        .keyboardShortcut(.escape, modifiers: [])

        Button {
            continueSession()
        } label: {
            HStack(spacing: Spacing.xs) {
                Image(systemName: "checkmark")
                Text("Continue")
            }
        }
        .buttonStyle(.borderedProminent)
        .tint(Color.statusSuccess)
        .keyboardShortcut(.return, modifiers: .command)
    }
    .padding(.horizontal, Spacing.xl)
    .padding(.vertical, Spacing.md)
    .background(palette.surface)
}
```

### Continue Action

```swift
private func continueSession() {
    // Send EOF (Ctrl+D = ASCII 4) to terminal
    let eof = "\u{04}"
    GhosttyManager.shared.sendText(eof)

    // Process will exit gracefully, triggering onSessionClosed callback
    // which calls sessionState.endInteractiveSession()
    // Daemon sees exit code 0, advances flow
}
```

### GhosttyManager Extension

Add `sendText(_:)` method to expose text input to the terminal:

```swift
// In GhosttyManager.swift
func sendText(_ text: String) {
    guard let surface = activeSurface else { return }
    text.withCString { ptr in
        ghostty_surface_text(surface, ptr, UInt(text.utf8.count))
    }
}
```

### View Structure Update

```swift
var body: some View {
    VStack(spacing: 0) {
        sessionHeader      // Remove "End" button
        Divider()
        terminalContent
        Divider()
        sessionFooter      // New: Cancel + Continue
    }
    .background(palette.background)
    // ... existing .task modifier
}
```

## Done when

```bash
# 1. Footer renders correctly
# - Two buttons visible below terminal
# - "Cancel" on left, "Continue" (green, checkmark) on right
# - Footer doesn't scroll with terminal content

# 2. Continue sends EOF and advances flow
lf design: test flow  # Start interactive session
# Click Continue
# Terminal exits cleanly (exit code 0)
# Daemon emits session.completed event
# If flow has more steps, next step runs

# 3. Cancel aborts without advancing
# Click Cancel
# Terminal killed (SIGTERM)
# Wave stays in WAITING state, can reconnect

# 4. Keyboard shortcuts work
# ⌘Return triggers Continue
# Escape triggers Cancel

# 5. "End" button removed from header
# Header only shows: status dot, wave name, step name, interactive badge
```
