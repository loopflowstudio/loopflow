# LOO-936: record only what the product reads, history apart from state

Status: 2026-11-16, slice 1 committed (`0cdba4ed89`), slice 2 and `opened_at` committed; Process placement revised (see the last section), code for slices 2–4 paused for its review.
Old `provider_session_observed`/`provider_account_selected` rows still sit in
live stores (tests seed them); slice 5's move pass deletes them. The inventory below is the review a person reads before
code changes (brief: "Start with an inventory a person reviews before code
changes").

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
| `events.jsonl` `provider_attempt_started/finished` (~2,140) | `project_provider_history` → "Recorded …" status in Telemetry and Task history | history | **history file** |
| `events.jsonl` `conversation` (~1,070) | `input_events` → `graph.rs:921` holds or fails a step | history read once by the Graph | **history file** |
| `manifest.json` (1,076, ~1 KB) | `pending_inputs`, `project_input_history`, summary "independent" flag (`sessions.rs:372`) | history; the independent flag is state | **history file**; independent flag becomes a column, see below |
| `terminal.json` (1,061) | `pending_inputs` end time, "Recorded X" | history | **history file** |
| `attachment_exit` (1,097) | `attachment_departure` (`processes.rs:1626`) reads only `departure`; the row pins its Process | history when a departure exists, else nothing | **history file, only with a departure**; stop writing `processes.attachment_exit_seq` (no reader) |
| `provider_transition` (~1,030) | latest → Session summary and Desktop explanation (`sessions.rs:458`, `SessionsView.swift:1703`); list → `lf session history show` | latest is state; the list is history | list → **history file**; latest → **`agent_sessions` column** |
| `interactive_opened` (51) | `task_conversations` (`sessions.rs:887`), `checkout_conversation` (`:914`) choose which conversation `lf resume` opens | state | **`agent_sessions.opened_at`** |
| `provider-session:*` | resume thread (`input_provider_session`), `sessions_for_agent_session` (`sessions.rs:1042`) | state: which provider thread resumes *this input*; `agent_sessions.provider_thread` is the Session's latest and cannot answer per input | **column on the `captured` input row** (`session_events.provider_thread`, written when the reference is observed); no history row: the fact lives once, in main, and `lf session history` renders it from the input |
| `provider_lineage`, `legacy_review_feedback`, `silence:N` (~40) | fork classification, `ready_summary`, `lf session usage --weekly` | history | **history file** |
| `store-gap` (212) | none typed; `lf session history` prints it | history: AGENTS.md requires the gap in Session history | **history file** |
| `agent_process_replaced`, `agent_process_ancestry_repaired` (42) | none outside tests | log | **`tracing::info!` to `lf.log`**, stop writing rows |
| `started` (1,248), `completed` (1,760) | `pending_inputs`, `recorded_turn_end` (Session list running/finished, `human_session/turns.rs:62`), `input_turn_context` (Task context report), origin attribution | history; "is the turn running" is state | **history file**; turn running/finished is read from `session_activity`, which already wakes the list (verify in slice 3; if not, a `last_turn_ended_at` column) |
| `output` (1,203, ~1 KB) | `input_final_answer`: Graph routing (`graph.rs:975`), `lf run` (`run.rs:836`), ci-fix conclusion (`pr_landing.rs:481`), Task view | copy of the provider's last message | **stop writing**; readers read the provider transcript through one function, one reader per provider; a parse failure fails the step naming the transcript path (decided 2026-11-16, see questions.md); old rows deleted by retention, never moved |
| `usage` (1,872) | `native_usage` → Telemetry, Task history, `lf usage` | history (copied from the transcript, one row per turn) | **history file** |

`lf session history <id>` and the rental export (`machine_records.rs:104`)
read every row; both read the history file after the change.

### `processes`

| Rows | Reader | Fate |
| --- | --- | --- |
| Unfinished `lf` and `agent` rows | liveness, `lf top`, Task moving/shipping (`durable.rs:403`, `task_work.rs:644`), idle wake (`steers.rs:229`) | **main** |
| Finished rows a main record references (Graph, Graph step, `task_graphs`, ship, `ci_incidents`, `agent_sessions.agent_process_id`/`transport_lf_process_id`, `desktop_shells`, `agent_limit_recoveries`, steer tables): ~48,100 today, 21,851 agent rows and 21,579 Graph rows among them | Graph inventory, failure cause (`processes.rs:916`), "started by Session" (`processes.rs:862`) | **main** |
| Every other finished row: 127,320 agent-issued commands, children of finished parents, rows pinned only by a `session_events` reference | `lf history list/show` (`history.rs:292`), `lf context --process`, Task Work list (`task_work.rs:812`), `lf doctor`, PR-landing blockers (`pr_landing.rs:445`) | **moved to the history file at exit** |

Columns with no production writer or reader (`viewed`, `native_input`,
`viewer_hold`, `viewer_hold_at`, `caller_provider_generation`,
`attachment_exit_seq`) belong to LOO-935; this Task only stops writing
`attachment_exit_seq`.

## Chosen approach

**One rule per Process and per Session fact: it lives in exactly one file.**
State in `loopflow.db`; history in `history.db` beside it, its own SQLite file
with its own write lock, WAL and revision row.

1. **History file.** `history.db` in the store's directory, opened by
   `loopflow-storage`, schema owned in code with `PRAGMA user_version` (a
   journal; no migration catalog). Tables: `session_history` (the
   `session_events` columns minus foreign keys; history kinds only) and
   `process_history` (the `processes` columns Process readers use: id, trace,
   parent, caller Session/agent, kind, command, repo, cwd, times, outcome,
   exit code, signal, error). Readers `ATTACH` it as `history` and name
   `history.session_history` / `history.process_history`; cross-file joins
   keep working, with indexes in the history file.
2. **Writes.** History rows go through a connection whose main database is
   `history.db`, with `loopflow.db` attached read-only (`?mode=ro`) for the
   attachment-token fence. `BEGIN IMMEDIATE` then locks only `history.db`.
   A failed history write is a `store::RecordingGap` in `lf.log`, never a
   failed turn.
3. **Process exit moves, never copies.** `FORGETTABLE` drops the
   `via_agent IS NOT 1` and `session_events` clauses; what it selects is
   inserted into `process_history` and deleted from `processes` (history
   insert first; a crash between leaves a duplicate the prune pass removes by
   id, never a lost row). The retention prune runs the same move for leftovers.
4. **State facts become columns on their owner** (one migration, additive):
   `agent_sessions.opened_at`, `agent_sessions.transition` (latest provider
   transition JSON), `agent_sessions.independent` (from the manifest flow
   kind). Written in the same call that appends the history row.
5. **Wake-ups.** `StoreRevisions` gains `history`, read from `history.db`.
   `changed()` ignores it (as it ignores `usage`) except for reads that
   declare it: every projection that names a `history.*` table declares it
   (Session History, `lf session history --subscribe`, Telemetry, the Task
   view's final answers), so a view showing `output` or usage still refreshes
   when the row lands. Task updates (`harness/task_updates.rs`) read only
   `captured`, Graph and Process state, so history writes never advance
   `task_updates`; after slice 3 the `task_update_session_events_*` triggers
   see only `captured` rows. The
   `usage` domain, which wakes nothing, goes with `session_events` usage rows.
   `held_terminals` update trigger advances `processes` only when
   `viewer_hold` or `viewed` changes value or `viewer_hold_at` goes NULL ↔
   value, the rule planning triggers already follow
   (`planning_update_triggers_compare_every_column`). Nothing that reads the
   stamp is a subscription: retirement reads it on its own clock.
6. **Retention** (`lf process prune`, `store/retention.rs`, run by the
   Machine server on a clock; today nothing schedules it): `process_history` 30 days; `session_history` 90 days
   (weekly usage and Task history read back weeks); `captured` inputs in main
   are kept (they anchor Sessions; 21k rows, 5 MB). Deleting history is one
   `DELETE … WHERE observed_at < ?` per table in batches, then incremental
   vacuum of `history.db`. `lf install` backs up and migrates `loopflow.db`
   only.
7. **Existing rows.** The same pass moves main's history-kind
   `session_events` rows and movable finished Processes into `history.db` in
   batches of 10,000, deleting the dead kinds outright, then reclaims main.
   This pass is also the cutover for released `lf`: a Process still on the
   old binary after install keeps writing `session_events` rows into main and
   the next pass moves them. File the deletion of the `session_events` move
   arm for after the staging window (see Record for the final review).

### Alternatives weighed

- **Same file, no revision triggers, retention only.** Fixes size and
  wake-ups, keeps the write lock shared. History writes are ~0.3/s against
  ~7/s of state writes, so the lock win alone is small; but every install
  backs up and migrates the whole file (the 913 MB backup beside the store
  today), and a second Machine or person would merge state while history
  stays per-Machine. Rejected: the brief requires history apart from the
  lock, and the file boundary is the one that matches the several-Machines
  future.
- **Per-Session JSONL files** (like `runs/<capture>/events.jsonl`). No
  lock at all, but `pending_inputs` and usage reports read time windows
  across Sessions; files make those scans. Rejected.
- **Read finished Processes from both files with a UNION view.** Avoided by
  the move rule: each Process is in one file, and `lf history` reads
  `process_history` plus the main rows a record pins through one query in
  `processes.rs`, not a view every caller sees. (Open: whether `lf history`
  should list pinned rows at all; today it does.)

### Wild success / failure

Success: `lf install` takes seconds because `loopflow.db` is ~60 MB; Desktop
stops re-reading five times a second; `lf history --caller <session>` still
shows what an agent ran last week. Failure modes to design against: a reader
that silently returns nothing because it still names `session_events` for a
history kind (every moved kind gets one reader test through the history file);
a history file that grows without bound because the prune never runs on a
Machine (the Machine server runs the prune on its clock, slice 5; `lf doctor` reports the
file's size and oldest row); a Graph step whose transcript was compacted,
moved or reformatted (the step fails naming the transcript path, and
`lf.log` says why; it never passes on an empty answer).

## Demo

```bash
ls -la ~/.lf/loopflow.db ~/.lf/history.db          # main ~60 MB, history holds the rest
sqlite3 -readonly ~/.lf/loopflow.db "select revision from store_revisions where domain='processes'"   # twice, 5 min apart: ~0.3/s, was 4.3/s
lf session history <id>                             # turns, usage, transitions as before
lf history list --caller <session>                  # agent-issued commands as before
```

## Acceptance at gate (headless, private `LF_HOME`)

- Fixture store seeded with every row group above → prune pass → main
  `session_events` holds only `captured`; `history.session_history` holds the
  history kinds; dead kinds gone. `lf session history <id> --json` output is
  unchanged for a seeded Session (snapshot compared before/after).
- A fixture agent runs `lf commit` (via_agent): after exit the row is in
  `process_history`, not `processes`; `lf history list --caller` lists it.
- Held-terminal viewer heartbeat × 10 → `processes` revision unchanged.
- History write with `history.db` locked by another connection → the turn
  completes, `lf.log` has one recording gap, and a main-store writer was not
  blocked (`assert_no_empty_writes`-style check on main).
- `store::query_plans`: every moved read plans `SEARCH` against measured stats.
- Measure on the real store copy (`.backup` to a temp dir, run the prune
  there): before/after file sizes, rows into main per hour, revision rates.
  Report in the PR body.

## Slices (one PR)

1. **Deletions first. Done.** No longer written: `provider_account_selected`,
   `provider_session_observed` (the `provider-session:*` reference is the
   one record), `agent_process_replaced` and
   `agent_process_ancestry_repaired` (now `tracing::info!`), departure-less
   `attachment_exit`, `processes.attachment_exit_seq`. Deleted:
   `provider_session_from_events` and the fallback, the two `CaptureFact`
   variants, `has_provider_history`'s events arm. Migration
   `history_apart_from_state.sql`: held-terminal update trigger ignores
   heartbeat re-stamps; `task_update_session_events_*` lose the
   `task_update` JSON arm. `compact_usage` waits for slice 3 (usage moves).
2. History file: open, schema, write connection, recording gap.
3. Move session history kinds; state columns on `agent_sessions` and
   `session_events.provider_thread` on the input (same migration, edited in
   place), backfilled from the `provider-session:*` rows before they go;
   `history` declared by every projection naming `history.*`; repoint
   readers (`session_events.rs`, `sessions.rs` `pending_inputs`,
   `session_record.rs`, `provider_transitions.rs`, `graph_inventory.rs`,
   `task_work.rs`, `machine_records.rs`).
4. Move finished Processes; `FORGETTABLE` widened; `lf history`, Task Work
   list, `lf doctor`, PR-landing blockers read `process_history`.
5. Retention on the Machine server's clock, move pass for existing rows, `history` revision for Session
   History subscriptions, `data.md` ("Loopflow keeps no copy of the
   stream" becomes true again once `output` is gone; check the doc).
6. (In slice 3) `output`: stop writing it; `last_assistant_message(provider,
   transcript)` in one place with a Claude and a Codex reader; its six readers
   call it and fail loudly on a parse failure.

## Delete — do not maintain

Remaining: `retention.rs` `compact_usage`, `Cursor`, `.usage-cursor` file
(slice 3); `store_revision_usage_insert` and the `usage` field of
`StoreRevisions` once usage rows leave main. Slice 1 removed the rest.

## Forbidden outcomes

A history kind readable from both files; a reader falling back from
`history.session_history` to `session_events`; copying a Process to history
while it stays in main; a history write that opens `loopflow.db` for writing.

## Exclusions

`session_activity` revision traffic (~2.5/s on `sessions`): state, owned by
LOO-934's read dependencies. Dropping dead columns and tables: LOO-935.
Renaming `session_events` (now only inputs) and `agent_sessions`: LOO-937.

## Record for the final review Task

- Contract after the staging window: drop the `session_events` move arm, then
  rebuild `session_events` as the inputs table without history-only columns
  and indexes (`session_turn_*`, `session_observation_receipt`,
  `session_events_provider_session`, …); expand step = this Task's migration.
- Drop `processes` columns LOO-935 does not take, and `usage` revision domain.

## Before / after to report

Tables, columns, store size (main and history), public functions in
`loopflow-storage`, SQL call sites outside it, rows into main per hour,
`processes` and `sessions` revisions per second.

## Docs written (intended behavior)

`docs/dev/data.md` (owner table, final-answer copy, new "State and history"
section, write-lock note), `docs/telemetry.md` (retention paragraph, prune
wording), `docs/troubleshooting.md` (what survives, disk, orphaned prune),
`docs/index.md` (file layout), `docs/dev/environment.md`,
`docs/dev/machines.md` (`history.db` beside `loopflow.db`). Slice 5 checks
them against the code; the docs also promise `lf doctor` reports
`history.db` size and oldest row.

## Revised direction, 2026-11-16: every Process row in the history file

Replaces "Process exit moves, never copies" (approach 3) and slice 4; quotes in
`questions.md`. `history.db` holds every Process row, live and finished, plus
Session history. Nothing moves at exit; retention deletes old finished rows in
place; `lf history` reads one table. Records in `loopflow.db` hold Process ids
with no foreign key across files and keep the results their readers need on
their own row (Graph step and ship outcome, end time, error), so
`failure_cause` and old Graphs never depend on a Process row retention
removed. No transaction spans both files: Process row first, pointer second; a
Process row nothing points to is harmless and the liveness check reaps it.
Task-list wake-ups on Process start and exit come from the history file's
revision. Slice 1, retention 30/90 days, the server-scheduled prune and
`output` read from transcripts stand. Code for slices 2–4 stops until this
section is reviewed (slice 2's `history.rs` and `opened_at` are committed:
`be56185174`, the `opened_at` commit after it).

### Counts (production code, branch at `be56185174`)

Method: every SQL string literal outside tests that names `processes`
(a scan of `rust/**/*.rs` with test files and `#[cfg(test)]` modules excluded), grouped by
function, then each writer's transaction read by hand through its `_in`
helpers. All of them are in `loopflow-storage`, plus
`server/subscription_measurement.rs`.

- 101 statements name `processes`: 56 touch only `processes`, 43 read it
  joined with a main table (41 functions), 2 are fixture writes.
- **8 transactions write `processes` together with another table**, all
  under the agent-attachment fence; 5 go through `attach_in`
  (`processes.rs:611`), which writes `processes.attachment_token` and
  `attached_lf_process_id`, `agent_sessions.agent_process_id` and
  `session_activity` together. 3 more (last rows) tie a Process write to main
  without writing both tables:

| Transaction | Writes beside `processes` |
| --- | --- |
| `SessionTransfer::claim_attachment` (`processes.rs:64`) → `attach_in` | `agent_sessions`, `session_activity` |
| `SessionTransfer::claim_input` (`processes.rs:84`) → `claim_session_input_in` (`sessions.rs:555`) | `session_events` (captured), `agent_sessions.current_capture`, + `attach_in` |
| `SessionTransfer::replace_agent` (`processes.rs:119`) | `agent_sessions.provider_thread`, + `attach_in` |
| `SessionTransfer::finish` (`processes.rs:173`) | `agent_sessions.completed_at`, `session_events` (`attachment_exit`) |
| `SessionTransfer::commit` (`processes.rs:280`) → `complete_dead_headless_run_in` | `agent_sessions.completed_at` |
| `attach_session` (`processes.rs:1484`) | + `attach_in` |
| `prepare_agent_process` (`processes.rs:1560`) | + `attach_in` |
| `record_session_connection` (`processes.rs:1222`) | `agent_sessions.provider_thread`, transition row (`session_events`) |
| `release_agent_ancestry` (`processes.rs:1386`) | `processes` only, but reads `agent_sessions` for the fence |
| `record_process_within` (`sqlite.rs:1919`) | `held_terminals` reference (FK `held_terminal_id`) |
| `relocate_repository` (`repositories.rs:116`) | `relocated_task_processes` (FK `ON DELETE CASCADE`), `tasks`, `task_checkouts` |

- **Schema across the line:** 15 foreign keys reference `processes`
  (`agent_sessions` ×2, `ci_incidents`, `desktop_shells` ×2,
  `graph_process_steps`, `graph_processes`, `session_events`,
  `steer_adapters`, `steer_batches`, `task_graphs`, `tasks.ship_lf_process_id`,
  `waves.project_activation_lf_process_id`, and the two `ON DELETE CASCADE`
  tables `agent_limit_recoveries`, `relocated_task_processes`); `processes`
  references `agent_sessions`, `held_terminals`, `session_events`. 11 triggers
  on `processes`; `task_update_processes_*` read `graph_processes`,
  `graph_process_steps`, `task_graphs` and write main `store_revisions`, so
  they cannot follow the rows into another file.
- **43 joined reads** (file:line, function): `canonical_ids.rs:52` preflight;
  `ci_incidents.rs:244` pr_repair_lifetimes; `durable.rs:407` task_state_sql
  (Task moving/shipping); `graph_inventory.rs` 58 graphs_in, 110 entry_in,
  225 process_under_graph, 445 graph_child_after, 489/535
  graph_execution_facts; `processes.rs` 209 finish, 311 agent_process_query
  (10 tables), 481 process_query, 532 attachment_in, 650 attach_in, 1034
  saved_session_connection, 1084 agent_process_identity;
  `project_selection.rs:347` readiness_in; `provider_transitions.rs` 95
  live_process, 173 observe_in; `repositories.rs:138` relocate_repository;
  `session_events.rs` 395 record_session_activity, 435 session_liveness, 578
  session_turn_origin; `sessions.rs` 69 session_projection_inputs_in, 168
  (summary fragment), 264 available_sql, 368 summary_query (11 tables), 771
  session_terminal, 789 session_owner, 814 process_session, 838
  terminal_retention, 1107 pending_inputs, 1900 require_current_actor_in;
  `steers.rs` 69/94 with_provider_context, 174 record_native_turn, 233
  idle_conversation_candidates, 692 claim_session_steers;
  `task_retirement.rs:65` task_close_candidates; `task_work.rs` 119
  lf_process_ids, 142 task_of_process, 185 completed_graph_steps, 647
  task_open_process_pairs.

### What the counts say

The joined reads move by naming `history.processes` through the read-only
attachment; SQLite plans each side with its own indexes. The hard part is the
attachment fence: all 8 transactions change or check
`processes.attachment_token` and `agent_sessions.agent_process_id` together,
and 20 statements join `agent_sessions` to `processes` on that pair
(`attachment_in`, `require_current_actor_in`, `with_provider_context`,
`record_session_activity`, `session_turn_origin`, `summary_query`, …; one,
`record_agent_inbox`, is a write the scan counted only as a join). "Process row first, pointer second" splits it. Two ways to
keep the fence whole, for review:

- **The attachment moves to `agent_sessions`.** `attachment_token` and
  `attached_lf_process_id` are which Process holds this Session now: state of
  the Session, not of the Process. In `loopflow.db` beside
  `agent_process_id`, every fence and the 8 transactions stay in one file;
  the Process row in the history file keeps pid, times and outcome. The
  attachment then names its Process by id with no cross-file key.
- **Fences read across files.** Leave the token on the Process row and check
  it through the attachment; a handoff becomes two commits with a window
  where the Session points at a Process whose token has not changed yet.

The first is smaller and matches the prior art (a pod's binding lives on the
pod, the node lease apart). Questions for review: does the attachment move to
`agent_sessions`; does Task moving/shipping (`durable.rs:407`) read Process
liveness from the history file's revision, or does each Graph and ship record
carry `ended_at` itself so state reads never join the history file.

### Revised slices (replaces 2–4)

2. Done: `history.rs` (writer, read-only attachment, gap). Schema gains
   `processes` (all columns Process readers use) when slice 4 lands.
3. Session history kinds and the state columns, as planned (`opened_at`
   done), `output` dropped.
4. Every Process row in the history file: attachment state onto
   `agent_sessions` (if accepted), result columns on the pointing records,
   the 15 foreign keys dropped with the cascade tables cleaned up explicitly,
   `task_update_processes_*` replaced by the history revision, the 43 reads
   repointed, `FORGETTABLE` replaced by retention by age.
