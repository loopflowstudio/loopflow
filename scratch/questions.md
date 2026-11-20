# From qa, 2026-11-19

- After the tmux fix a provider found only on the caller's PATH (not the tmux
  server's, not a tool directory) fails with "'claude' is not installed". Should
  that message name the PATH it searched, or is the tool-directory list enough?
- `lf open --focus` names the last view opened; with that pane closed and an
  earlier view still attached it fails "can't find pane". Not from this diff.
