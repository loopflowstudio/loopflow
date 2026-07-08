# Session lifecycle

The spine. Audited 2026-07-08: survive-restart already holds — by
statelessness, not sessions. The wave process is externalized to detached
tmux; the app holds no handle and rediscovers via `wave/<name>/
.wave-endpoint` + full SSE replay each launch. That mechanism is
pass-model-compatible (the listener persists; only vendor sessions are
per-pass).

## KRs

- The stateless reattach contract is named and tested as the contract:
  tmux-externalized process + endpoint rediscovery + replay, 5/5 dogfood
  trials across app restarts.
- The iOS-only legacy session-record machinery (SessionState.
  reconnectIfNeeded, UserDefaults session ids, TerminalWorkspaceStore) is
  deleted or ported to the stateless model — no Swift-owned parallel
  lifecycle.
- Dispatched runs get an attach surface: today they are only op frames +
  `lf runs` history; the surfacing doctrine says execs are attachable —
  one action from the run row to its live tmux session.
