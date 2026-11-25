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
