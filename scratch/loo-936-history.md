# LOO-936: record only what the product reads, history apart from state

Status: 2026-11-17, the five changes from the concept review of the built
change are built (see "Shape after the concept review"). Remaining: the
three repairs in "Next implement direction", then gate.
Process rows are LOO-953 ("Remove Process rows: liveness on Sessions and
Graph runs", its own worktree); nothing here moves them.

## What the live store showed (2026-11-16, `~/.lf/loopflow.db`, read-only)

| | Now |
| --- | --- |
| File | 986 MB; `session_events` 289 MB + ~200 MB indexes, `processes` 169 MB + ~95 MB indexes (`dbstat`) |
| `session_events` | 345,557 rows; ~19,800 written/day over the last 7 days |
| `processes` | 183,673 rows; 3,067 `lf` + 1,144 `agent` inserted/day; 70 MB of it is `command` text |
| `lf` rows kept | 161,328: 127,320 `via_agent=1` (never forgettable), 30,118 pinned by a child, 20,688 pinned by a `session_events` reference |
| Revisions over 600 s | `processes` +2,551 (4.3/s), `sessions` +1,481 (2.5/s), `planning` +52, `graphs` +3 |
| What moved `processes` (397 s sub-window, +1,864) | 29 Process starts and 28 exits (~90 advances); 46 held terminals had a viewer stamp in the last minute. The remaining ~1,770 advances are the viewer heartbeat: `transport.rs:831` re-stamps `held_terminals.viewer_hold_at` every 10 s per viewed terminal, and `store_revision_held_terminals_update` advances `processes` on every update |
| `sessions` in the sub-window (+1,074) | 37 `session_events` rows; the rest is `session_activity` (out of scope, see Exclusions) |

Every subscription re-reads on any domain but `usage` (`read_stream.rs:107`
`changed`), so the viewer heartbeat re-runs every open Desktop projection
(`task.get`, Session list, plan list, view list) about five times a second.
This is the largest wake-up source and it is not a `processes` row.

## Inventory: every kind of row, its reader, its fate

Paths under `rust/`. "History" = a person looks at it later; "state" = the
product needs it now; "log" = diagnostics for `logs/lf.log`.

### `session_events`

| Rows (per day) | Reader | Job | Fate |
| --- | --- | --- | --- |
| `captured` (1,077) | `agent_sessions.current_capture`, Session's Graph via `captured.lf_process_id`, selector resolution (`sessions.rs:1011`, `:1384`, `:1646`) | state: the input anchor | **stays in main** |
| `observed` `task_update` (0 since #1923, 8,231 rows, 87 MB) | none (writer and reader deleted 2026-11-14) | — | **delete rows**; drop the JSON arm of `task_update_session_events_*` |
| `codex_relay`, `codex_submission`, `provider_launch`, `driver_exit`, `legacy_flow*` (0) | none | — | **delete rows** |
| `events.jsonl` `provider_account_selected` (~1,070) | fallback in `provider_session_from_history` (`loopflow-storage/src/session_record.rs:648`) | account evidence for removed accounts (LOO-913) | **stop writing**, delete rows |
| `events.jsonl` `provider_session_observed` (~1,070) | same fallback; `has_provider_history` EXISTS (`sessions.rs:386`) | duplicates the `provider-session:*` row and `agent_sessions.provider_thread` (20,240 of 21,156 Sessions have it) | **stop writing**, delete rows; `has_provider_history` reads the current input's resume thread (see `provider-session:*`), not `agent_sessions.provider_thread`, so a new input never borrows an older one's |
| `events.jsonl` `provider_attempt_started/finished` (~2,140) | `project_provider_history` → Telemetry and Task history | history; for an ordinary turn it restates the turn row | **history file, only on a retry or a failed attempt** |
| `events.jsonl` `conversation` (~1,070) | was `input_events` → `graph.rs` holds or fails a step | history; a completed turn restates the turn row | **history file, only for a turn that did not complete or an error**; the Graph reads the input row's `turn_status`/`turn_error` |
| `manifest.json` (1,076, ~1 KB) | `pending_inputs`, `project_input_history`, summary "independent" flag (`sessions.rs:372`) | history; the independent flag is state | **history file**; flag → `session_events.independent` on the captured row |
| `terminal.json` (1,061) | `pending_inputs` end time, "Recorded X" | history: the command's settlement, which no turn row says | **history file** |
| `attachment_exit` (1,097) | `attachment_departure` (`processes.rs:1626`) reads only `departure`; the row pins its Process | history when a departure exists, else nothing | **history file, only with a departure**; stop writing `processes.attachment_exit_seq` (no reader) |
| `provider_transition` (~1,030) | latest → Session summary and Desktop explanation (`sessions.rs:458`, `SessionsView.swift:1703`); list → `lf session history show` | latest is state; the list is history | list → **history file**; latest → **`agent_sessions` column** |
| `interactive_opened` (51) | `task_conversations` (`sessions.rs:887`), `checkout_conversation` (`:914`) choose which conversation `lf resume` opens | state | **`agent_sessions.opened_at`** |
| `provider-session:*` | resume thread (`input_provider_session`), `sessions_for_agent_session` (`sessions.rs:1042`) | state: which provider thread resumes *this input*; `agent_sessions.provider_thread` is the Session's latest and cannot answer per input | **column on the `captured` input row** (`session_events.provider_thread`, written when the reference is observed); no history row: the fact lives once, in main, and `lf session history` renders it from the input |
| `provider_lineage`, `legacy_review_feedback`, `silence:N` (~40) | fork classification, `ready_summary`, `lf session usage --weekly` | history | **history file** |
| `store-gap` (212) | none typed; `lf session history` prints it | history: AGENTS.md requires the gap in Session history | **history file** |
| `agent_process_replaced`, `agent_process_ancestry_repaired` (42) | none outside tests | log | **`tracing::info!` to `lf.log`**, stop writing rows |
| `started` (1,248), `completed` (1,760) | `pending_inputs`, Session list running/finished, Graph routing, `input_turn_context` | one turn; whether it ended and which turn answered is state | **`history.turns`** (one row per turn); the input's newest ended turn → **`session_events.turn_*` on its captured row** |
| `output` (1,203, ~1 KB) | Graph routing, `lf run`, ci-fix, Task view, `--final` | copy of the provider's last message | **stop writing**; routing reads the transcript; `history.turns.final_message` keeps a copy for `--final` and the Task view when the transcript is gone; old rows deleted by retention, never moved |
| `usage` (1,872) | Telemetry, Task history, `lf usage` | per-turn accounting | **`history.turns`** columns |

`lf session history <id>` and the rental export (`machine_records.rs:104`)
read every row; both read the history file after the change.

## Shape after the concept review (2026-11-17, built)

Answers quoted in `questions.md` ("1b 2b 3b 4b 5b"); page
`scratch/concept-review.html`.

**One rule per fact: it lives in exactly one file.** An input's outcome (which
turn answered it, how its newest turn ended, what failed it) is one fact in
`loopflow.db`; each turn's accounting (tokens, time, its own status) is
another, in `history.turns`. `loopflow.db` holds what
the product acts on now; `history.db` holds what a person reads later:
`session_history` (90 days) and `turns` (kept), one write lock, one writer
per process (`history.rs` `HistoryFile`), one revision (`history_revision`).
Deleting `history.db` loses history and usage, never what a running Graph or
Session list sees.

1. **Turn state on the message row.** The captured row gains `turn_thread`,
   `turn_id`, `turn_ended_at`, `turn_status`, `turn_error` (migration
   `history_apart_from_state.sql`, backfilled from main's `completed` rows,
   index `session_capture_turn`). `FactWriter` remembers the input each turn
   it saw answered (from the frozen origin) and writes the newest ended turn
   at TurnCompleted (`record_answered_turn_end`); a replayed turn it never saw
   writes no state. The recorder writes a stream error onto the row
   (`record_input_error`). Readers on main only: `input_answered_turn`
   (routing; requires `turn_status='completed'`, no guess),
   `recorded_turn_end` (Session list), `input_turn_end` (Graph failed step,
   Task view). The completion dedupes (`session_turn_completed`,
   `session_completions`) stay on `history.turns`: they guard history's own
   numbers (questions.md).
2. **One file.** `usage.db` is gone; no Machine has it installed.
   `session_history.kind` dropped (it had one legal value); the receipt key
   names the kind. `Watch::History` for Session History and the Task view;
   the Session list is `Watch::State`.
3. **History stops restating the turn.** Attempt start/finish only on a retry
   or a non-`completed` finish; conversation `turn_completed` only when not
   completed. `terminal.json` stays: it is the command's settlement, which
   no turn row says. The
   retention pass deletes old stores' rows of that shape instead of moving
   them (`UNREAD_OBSERVATION`).
4. **Final message on the turn row.** `turns.final_message`, written by the
   transcript pass with the numbers (`facts.rs` `usage`), first write kept.
   `input_final_answer_for_people` (`--final`, Task view) reads it only when
   the transcript cannot answer; routing never does.
5. **`lf machine prune`** is store retention (the server's hourly pass);
   `lf process prune` keeps receipts and orphaned providers only.

## Next implement direction (concept review of `f326e99cbb`, 2026-11-17)

The message row's `turn_*` columns are the one place routing, the Session
list and a failed step read; three things keep them from being exact.

1. **`turn_error` belongs to the newest ended turn.** Two writers fill it:
   the recorder on a stream `error` (`record_input_error`) and `FactWriter`
   at turn end (`record_input_turn_end`), in no fixed order, and the turn end
   writes `turn_error=?6` unconditionally, so a failed turn whose provider
   detail carries no error erases the error lf saw and the Graph reports
   "process exited with N" (the old reader scanned every event, so it never
   lost one). Rule: a turn end writes its own error; without one, a
   non-`completed` end keeps the row's error; a `completed` end clears it.
   Test: stream error, then a `failed` end with no detail error → the failed
   step names the stream error; stream error, then a `completed` retry →
   no error.
2. **The recorder's two writes are independent.** `record_conversation`
   returns on a failed `record_input_error` before keeping the history row;
   keep the history row first, and a failed main write is the recorder's
   usual telemetry warning, not a lost history row.
3. **Name the recorder's store for what it is.** `SessionRecorder.history`
   now writes the input row in `loopflow.db` too; call it `store`.

## Acceptance at gate (headless, private `LF_HOME`)

- Fixture store seeded with every row group → prune → main `session_events`
  holds only `captured`; `history.session_history` the history kinds minus
  ordinary-turn restatements (first attempt, completed turn); `lf session history <id> --json` unchanged.
- A turn readable as running from `history.turns` between start and end; the
  finished row carries tokens and final message.
- `history.db` locked for a turn: routing and the Session list still exact
  (message row), one wait per process, one gap in `lf.log`.
- Viewer heartbeat × 10 → `processes` revision unchanged.
- `store::query_plans`: every moved read plans `SEARCH`.
- Measure on a `.backup` copy: sizes, rows into main per hour (now + one
  update per turn end), revision rates. Report in the PR body.

## Delete — do not maintain

Done in this pass: `usage.db`, `USAGE`/`Journal`/`JOURNALS`,
`journals.rs` (now `history.rs`), `write_usage`, `execute_usage`,
`StoreRevisions::turns`, `Watch::Journals`, `session_history.kind`, the
routing fallback (`unrecorded_answer`, `AnsweredTurn::captured_at`), the
Graph's `turn_events`, the Task view's `input_events` loop, the store part
of `lf process prune` (`ProcessPruneReport::store`). Earlier passes removed
`compact_usage`, the cursor files, `provider_session_from_history`,
`ProviderFact::Output`, `SessionEventKind::Output`, `legacy_review_feedback`
writes and `process_history`.

## Forbidden outcomes

A history kind readable from both files; a reader falling back from
`history.*` to `session_events`; routing or the Session list reading
`history.db`; a history write that opens `loopflow.db` for writing.

## Exclusions

`session_activity` revision traffic (~2.5/s on `sessions`): LOO-934. Dropping
dead columns and tables: LOO-935. Renaming `session_events` and the message's
five names: LOO-937.

## Record for the final review Task

- Contract after the staging window: drop the `session_events` move arm, then
  rebuild `session_events` as the inputs table without history-only columns
  and indexes; expand step = this Task's migration.
- Drop the `usage` revision domain row.

## Upgrade skew (detect-and-name, settled)

A Process on the released lf after `lf install` keeps writing history kinds
into `loopflow.db` (moved by the hourly pass), an old Graph finds no `output`
row and spends correction turns, an old `lf session resume` starts a fresh
provider conversation; a released lf's turns leave the message row's
`turn_*` empty, so a new Graph routing such an input finds no answer.
`lf install` names live superseded processes ("not having to worry too much
about stable APIs or supporting old versions", Jack Heart, 2026-10-07). The
PR body states these costs.

## Measured before this pass (2026-11-16, `.backup` copy, three-file shape)

Migration 6.6–18 s; first prune 58 s (116k observations moved, 30,469 turns
converted, 105k rows forgotten); second prune 0.5 s. `loopflow.db` 997 MB →
607 MB (rest mostly `processes`, LOO-953); history 119 MB, usage 21 MB. A
held history lock costs a process about 5 s once (`BUSY` 2 s plus WAL
retries). Re-measure for one file and the restating rows dropped.

Added main-lock writes (estimate, 2026-11-17, live store read-only): one
`UPDATE` of the captured row per turn end. The live store captured 7,790
inputs in the last 7 days (~46/h) and the inventory measured ~1,760
`completed` rows a day (~73/h), against ~7 state writes a second
(~25,000/h): about +0.3% of main writes. Measure after install.

Live-run facts: a headless Claude step keeps its structured answer as a
`StructuredOutput` `tool_use`; Codex ends a turn with `task_complete`
carrying `last_agent_message`.

## Checks (2026-11-17)

- storage `history|turns|retention|session_events|revisions|query_plans|provider_transitions|sessions|processes|task_work`: 72 passed.
- execution `transcript|read_stream|store|session_record|harness::{fact_writer,dispatch_tests,conformance_tests}|human_session|native_exit`: 385 passed after fixes; `canonical_ids_commands_read_the_upgraded_work_and_native_history` timed out at 300 s in the run and passed alone in 249 s at load ~55 (CI).
- loopflow lib `lf::tests|doctor|top|maintenance|graph|task_view|activity|boundary|subscription_measurement|session::tests`: 89 passed; `documented_commands::` 4 passed; `public_history_discovers_unlinked_native_receipts…` passed; `cargo clippy --all-targets -D warnings` clean.
- Gate: `test(graph::)` and `test(task_graph_launch_tests::)` (failed-step read now from the input row), `test(world_recipes::)`, `binary(served_reads)`, `test(session_cli_tests::)`, `binary(session_lifecycle_tests)`, `test(store::query_plans::tests::)`; rerun the locked-history QA scenario.

## Follow-ups, not this PR

Headless Claude turns record no duration; `lf usage` grouping by skill, model
and step; one name for the message (input, capture, receipt) and for
`session_events`; `input_events` is now read only by tests.
