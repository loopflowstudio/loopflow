# LOO-398 assumptions — 2026-10-07

- Proposed reversible choice: send lf's small terminal-only reports without a
  detection round trip; this claims no consumer support. Gate must disprove
  visible corruption or input theft on unsupported terminal/multiplexer paths.
- Ghostty's parser is present but its inspected embedder API has no status action.
  Candidate `a60e9e2a…` confirms the libghostty-vt callback and missing embedder
  action. The design selects a small third patch; its build and runtime behavior
  remain unproved. Recheck if the selected revision changes.
- LOO-394 owns the absent PTY relay. Its Codex engine exception and shell-pane
  restoration remain unresolved in its own draft; LOO-398 assumes neither choice.
  Detached Session reporting cannot be called complete before that integration.
- Keep LOO-384 for non-reporters until a supported Claude release demonstrates
  complete reporting. Verbal ecosystem support is insufficient to delete it.
