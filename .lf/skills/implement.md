---
extends: builtin
docs: scripts/testing.md
---
## In this repository

**A number on screen holds still.** Show progress until a count or total
finishes loading, then draw the final value once; never let it climb live.

**One spinner per pane.** Delay briefly on an empty pane so fast opens never
flash, then center a regular-size spinner. Over existing content, keep the body
clear and show progress in the toolbar strip.

**Plain icon buttons show hover and state.** Give an icon-only `.plain` button
a hover highlight and tooltip explaining what it does; when it toggles a view or
mode, show its on/off state clearly.

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
