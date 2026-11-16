# LOO-936 open choices

- History retention: 30 days for finished Processes, 90 days for Session
  history. Chosen so weekly usage and recent Task history keep working;
  either number is one constant. Jack Heart, 2026-11-16: "Seems fine".
- `lf history list` keeps listing finished Processes that a main record pins
  (Graph steps, ship), read from `processes` beside `process_history`. The
  simpler alternative is to list only `process_history`, which would hide
  Graph step commands from `lf history`.
- The attachment-token fence on history writes reads `loopflow.db` without
  its write lock, so a replaced agent can append one stray history row after
  a handoff. Accepted because history grants no authority.
- `output` (the last assistant message) stays copied into history: Graph
  routing, `lf run`, ci-fix and the Task view read it, and reading provider
  transcripts instead would tie routing to each provider's file format.
- The plan calls the retention pass "the existing hourly `lf top --prune`
  pass", but nothing in the repo schedules `lf process prune` (only manual
  runs and `scripts/disk-cleanup.sh` paths). Docs say retention applies when
  `lf process prune` runs and make no hourly claim. Asked whether the Machine
  server should run it on a clock; Jack Heart, 2026-11-16: "Yeah that woudl
  be good". Slice 5 adds the server's clock and the docs state it.
- `has_provider_history` (slice 1) drops only its `provider_session_observed`
  arm and keeps reading the current input's `provider-session:*` row, not
  `agent_sessions.provider_thread`: a new input must not borrow an older
  input's resume reference (`recovery_refresh_recognizes_imported_history_before_attachment`).
  Settled in the design: the per-input thread becomes a column on the
  `captured` row in main, so it never reads the history file.
