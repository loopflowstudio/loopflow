---
extends: builtin
docs: scripts/testing.md
---
## In this repository

**A number on screen holds still.** A count or total drawn while data loads
shows progress until the load finishes, then the final value; it never climbs
while someone watches.

**One spinner per pane.** On an empty pane, delay briefly so fast opens never
flash, then center a regular-size spinner. Over existing content, keep the
body clear of overlays and show progress in the toolbar strip instead.
