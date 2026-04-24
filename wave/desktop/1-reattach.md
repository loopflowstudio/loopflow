# 02: Reattach From Anywhere

One-click reconnection to any running tmux session — in Concerto's embedded terminal or a new external window.

## Done when

A "detached" session in Concerto's sidebar has two buttons: "Open in Terminal" (launches external terminal attached to tmux session) and "Embed" (opens in Concerto's Ghostty view). Both work on sessions that were originally launched externally.

## Foundation from shipped work

The workspace shell buttons already demonstrate the attach pattern: create-or-attach a tmux session (`lf-<waveId>-shell`), open in external Ghostty or embedded GhosttyTerminalView. Reattach extends this to all sessions (agent sessions, not just user shells).

Note: workspace terminal actions are local-only. Remote targets use existing SSH/IDE flows. Reattach should follow the same boundary.

## What to build

- Detect session state: attached (external), attached (embedded), detached, completed
- "Open in Terminal" action — relaunch external terminal attached to existing tmux session
- "Embed" action — attach Concerto's GhosttyTerminalView to existing tmux session
- Handle the case where the external terminal is still open (don't double-attach, or allow multiple views)
