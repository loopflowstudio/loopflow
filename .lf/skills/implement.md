---
extends: builtin
docs: scripts/testing.md
---
## In this repository

**App writes go through `PendingChanges.make`.** Flip the control in the click
and revert on refusal; use `PendingChanges.request` only for writes with no
visible state, never Task actions or `"… requested"` waits.

**A number on screen holds still.** Show progress until a count loads, then
draw the final value once.

**One spinner per pane.** Delay on an empty pane before centering a regular
spinner; over content, show progress in the toolbar strip.

**Plain icon buttons show hover and state.** Give `.plain` icon buttons a hover
highlight, tooltip, and clear on/off state when they toggle.

**Desktop confirmations are sheets, not `.confirmationDialog`.** On macOS,
`.confirmationDialog` with a `.destructive` button promotes Cancel to the
accented default button, painting both red. Use a `.sheet` with
`LoopflowButtonStyle`: at most one filled button (`.accent` for primary,
`.success` for completing work), neutral Cancel, and `.destructive` outline
when an action destroys work.

**Isolate modifier sheets and borderless menus.** Put a `ViewModifier`'s
`.sheet` inside `.background { Color.clear.sheet(...) }` so sibling `.sheet`
modifiers do not swallow it. Style icon-only `Menu` buttons with
`.menuStyle(.borderlessButton)` and a `HitTarget.control` frame, not
`.menuStyle(.button).buttonStyle(.plain)`.
