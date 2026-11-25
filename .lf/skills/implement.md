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
