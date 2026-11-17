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
- Slice 3 choices (headless, reversible): the independent flag is a column
  on the input's captured row, not `agent_sessions` (it is per input; a new
  input resets it by having none); the resume account is
  `session_events.provider_account` beside `provider_thread`; after #1953
  it is still read, to adopt an older managed-home transcript on resume
  (`human_session/provider_conversation.rs:229`); the Session feed lists history rows only,
  because one feed over two files' sequences cannot stay append-only; a
  rented Machine's record copies carry only captured rows, not its history.

## Machine input outage, 2026-11-16

Jack Heart: "almost or all of my sessions seem to not be receiving input anymore";
clarification: "No typed characters appear". Rex screenshot showed multiple
Claude and Codex Sessions. Direction after considering restarts: "well if the
machine server is the issue", "then i guess do somethign eslse".

At 17:58 PST, installed `lf server status` reported `ready:false`,
`listening:false`; Dev Desktop showed stale Session screens and "Loopflow server
is not running". launchd's `com.loopflow.server` had exit 78 and a cached
Executable `/Applications/Loopflow.app//Users/jack/.lf/performance/live/20261116-workflows/lf-b7e820d1f`,
while the bundle plist named `Contents/MacOS/lf`. `lf install server` returned
unavailable twice; unloading the stopped job did not clear SMAppService's
registered status. macOS App Management refused adding a temporary registration
helper to `/Applications/Loopflow.app`; no bundle files changed.

Restored service with launchd job `com.loopflow.server.recovery`, definition
`~/.lf/server-recovery.plist`, using the installed
`/Applications/Loopflow.app/Contents/MacOS/lf server run`; output goes to
`~/.lf/logs/server-recovery.log`. At 18:02 PST readiness was true, instance
`6b911057-f564-4159-ae01-c210f09e4ebd`, revision 5; Desktop reconnected.
No Rex Sessions or provider processes were restarted. This job lasts until
logout/reboot; repair the bundled SMAppService registration, then unload the
recovery job. Whether this restores Rex typing is awaiting a reply.

## Process rows: moved to LOO-953

Quotes and the OS process-list measurements on Process placement, per-kind
tables and whether Process rows are needed are in LOO-953's
`scratch/questions.md` and in this branch's git at `e500a49e70`.

## Usage investigation, 2026-11-16

- On `lf history`: Jack Heart: "what im most interested is being able to investigate things like tokens and time by wave or by skill or other things like that"

- Jack Heart: "i do wonder if the per-turn session rows (which can be useful also for like group-by-model) should also go in a separate db"

- On the three problems for usage investigation (90-day retention cutting off trends; token counts inside JSON with start and end as separate rows; model recorded on 6,261 of 21,338 Sessions): Jack Heart: "Agree we want to fix tthese"

## Calls made to relaunch, 2026-11-16 (each one a constant or a section to change)

- Process removal is its own Task, not slice 4 here. Offered twice with "I'd
  take the second"; the reply to the message carrying it was "all seems fine."
- Per-turn rows go in `usage.db`, self-contained. Raised as "i do wonder if
  ... should also go in a separate db"; not confirmed in words.
- Turn rows kept with no retention; grouping values frozen at turn end with
  the Session id kept. Neither was answered.
- `lf history list` is untouched here; whether it goes is decided with the
  Process table in the other Task.
- Turn grouping (concept review, headless): `usage.turns` takes Task, Wave,
  skill and Graph step as frozen at dispatch, because the code already
  freezes a turn's origin there (`record_session_turn_origin`), not "at turn
  end" as the direction said; the two differ only for a bind made mid-turn.
  Still open: whether reports default to these frozen values or follow later
  binds through the Session id.

## Turn table choices (2026-11-16, headless, reversible)

- Graph and step are frozen as the Graph's name and the step's compiled
  `label` (`graph_processes.compiled.steps[key=node].label`), read through the
  captured input's `lf_process_id`.
- A conflicting second usage reading or completion status is a recording gap
  that keeps the first, as receipts did; at TurnEnd a Claude transcript read
  before its last assistant row could keep a short count (the old code kept
  the first row too).
- `NativeTurn.start_seq`/`completion_seq` now hold the turn row's id, so
  Desktop's DTO keeps its shape; both are opaque.

## Slice 5 choices (2026-11-16, headless, reversible)

- History and turns pin a finished `lf` Process for 30 days after it
  finished (`FORGETTABLE`), then it is forgotten; captured inputs still pin
  theirs. This is how "30 days for finished Processes" was read.
- The server runs the store part of `lf process prune` only (no receipt
  removal, no agent reaping), hourly.
- `Watch::JOURNALS` for the Session list, `lf history` and the Task
  view/status; everything else `Watch::STATE`. A journal write wakes a
  watching subscription within the 1 s revision check, not by file event.
