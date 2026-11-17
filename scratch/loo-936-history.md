# LOO-936: record only what the product reads, history apart from state

Status: 2026-11-16, slices 1–5 built (slice 5: "Slice 5 as built");
merged with main through #1962 (`4a7c949b9e`). Remaining: gate. Since #1955
an agent Process records no outcome, so a departure-less attachment exit now
leaves nothing (no history row, no `processes.outcome`). Process rows are LOO-953 ("Remove Process rows:
liveness on Sessions and Graph runs", its own worktree); nothing here moves
them.

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
| `manifest.json` (1,076, ~1 KB) | `pending_inputs`, `project_input_history`, summary "independent" flag (`sessions.rs:372`) | history; the independent flag is state | **history file**; flag → `session_events.independent` on the captured row |
| `terminal.json` (1,061) | `pending_inputs` end time, "Recorded X" | history | **history file** |
| `attachment_exit` (1,097) | `attachment_departure` (`processes.rs:1626`) reads only `departure`; the row pins its Process | history when a departure exists, else nothing | **history file, only with a departure**; stop writing `processes.attachment_exit_seq` (no reader) |
| `provider_transition` (~1,030) | latest → Session summary and Desktop explanation (`sessions.rs:458`, `SessionsView.swift:1703`); list → `lf session history show` | latest is state; the list is history | list → **history file**; latest → **`agent_sessions` column** |
| `interactive_opened` (51) | `task_conversations` (`sessions.rs:887`), `checkout_conversation` (`:914`) choose which conversation `lf resume` opens | state | **`agent_sessions.opened_at`** |
| `provider-session:*` | resume thread (`input_provider_session`), `sessions_for_agent_session` (`sessions.rs:1042`) | state: which provider thread resumes *this input*; `agent_sessions.provider_thread` is the Session's latest and cannot answer per input | **column on the `captured` input row** (`session_events.provider_thread`, written when the reference is observed); no history row: the fact lives once, in main, and `lf session history` renders it from the input |
| `provider_lineage`, `legacy_review_feedback`, `silence:N` (~40) | fork classification, `ready_summary`, `lf session usage --weekly` | history | **history file** |
| `store-gap` (212) | none typed; `lf session history` prints it | history: AGENTS.md requires the gap in Session history | **history file** |
| `agent_process_replaced`, `agent_process_ancestry_repaired` (42) | none outside tests | log | **`tracing::info!` to `lf.log`**, stop writing rows |
| `started` (1,248), `completed` (1,760) | `pending_inputs`, `recorded_turn_end` (Session list running/finished, `human_session/turns.rs:62`), `input_turn_context` (Task context report), origin attribution | one turn, read while it runs | history file in slice 3; → **`usage.turns`** (one row per turn) |
| `output` (1,203, ~1 KB) | `input_final_answer`: Graph routing (`graph.rs:975`), `lf run` (`run.rs:836`), ci-fix conclusion (`pr_landing.rs:481`), Task view | copy of the provider's last message | **stop writing**; readers read the provider transcript through one function, one reader per provider; a parse failure fails the step naming the transcript path (decided 2026-11-16, see questions.md); old rows deleted by retention, never moved |
| `usage` (1,872) | `native_usage` → Telemetry, Task history, `lf usage` | per-turn accounting | history file in slice 3; → **`usage.turns`** columns |

`lf session history <id>` and the rental export (`machine_records.rs:104`)
read every row; both read the history file after the change.

### `processes`

Untouched here beyond slice 1 (no `attachment_exit_seq` writes; heartbeat
re-stamps no longer advance the revision). Its inventory, the 101 statements,
8 fenced transactions and 15 foreign keys are in git at `e500a49e70`
("Revised direction" and "Slice 4 tables") for LOO-953.

## Chosen approach

**One rule per fact: it lives in exactly one file.** `loopflow.db` holds
what people work on; `history.db` what happened in detail, safe to delete;
`usage.db` one row per turn, kept. Each is its own SQLite file with its own
write lock, WAL and revision row.

1. **History file** (built). `history.db` beside the store, opened by
   `loopflow-storage` (`history.rs`), schema in code under `PRAGMA
   user_version`. One table, `session_history` (the `session_events` columns
   minus foreign keys; history kinds only). Every store connection attaches it
   read-only as `history`.
2. **Writes** (built). A connection whose main database is `history.db`,
   `loopflow.db` attached read-only for the attachment fence; `BEGIN
   IMMEDIATE` locks only `history.db`. A failed write is a
   `store::RecordingGap`, never a failed turn.
3. **State facts are columns on their owner** (built, migration
   `history_apart_from_state.sql`): `agent_sessions.opened_at`,
   `agent_sessions.transition`, and on the captured input row
   `session_events.provider_thread`, `.provider_account`, `.independent`.
4. **Turns** in `usage.db` (to build, see "Turns, settled in concept review").
5. **Wake-ups.** `StoreRevisions` gains `history` (built) and `turns`.
   `changed()` ignores both except for reads that name `history.*` or
   `usage.*` (Session History, `lf session history --subscribe`, Session list
   running state, Telemetry, Task history). Not wired yet. Task updates read
   only `captured`, Graph and Process state, so history writes never advance
   `task_updates`. The `held_terminals` trigger change is built (slice 1).
6. **Retention** (`lf process prune`, `store/retention.rs`, run by the
   Machine server on a clock; today nothing schedules it):
   `session_history` 90 days, `usage.turns` not pruned, `captured` inputs
   kept. One batched `DELETE … WHERE observed_at < ?`, then incremental
   vacuum of `history.db`. `lf install` backs up and migrates `loopflow.db`
   only.
7. **Existing rows.** The same pass moves main's history-kind
   `session_events` rows into `history.db` and builds turn rows from
   `started`/`usage`/`completed` in either file, in batches of 10,000,
   deleting dead kinds, then reclaims main. It is also the cutover for a
   released `lf` still writing those rows into main after install.

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
### Wild success / failure

Success: `lf install` takes seconds because `loopflow.db` is ~60 MB; Desktop
stops re-reading five times a second; one query on `usage.turns` answers
tokens and time by skill or model. Failure modes to design against: a reader
that silently returns nothing because it still names `session_events` for a
history kind (every moved kind gets one reader test through the history file);
a history file that grows without bound because the prune never runs on a
Machine (the Machine server runs the prune on its clock, slice 5; `lf doctor` reports the
file's size and oldest row); a Graph step whose transcript was compacted,
moved or reformatted (the step fails naming the transcript path, and
`lf.log` says why; it never passes on an empty answer).

## Demo

```bash
ls -la ~/.lf/loopflow.db ~/.lf/history.db ~/.lf/usage.db
sqlite3 -readonly ~/.lf/loopflow.db "select revision from store_revisions where domain='processes'"   # twice, 5 min apart: ~0.3/s, was 4.3/s
lf session history <id>                             # turns and observations
sqlite3 -readonly ~/.lf/usage.db "select skill, sum(output_tokens), sum(ended_at-started_at) from turns group by 1 order by 2 desc limit 10"
```

## Acceptance at gate (headless, private `LF_HOME`)

- Fixture store seeded with every row group above → prune pass → main
  `session_events` holds only `captured`; `history.session_history` holds the
  history kinds; dead kinds gone. `lf session history <id> --json` output is
  unchanged for a seeded Session (snapshot compared before/after).
- A fixture turn is readable as running from `usage.turns` between start and
  end, and the finished row carries its token counts as columns.
- Held-terminal viewer heartbeat × 10 → `processes` revision unchanged.
- History write with `history.db` locked by another connection → the turn
  completes, `lf.log` has one recording gap, and a main-store writer was not
  blocked (`assert_no_empty_writes`-style check on main).
- `store::query_plans`: every moved read plans `SEARCH` against measured stats.
- Measure on the real store copy (`.backup` to a temp dir, run the prune
  there): before/after file sizes, rows into main per hour, revision rates.
  Report in the PR body.

## Slices (one PR)

1. **Done.** Dead kinds no longer written (`provider_account_selected`,
   `provider_session_observed`, `agent_process_replaced`,
   `agent_process_ancestry_repaired` now `tracing::info!`, departure-less
   `attachment_exit`, `processes.attachment_exit_seq`); heartbeat trigger and
   `task_update_session_events_*` narrowed.
2. **Done.** `history.rs`: writer, read-only attachment, gap.
3. **Done**, see "Slice 3 as built".
4. **Done**, see "Turn table as built".
5. **Built**, see "Slice 5 as built".

## Delete — do not maintain

Slice 3 removed `compact_usage`, `Cursor`, the cursor files' code,
`native_usage_key`, `provider_session_from_history`, `ProviderFact::Output`
and its emission in the four decoders, `SessionEventKind::Output`,
`TurnFacts::final_answer`, `MEMBERSHIP_KIND`, `RetentionReport::usage_rows`,
`StoreRevisions::usage` (now `history`), `store_revision_usage_insert`, the
`legacy_review_feedback` write and the unused `process_history` table.
Slice 5 bounded `FORGETTABLE`'s journal arms by age (history and turns pin
a finished Process for 30 days) and renamed `HistoryFile`/`history.rs` to
`JournalFile`/`journals.rs`, `server/statistics.rs` to `maintenance.rs`.

## Forbidden outcomes

A history kind readable from both files; a reader falling back from
`history.session_history` to `session_events`; a turn read from rows other
than `usage.turns`; a history or usage write that opens `loopflow.db` for
writing.

## Exclusions

`session_activity` revision traffic (~2.5/s on `sessions`): state, owned by
LOO-934's read dependencies. Dropping dead columns and tables: LOO-935.
Renaming `session_events` (now only inputs) and `agent_sessions`: LOO-937.

## Record for the final review Task

- Contract after the staging window: drop the `session_events` move arm, then
  rebuild `session_events` as the inputs table without history-only columns
  and indexes (`session_turn_events`, `session_turn_window`,
  `session_observation_receipt`, `session_events_provider_session`,
  `session_input_membership`, `session_provider_transitions`,
  `session_input_outcome_window`, `session_event_input`,
  `session_events_turn`, …); expand step = this Task's migration.
- Drop the `usage` revision domain row.

## Before / after to report

Tables, columns, store size (main, history, usage), public functions in
`loopflow-storage`, SQL call sites outside it, rows into main per hour,
`processes` and `sessions` revisions per second.

## Docs written (intended behavior)

`docs/dev/data.md` (owner table, final-answer copy, new "State and history"
section, write-lock note), `docs/telemetry.md` (retention paragraph, prune
wording), `docs/troubleshooting.md` (what survives, disk, orphaned prune),
`docs/index.md` (file layout), `docs/dev/environment.md`,
`docs/dev/machines.md` (`history.db` beside `loopflow.db`). Written when
Processes were to move too: slice 5 checks them against the code, removes
Process-in-history claims, adds `usage.db`; the docs also promise `lf doctor`
reports `history.db` size and oldest row.

## Slice 3 as built (2026-11-16)

- `session_events` in `loopflow.db` holds only `captured` rows. Every other
  kind is written through `SqliteStore::append_history`/`write_history` to
  `history.session_history` and read only there. Migration
  `history_apart_from_state.sql` adds `session_events.provider_account` and
  `.independent` (the captured row's resume reference reuses its
  `provider_thread` column; backfilled from the newest `provider-session:*`
  row and the manifest), `agent_sessions.transition` (backfilled from the
  newest transition, legacy `agent-session:*` shape included), the index
  `session_capture_provider_thread`, a `preserve_capture_update` trigger that
  allows only those three columns to change, and drops
  `store_revision_usage_insert`.
- A main-store write that also produces history (transition, attachment exit,
  observation) commits first, then appends history; the history row never
  fails the caller. Conflicting receipts are gaps that keep the first row.
- `HistoryFile` carries the store's path, so a history write never takes the
  store's mutex. Every store connection attaches `history.db` read-only
  (creating it when missing); one that cannot gets an empty in-memory
  `history` so reads answer empty instead of failing. Read-only openers now
  pass `SQLITE_OPEN_URI`. `refresh_statistics` and the query-plan fixtures
  analyze `main` only.
- The Session feed (`session_history`, `lf session history <id>`) lists
  history rows only; inputs stay Session state (`session_inputs`).
- `output`: `input_answered_turn` names the turn; `transcript::answer`
  reads its answer from the provider transcript and errors naming the file.
  Graph routing, `lf run`, ci-fix, `lf session history show --final` and the
  Task view call it; the Task view shows the error in place.
- Tests: `history_kinds_are_read_from_the_history_file_only` (storage),
  `transcript::answer::tests` (Codex, Claude structured and text, gone
  transcript), plus the existing suites repointed; integration fixtures read
  `history.db` (`8982aba4e2`) and stand-in providers keep transcripts where
  their provider does (`b9df987fe0`, `loopflow-execution/tests/support/`).

Live-run facts (2026-11-16, read-only on this Machine): a headless Claude
step's transcript keeps its structured answer as an assistant `tool_use`
named `StructuredOutput` whose `input` is the object, followed by a
`tool_result` "Structured output provided successfully". The turn lf
recorded (`c768ee3e-…` in thread `367239b4-…`) is the `uuid` of the request
row in that transcript. The transcript's object omits null fields the stream
result carried (`"reason": null`); `GraphOutput::decode` counts non-null
fields, so both decode alike. Codex rollouts end a turn with
`event_msg`/`task_complete` carrying `last_agent_message`.

Until slice 5 declares the `history`/`turns` revisions, a Desktop view of a
running turn refreshes only on other domains' changes.
`opencode_disconnect_after_tool_preserves_unknown_native_completion` expects
status `interrupted` for the turn OpenCode's store left open, as for a crashed
Pi turn; whether that is right for an unknown completion is a product call.

Check results: `cargo nextest run -p loopflow-storage -p loopflow-execution
--lib`: 2438 passed, 2 failed; the two were
`ordinary_task_comments_and_completion_preserve_live_graph_and_arrival`
(fixed since by the stand-in transcript) and
`native_runner_is_quiet_until_its_turn_ends_codex_foreground` (sccache
"cannot find binary path" in this environment; for gate/CI).

## Upgrade skew (to design before ship)

A Process still running the released lf after `lf install` keeps its old
reads and writes while new Processes use the history file:

- It writes history kinds into `loopflow.db`; new readers do not see them
  until the move pass carries them over (a live turn's usage, start and end
  vanish from Session History meanwhile).
- An old Graph process reads its step's answer from `output` rows in
  `loopflow.db`; a step run by the new lf writes none, so the old Graph sees
  "final output is absent" and spends its correction turns. An old
  `lf session resume` reads the resume thread from `provider-session:*` rows,
  which the new lf no longer writes, so it starts a fresh provider
  conversation instead of resuming.
- New readers of the resume reference, the independent flag and the latest
  transition see what the migration backfilled, plus what new Processes write.

Settled in concept review after slice 4: the new lf does not answer old
readers. `lf install` already names every live process on a superseded build
before and after the promote (`binaries::report_before_promote`,
`report_after_promote`: "still running the previous lf: …", with the
`lf resume` remedy), which is the standing rule: "not having to worry too much
about stable APIs or supporting old versions" (Jack Heart, 2026-10-07; detect
the skew, say so, name the fix). What a superseded process writes into
`loopflow.db` (observations, three-row turns) is carried over by slice 5's
move pass on the server's clock. The PR body states the two costs while one
runs: an old Graph step finds no `output` row and spends correction turns; an
old `lf session resume` starts a fresh provider conversation.

## Direction after review, 2026-11-16 evening

Quotes in `questions.md`. This Task ends at Session history plus per-turn
accounting; Process rows are LOO-953. The turn table is self-contained (one
query by skill or model, no join to `loopflow.db`), columns not JSON, one row
per turn, written by `loopflow-storage` like `history.db`, not under the
90-day retention. Existing rows (29,440 `usage` / 24,900 `started` / 28,791
`completed` since 2026-09-30) are converted. `lf usage` grouping flags and
dashboards are separate work. The next section settles how.

## Turns, settled in concept review (2026-11-16, headless)

The direction above ("one row per turn ... written once when the turn ends")
meets the slice 3 code at one point it did not account for: a turn is read
while it runs. `pending_inputs` (running/finished on the Session list and
Desktop's `SessionHistory`), `input_answered_turn` (Graph routing),
`session_turn_completed`/`session_completions` (replay guard) and Process Task
membership (`processes.rs:497`) read the `started` row, and
`record_session_turn_origin` freezes the turn's Task, Wave, input and Process
at dispatch on purpose ("Freeze the request's attribution while its attachment
still owns dispatch"). So:

- **A turn is one row in `usage.turns`, inserted when the turn starts and
  completed in place when it ends.** The `started`, `usage` and `completed`
  history kinds are not written anywhere else; `history.session_history`
  keeps only `observed` rows (its `kind` CHECK narrows to that under
  `user_version` 2). No reader self-joins three rows into a turn again.
- **Columns.** Key: Session, provider thread, provider turn (unique). At
  start: input (`captured_event`), lf and agent Process ids, Task, Wave,
  Project, repository, skill, Graph and step, `started_at`. At end:
  `ended_at`, status, provider and model as reported, the token counts
  (input, cached input, cache write, output, reasoning, context window) and
  the turn context `completed.context` holds today (first and peak request
  tokens, tool results, tool output tokens, compactions, compaction tokens),
  each its own column. Grouping values are the ones frozen at dispatch, not
  at turn end; they differ only for a bind made mid-turn.
- **Usage counts are resolved before the write.** `native_usage` turns
  Codex's thread-cumulative `total`/`last` and OpenCode message readings into
  one turn's `TurnUsage`; the turn row stores that result, so a report is a
  `SUM` with no counter logic. A turn whose usage arrives after it completed
  (replayed snapshot) updates the same row once; a second, different reading
  is a gap that keeps the first, as receipts do now.
- **One writer, two files.** `history.rs`'s `HistoryFile` (own connection,
  read-only `loopflow.db`, `RecordingGap` on failure) is opened once per file
  with that file's name and schema; `usage.db` gets no second writer type.
  Every store connection attaches `usage.db` read-only as `usage`, with the
  same empty in-memory stand-in when it cannot.
- **Readers repointed:** `pending_inputs` and the summary fragments in
  `sessions.rs:1100–1200`, `recorded_turn_end`, `session_turn_completed`,
  `session_completions`, `input_turn_context`, `input_answered_turn`,
  `session_turn_facts`, `read_history`'s turn arms, `native_usage`'s callers
  (Telemetry, Task history, `lf usage`), `processes.rs:497` and `:2475`.
- **`lf session history <id>`** shows the Session's turns (from `usage.turns`)
  and its observations (from `history.session_history`) as two lists; the
  observation feed stays append-only by `seq`, and a turn is a record that
  changes once, so it is not a feed entry. `--json` changes shape on purpose
  (`turns`, `events`); Desktop's `SessionHistory` DTO keeps its shape and is
  built from turns.
- **Wake-ups.** `usage.db` has its own revision row; `StoreRevisions` reads
  it as `turns`. `changed()` ignores it except for reads that name `usage.*`
  (Session list running state, Session History, Telemetry, Task history),
  the same rule as `history`.
- **Existing rows and skew are one pass.** The backfill that builds turn rows
  from `started`/`usage`/`completed` (paired per Session, thread and turn,
  grouping from the `started` row's origin, model where known) reads both
  `history.session_history` and main `session_events`, then deletes what it
  converted. Run by slice 5's move pass, it also carries the rows a released
  lf keeps writing into `loopflow.db` after install.
- **Retention:** no pass deletes `usage.turns` rows (open in `questions.md`:
  indefinitely or a long window; the choice is one constant later).

Tests to show it: a turn is readable as running between start and end and
from `usage.turns` only (`history.session_history` has no turn row); a Codex
turn's cumulative reading is stored as its own delta; the backfill converts a
seeded Session's three rows into one turn and `pending_inputs` reads the same
before and after; a locked `usage.db` makes a gap, not a failed turn.


## Turn table as built (2026-11-16)

- `history.rs` describes each file as a `Journal` (name, alias, schema,
  `user_version`); `HistoryFile` is the one writer, opened per journal.
  `SqliteStore` holds `history` and `usage` writers; every store connection
  attaches both read-only (`history`, `usage`), each with an empty in-memory
  stand-in when unreachable. `history.db` is `user_version` 2: fresh files
  CHECK `kind='observed'`; a version-1 file keeps its wider CHECK and its two
  turn indexes (a journal only adds), and its turn rows convert.
- `usage.turns` (`sqlite/turns.rs`): one row per (Session, thread, turn).
  `record_turn_start`, `record_session_turn_origin` (freezes Task, Wave,
  Project, repository, skill, provider, Graph and step label at dispatch),
  `record_turn_usage` (resolves the reading with `resolve_usage` before the
  write), `record_turn_completion` (status, duration, error, context columns).
  First reading kept; a different second usage or completion status is a gap.
  `record_session_event` and the `Started`/`Usage`/`Completed` kinds are gone.
- `resolve_usage` replaces `native_usage`/`UsageReading`: a `usage` object is
  the turn's; Codex `total` less `baseline` (else the prior turn's total, else
  `total − last`); OpenCode `message.tokens` one call. Codex transcript
  readings now carry `"partial": true` when counted from the first report.
  `reduce_usage` sums turns; a gap is a turn with no start or a partial reading.
- Readers on `usage.turns` only: `pending_inputs` (window arms, ended/latest,
  scope filters), `turns_matching` (input and orphan turns for
  `read_history_input`), `session_turn_facts`, `recorded_turn_end`,
  `session_turn_completed`, `session_completions`, `input_turn_context`,
  `input_answered_turn`, `session_turns`, Process membership
  (`processes.rs` performed-work arm, `task_work.rs` three arms,
  `process_session`, `FORGETTABLE`). `read_history` reads observations only.
- `lf session history <id> --json` is `{"turns": [...], "events": [...]}`;
  text prints turns first. `ProviderHistoryReference::NativeTurn` keeps its
  shape: `start_seq`/`completion_seq` hold the turn's id when started/ended.
- `StoreRevisions.turns` reads `usage.turns_revision`; `changed()` ignores it
  like `history` (no read declares either yet: slice 5).
- `convert_turn_rows` (run by `lf process prune`, reported as
  `converted_turns`) turns `started`/`usage`/`completed` rows from main and
  `history.db` into turn rows per Session thread, then deletes them.
- Not kept from old completion payloads: `result_id`, `subtype`,
  `total_cost_usd`, `started_at`/`completed_at` in the payload (no reader).

Check results (2026-11-16, load average ~140): storage modules touched
(`turns`, `session_events`, `revisions`, `history`, `sessions`, `processes`,
`task_work`, `query_plans`, `retention`, `provider_transitions`,
`task_retirement`): 79 passed; `process_membership_does_not_read_events_that_name_no_process`
timed out at 300 s in the run and passed alone in 250 s (load; gate/CI).
Execution modules reached (`transcript`, `store::*` lifecycle and history,
`session_record::capture_tests`, `harness::{fact_writer,dispatch_tests,
conformance_tests,pi_tests,claude,codex}`, `read_stream`, `ops::task::tests`):
214 passed after fixes.

Swift has no reader of turns; its `SessionEvent` kinds narrowed to
`captured`/`observed` and its fixture test reads `events`.

## Concept review after slice 4 (2026-11-16, headless)

The built turn table matches "Turns, settled in concept review": one row,
columns not JSON, resolved usage, readers on `usage.turns` only, one writer
for both files, history narrowed to `observed`. Two things the design
implies, for slice 5:

- **Name the writer for what it is.** `HistoryFile` now writes `usage.db`
  too (`HistoryFile::beside(&USAGE, …)` reads wrong). Rename it `JournalFile`
  and the module `history.rs` → `journals.rs` (it holds `HISTORY`, `USAGE`,
  `attach`, `open_writer`); `history_path`/`usage_path` stay. 11 uses, 15
  importing files.
- **Upgrade skew is detect-and-name**, settled above under "Upgrade skew";
  no compat reads.

Still open, none blocking slice 5: turn-row retention (one constant), and
whether reports group by the values frozen at dispatch or follow later binds
through the Session id (`questions.md`); "interrupted" for an unknown
OpenCode completion.

## Slice 5 as built (2026-11-16)

- `JournalFile` in `journals.rs` (was `HistoryFile`, `history.rs`).
- `retention::prune(database, dead, now)`, run by the Machine server at start
  and hourly (`server/maintenance.rs`, beside the daily statistics refresh,
  which now also optimizes `history.db` and `usage.db` through their writers).
  Order: close dead Processes; in `loopflow.db` carry `provider-session:*`
  onto the captured row and `interactive_opened` onto `opened_at`, null
  `processes.attachment_exit_seq`, delete unread observations
  (`UNREAD_OBSERVATION`) and `observed`/`output` rows past 90 days; move the
  remaining `observed` rows to `history.db` in seq batches (`INSERT OR
  IGNORE`, then delete); convert turn rows; delete `history.session_history`
  past `HISTORY_DAYS` = 90 and reclaim `history.db`; forget Processes;
  reclaim main. `FORGETTABLE`: a history or turn row pins a finished Process
  only until 30 days after it finished. Turns: no retention.
- A released lf's transitions moved by the pass do not update
  `agent_sessions.transition` (the summary's latest); detect-and-name skew.
- `read_stream::Watch` (`STATE`, `JOURNALS`) is an argument of
  `start`/`run`; Session list, `lf history` (Desktop Session History) and the
  Task view/status watch journals, every other subscription state only.
- `lf doctor` `journals` check: both files' size and oldest row; warns when
  history is past 92 days. `lf process prune` prints moved/forgotten counts.
- Docs: `data.md` State and history and owner table, `telemetry.md`,
  `troubleshooting.md`, `index.md`, `environment.md`, `machines.md` now name
  `usage.db`, the server's hourly clock, and no Process move.
- The live-copy run found `retain_task_agent_turn` refusing to delete a
  Task's last `started` row, so turn conversion failed on a real store; the
  migration drops the trigger (a Task's work is its turns now).
- Tests: `retention::history_tests::prune_moves_old_shape_history_out_and_forgets_only_aged_rows`
  (seeded old shape → new files, second pass idempotent),
  `read_stream::tests::a_turn_reaches_a_subscription_that_watches_turns`,
  `history_and_turns_reach_only_the_reads_that_watch_them`.
- `reclaim` stepped `PRAGMA incremental_vacuum` once through
  `execute_batch`, which freed nothing on the incremental store; it now steps
  it to the end in 10,000-page transactions.

Measured 2026-11-16 on a `.backup` copy of `~/.lf/loopflow.db` (the
storage `prune` called directly; no agent reaping): migration 6.6–18 s; first
prune 58 s: 116k observations moved, 30,469 turns converted, 105k rows
forgotten, 33 Processes forgotten; second prune 0.5 s, all zeros. Files:
`loopflow.db` 997 MB → 607 MB after reclaim (10.5 s, four chunks before the
chunk was cut to 10,000 pages; the rest is mostly `processes`, LOO-953),
`history.db` 119 MB, `usage.db` 21 MB. `session_events` left: 21,601
`captured`, 24,141 `output` (gone at 90 days). Single statements on the copy:
null `attachment_exit_seq` 0.7 s, one 10k unread-delete batch 0.7 s, one 10k
move-delete batch 0.3 s. After `ANALYZE`, every journal read checked plans
`SEARCH` (`turns` by session, input, start/end window, Task, Process;
`session_history` by receipt, order, window). Revision rates after the change
are not measurable until this build is installed (the live Machine runs the
released lf).

Check results: storage `retention`/`turns`/`journals`/`revisions`/forget
tests 20 passed; storage+execution `retention`, `read_stream::tests`,
forget tests 13 passed; `documented_commands::` 4 passed; `cargo clippy --all-targets -- -D warnings` clean. Gate: `binary(served_reads)` (server maintenance), `test(store::query_plans::tests::)`, `test(session_cli_tests::)`.
