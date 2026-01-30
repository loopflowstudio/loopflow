# Continue Button for Interactive Sessions

Add "Continue" button to InteractiveSessionView that signals completion of the current step and advances the flow.

## Context

From 03-conduct-ux.md:

```
┌─────────────────────────────────────────────────────┐
│ Terminal output here...                             │
│                                                     │
│ > claude: I've completed the design. Ready to       │
│   proceed when you are.                             │
│                                                     │
├─────────────────────────────────────────────────────┤
│                              [Cancel]  [✓ Continue] │
└─────────────────────────────────────────────────────┘
```

"Continue" = Ctrl+D equivalent. Tells the agent you're satisfied, flow proceeds to next step.

This button lives outside the terminal view so it's always visible and tappable on mobile.

## Current State

InteractiveSessionView has:
- Session header with wave name, step name, "interactive" badge
- Ghostty terminal view running `lf <step>`
- "End" button to terminate the session

Missing:
- "Continue" button to advance to next step
- Footer bar with Cancel/Continue actions

## Requirements

1. **Continue button** - Prominent green button, visible below terminal
2. **Cancel button** - Secondary action to abort without continuing
3. **Footer bar** - Fixed at bottom, always visible (not scrolled with terminal)
4. **Action handling** - Send Ctrl+D to terminal (or equivalent signal)
5. **Flow awareness** - After Continue, either:
   - Next step starts in same session
   - Session ends and wave advances

## Implementation

Add footer bar to InteractiveSessionView:

```swift
private var sessionFooter: some View {
    HStack(spacing: 16) {
        Spacer()

        Button {
            cancelSession()
        } label: {
            Text("Cancel")
        }
        .buttonStyle(DarkButtonStyle())

        Button {
            continueToNextStep()
        } label: {
            HStack(spacing: 4) {
                Image(systemName: "checkmark")
                Text("Continue")
            }
        }
        .buttonStyle(.borderedProminent)
        .tint(.green)
    }
    .padding(.horizontal, 20)
    .padding(.vertical, 12)
    .background(palette.surface)
}
```

The Continue action should:
1. Send Ctrl+D to the terminal (EOF signal)
2. Wait for process to exit gracefully
3. Update wave status / trigger next step

## Open Questions

- How does the daemon know to advance to the next step? Need to check if lfd has a "continue" API or if Ctrl+D is sufficient.
- Should Continue be disabled while the agent is actively working? Need UI indicator for "agent is done, waiting for you."
