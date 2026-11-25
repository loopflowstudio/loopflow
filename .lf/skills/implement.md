---
extends: builtin
docs: scripts/testing.md
---
## In this repository

**A number on screen holds still.** Show progress while loading, then draw the
final count once.

**One spinner per pane.** Delay on an empty pane so fast opens don't flash, then
center one; over content, show progress in the toolbar.

**Plain icon buttons show hover and state.** Give icon-only `.plain` buttons
hover, a tooltip, and on/off state when they toggle.

**Inputs read as inputs.** Frame borderless text or search fields (`SearchBox`,
or `palette.surface` with `palette.borderStrong`), never bare on `palette.background`.
