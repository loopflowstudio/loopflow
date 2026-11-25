---
extends: builtin
docs: scripts/testing.md
---
## In this repository

**A number on screen holds still.** Show progress until a load finishes, then
the final value; never let a count climb while someone watches.

**One spinner per pane.** Delay briefly on an empty pane so fast opens never
flash, then center a regular spinner; over existing content, keep the body
clear and show progress in the toolbar strip.

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
