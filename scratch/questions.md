# LOO-936 open choices

- History retention: 30 days for finished Processes, 90 days for Session
  history. Chosen so weekly usage and recent Task history keep working;
  either number is one constant. Jack Heart, 2026-11-16: "Seems fine".
- The attachment-token fence on history writes reads `loopflow.db` without
  its write lock, so a replaced agent can append one stray history row after
  a handoff. Accepted because history grants no authority.
- `output` (the last assistant message): Jack Heart, 2026-11-16, chose
  option B: "(2) (B)". Stop copying `output` into the store; its readers
  (Graph routing `graph.rs:975`, `lf run` `run.rs:836`, ci-fix
  `pr_landing.rs:481`, Task view, `session_history`, `fact_writer.rs:846`)
  read the last assistant message from the provider transcript when they
  need it. Risks to design for: Claude and Codex transcript formats and
  locations change; transcripts can be compacted, moved or deleted; a parse
  failure must not silently pass a step (fail it with a clear error naming
  the transcript path, and log it); one reader per provider behind one
  function. Existing `output` rows get no new writes and are deleted under
  retention, not moved.
- Still open: whether `lf history` lists finished Processes a main record
  pins (A), or (C) those records keep outcome, end time and error themselves.
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
