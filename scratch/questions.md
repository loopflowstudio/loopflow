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
- Where Process rows live. Jack Heart, 2026-11-16: "i guess i wonder if all
  process info should just be in a separate db/", then, after the prior-art
  comparison (Kubernetes: pod status with leases, events with TTL in a
  separate etcd, records keep their own status; Slurm: slurmdbd accounting
  apart from the controller): "ok. kube and slurm seem like reliable prior
  art." Direction: every Process row, live and finished, lives in the
  Machine-history file; nothing moves at exit. This settles the `lf history`
  question as (C): records in `loopflow.db` keep the results their readers
  need on their own row.
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
- The two "Revised direction" questions (attachment placement; Task state
  reading the history file). Jack Heart, 2026-11-16: "(1) on the session (2)
  scond": the attachment token and attached Process move to `agent_sessions`;
  Graph and ship records carry their own `ended_at`, outcome and error, and
  the liveness check stamps them for a Process that dies without exiting.
- Process rows in the history file. Jack Heart, 2026-11-16, on the `outcome`
  column: "i just wanted to flag to you as somethign that needs a better
  design" and "and like maybe lf processes and agent processes shouldnt be in
  the same table?" Direction (steer): one table per kind (LfProcess,
  AgentProcess), only the columns its kind fills and a reader uses, no
  `outcome` column (derive from exit_code and error, also on Graph and ship
  records), and say how an agent Process records how it ended. Table
  definitions and each column's reader go in the plan before slice 4. Another
  branch stops writing `processes.outcome` for agent rows; whichever lands
  second reconciles. Also to design before shipping: what a live Session on
  the released lf sees after install, while it still reads and writes
  `processes` and the attachment token in `loopflow.db`.
- Slice 3 choices (headless, reversible): the independent flag is a column
  on the input's captured row, not `agent_sessions` (it is per input; a new
  input resets it by having none); the resume account is
  `session_events.provider_account` beside `provider_thread`, since
  `agent/session.rs` resumes on it; the Session feed lists history rows only,
  because one feed over two files' sequences cannot stay append-only; a
  rented Machine's record copies carry only captured rows, not its history.
