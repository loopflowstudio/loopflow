---
extends: builtin
docs: scripts/testing.md
---
## In this repository

**App writes go through `PendingChanges.make`.** Flip the control in the click
and revert on refusal; use `PendingChanges.request` only for writes with no
visible state, never Task actions or `"… requested"` waits.

**A number on screen holds still.** Show progress while loading, then draw the
final count once.

**One spinner per pane.** Delay on an empty pane so fast opens don't flash, then
center one; over content, show progress in the toolbar.

**Plain icon buttons show hover and state.** Give icon-only `.plain` buttons
hover, a tooltip, and on/off state when they toggle.

**Inputs read as inputs.** Frame borderless text or search fields (`SearchBox`,
or `palette.surface` with `palette.borderStrong`), never bare on `palette.background`.

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
