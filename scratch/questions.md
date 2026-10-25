# LOO-580 questions and assumptions

- 2026-10-24, assumption: the 16 s "Preparing conversation" was not the provider-file
  search (`admit` already rejects non-UUIDs). A sample of `lf session connect NOPE-185`
  showed `sessions_for_agent_session` JSON-parsing every `observed` payload (2.5M rows,
  7 GB on Jack's store; 26 s with lf 0.14.15). Fixed with a `receipt_key` prefilter
  (`:provider-session:`), ~0.6 s on the same data, no schema change. An expression index
  would make it instant but needs a migration; left out.
- 2026-10-24, decision: "detached" = interactive Session, live agent, transport owner
  running with `processes.viewed=0` (already recorded by `transport/idle.rs`). Owners
  from builds that never recorded viewers stay `active`. A just-launched owner reads
  `detached` for the instant before its first viewer attaches.
- 2026-10-24, decision: new `SessionState::Detached` wire value (Rust, Swift, fixtures);
  its one action is Open labeled "Reconnect"; no Move here.
- The reported reconnect hang did not reproduce in the transport: observing Jack's
  orphaned owner (pid 21908, Session 70ce9350…) produced its screen in 0.49 s, and its
  launch lock was free. The hang was the id lookup above.
- Why the cmux pane disappeared: no evidence beyond the known
  `terminal host title update failed … cmux exited exit status: 1` lines. Out of scope.
