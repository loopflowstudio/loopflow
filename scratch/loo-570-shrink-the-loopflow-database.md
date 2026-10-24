# LOO-570: Shrink the Loopflow database and simplify installation

Status: slices 1–3 implemented (2026-10-24). Jack Heart delegated the decisions
(October 23); assumptions sit in `scratch/questions.md`.

## Problem

`lf install` of v0.14.15 took minutes on Jack's 9.8 GB `~/.lf/loopflow.db`.
Install walks the whole store repeatedly:

- a whole-database `foreign_key_check` in preflight (`validate_sqlite`; about
  100 s cold on this store);
- a backup-API copy to migrate a private snapshot, plus another
  `foreign_key_check` there;
- a backup-API pre-migration backup;
- a final `foreign_key_check` before commit.

Each cost scales with the store, and the store is mostly provider text lf never
reads back. Jack Heart: "i still think we shouldnt be storing 1gb of text in an
lf db." The target is a store of facts and references.

## Demo

```
lf install                   # Jack's store: seconds (preflight FK check 0.3 s)
lf disk clean                # runs the retention prune now, instead of within the hour
ls -la ~/.lf                 # loopflow.db ≈ 550 MB, no multi-GB loopflow.db.backup-*
lf history usage --weekly    # same weeks, tokens and silences as before the prune
lf history show <input> --events   # verbatim stream from runs/<capture>/events.jsonl
```

## Findings

### Q1: What reads `session_events`

Writers: `SessionRecorder` (`session_record.rs:599-650`) appends every capture
event to `runs/<capture>/events.jsonl` **and** retains most of them in SQLite as
`observed` rows `<input>:events.jsonl:<seq>` (`StreamedHistory::admit` already
drops token increments). Other `observed` rows are small facts: `manifest.json`,
`terminal.json`, `provider-session:*`, `:exit`, `codex_submission`,
`codex_relay`, `interactive_opened`, `attachment:*:exit`, `agent-process:*`,
`legacy_review_feedback`.

| Reader | Needs | Full stream body? |
|---|---|---|
| `lf history` / Desktop (`RegistryQuery.swift:296` → `lf history --json`), `project_input_history`, `conversation_inputs`, `summary_for_input` | manifest (task/pr/repo/cwd/skill/harness/model/surface/flow), terminal outcome, provider_attempt_*, account_selected, provider_session_observed, usage, native started/usage/completed/output | No. SQL already excludes transcript types (`session_events.rs:336-340`) |
| Resume, continuation, account attribution (`provider_session_from_history`; `human_session.rs`, `conversation_log.rs:42`, `restore.rs`, `primary.rs`, `connection.rs`, `agent/session.rs`) | provider-session rows, attempt/account/session_observed envelopes | No |
| Input membership (`session_input_membership` index, `sessions.rs:237`) | manifest `schema_version`, `artifact_key`, `flow.kind` | No |
| Graph step failure (`graph.rs:531-561`) | `conversation` events `turn_completed{status}`, `error{code,message}` | No: two small event types |
| `lf history usage --weekly` silences (`context_cost.rs:176` → `input_event_times`) | `observed_at` of every stream event | No: only gaps over 600 s |
| Final answer (`input_final_answer`) | native started/completed/output | No `observed` rows |
| Available / attention | `session_activity`, written from live provider streams | No `session_events` at all |
| Task work (`task_work.rs:66-100`) | `lf_process_id`, `session_id` | No |
| Codex submission recovery (`codex_connection.rs:273`) | `codex_submission` rows | No |
| `lf history show --events` (`session_history.rs:110`) | every envelope, printed | Yes: a debug dump |
| `lf session history` (`session.rs:147`) | every row, printed | Yes: a debug dump |

Session closing is `agent_sessions.completed_at`, set only for disposable
attachments and primary replacement; nothing reopens it. On Jack's store
5,384 of 5,669 Sessions never got it.

### Q2: Is lf's copy different from the provider's?

Two headless Sessions:

- **Claude** `session_e5606d0d…`; native `~/.claude/projects/…/e8917bb4….jsonl`
  (32 lines, 439 KB).
- **Codex** `session_d59484ad…`; native
  `~/.codex/sessions/2026/10/23/rollout-…01a16b4b-fff2….jsonl` (41 lines, 447 KB).

| Fact | Claude: lf rows | Claude: native | Codex: lf rows | Codex: native | Verdict |
|---|---|---|---|---|---|
| User/assistant messages, tool calls and results | `provider_output` user/assistant (24 KB), `conversation` (3 KB) | yes | `item/started` and `item/completed` (249 KB), `conversation` (178 KB) | `response_item`, `item_completed` | Duplicate, twice in lf (raw line plus normalized `conversation`) |
| Input prompt text | `user_input` (17 KB), manifest `exec` (20 KB) | yes | `user_input` (27 KB), manifest `exec` | yes | Duplicate, three times (the `runs/` manifest also holds it) |
| Token usage | `usage` envelopes | `cost-state` | `usage` envelopes, `thread/tokenUsage/updated` | `token_count`, `token_usage_record` | lf extract kept; raw line duplicate |
| Hooks, `system/init` tool list, thinking-token estimates, `rate_limit_event`, `result/success`; Codex MCP startup, rate limits, thread status | `provider_output` | no | `provider_output` | no | Only in lf; **no reader** |
| Process-tree samples (`activity`) | 2 rows | no | 3 rows | no | Only in lf; no reader |
| Attempt start/finish, account selected, provider session observed | envelopes | no | envelopes | no | Only in lf; **read** |
| Receipts, Process and Task attribution, captured input | `captured`, `started`, `completed` rows and columns | no | same | no | Only in lf; read |

Every fact that only lf owns, and that something reads, is already a small
envelope or a column. The bulk is either a duplicate or unique and unread.

### Q3: A provider-aware reader over native files?

Not for the store. Readers that need conversation content already read native
files (`conversation_log.rs`). No fact reader needs a body, so nothing needs to
route to provider files. The two debug dumps read lf's verbatim
`runs/<capture>/events.jsonl`, which the recorder already writes and Home
rotation already bounds (`disk.rs`).

| Alternative | Why not |
|---|---|
| Keep while open (Jack's candidate) | Frees almost nothing, since nearly every Session stays open. No reader needs bodies for open Sessions either |
| Slim extract plus truncated payloads | A second, lossy copy of `events.jsonl` |
| Compressed payload files beside the store | Moves data that has no reader |
| Provider reader | Retention varies: Claude prunes by `cleanupPeriodDays`, Codex never rotates. The files exist only on the Machine that ran the agent |

Once nothing in the store depends on provider files, their rotation and remote
Machines stop mattering to it.

### Migration cost (probes on `cp -c` clones of Jack's store)

| Step | Time |
|---|---|
| APFS clone of the 9.8 GB store and WAL | < 1 s |
| In-place `DELETE` of 2,237,877 transcript rows (foreign keys off) | 279 s |
| Rebuild table instead (copy kept rows, drop, reindex) | 224 s, then 100 s FK check |
| Strip `exec` from 5,754 manifests | 7 s |
| Expire 848,734 inspection Processes | 9.5 s |
| `VACUUM` at 1.1 GB / 500 MB live | 16 s / 3 s |
| `foreign_key_check` at 9.8 GB / 500 MB | ~100 s / 0.29 s |

The cost is IO-bound (10% CPU). Freeing 6.6 GB of overflow chains means reading
them at random, and no form of in-place SQL avoids that. As a release migration
it would hold `BEGIN EXCLUSIVE` on the live store for about 5 minutes, with
running agents' writers timing out after 15 s
(`SQLITE_WRITE_BUSY_TIMEOUT`). It would also run twice per install: once on the
preflight's private copy, once for real. **So retention runs as a batched
background prune, not a migration.**

## Chosen approach

**The store keeps facts; streams stay in files.** A capture's stream is recorded
verbatim in `runs/<capture>/events.jsonl` (bounded by Home rotation). SQLite
retains only what a reader uses. One retention rule applies at write time
(recorder) and at prune time (existing rows).

1. **Recorder retains facts only** (`session_record.rs`). One predicate,
   `retained_in_history(&CaptureEvent)`, keeps these in SQLite:
   - `provider_attempt_started`, `provider_attempt_finished`,
     `provider_account_selected`, `provider_session_observed`, `usage`;
   - `conversation` events of type `turn_completed` or `error`.

   Everything else is written to the file only: `provider_output`, other
   `conversation` events, `activity`, `handoff`, `user_input`, `text`,
   `tool_use`, `result`. Delta coalescing (`StreamedHistory.run`/`diff`) existed
   only to shrink retained transcript rows, so it goes.
2. **Silences become a recorded fact.** When an event arrives more than
   `SILENCE_SECONDS` (600 s; moves from `context_cost.rs` to `session_record.rs`)
   after the previous one, history retains `<input>:silence:<seq>` with evidence
   `{since, observed_at}` (unix seconds). `input_event_times` becomes
   `input_silences`, which returns gaps; `context_cost::silences` still filters
   them by turn window.
3. **The retained manifest drops `exec`.** Its prompt text has no history reader.
   Replay and `--context`/`--instructions` read `runs/<capture>/manifest.json`.
4. **Debug dumps read the file.** `lf history show --events` prints
   `runs/<capture>/events.jsonl` when it exists. Otherwise it prints the
   retained envelopes and writes "stream file rotated; retained facts only" to
   stderr. `lf session history` is unchanged; it now prints facts.
5. **Retention prune** (`store/sqlite/retention.rs`,
   `SqliteStore::prune_retention`). It runs from `disk::rotate_when_due`'s
   hourly in-process thread, which adds no Process history, and on demand from
   `lf disk clean`. It works on its own connection with `foreign_keys` off, as
   `_migration_transaction` does. Its predicates delete only rows that nothing
   references.
   - **Captures:** in capture order from a cursor file beside the database
     (`loopflow.db.retention-cursor`, so a draft copy keeps its own), one short
     transaction per capture, stopping at the first capture under 24 h old (an
     earlier lf may still be writing it). Only a capture holding rows outside
     the retained set is touched, so captures recorded under the new rule never
     get silences derived from their sparse facts. Each transaction:
     - inserts that capture's `silence` rows (`INSERT OR IGNORE`). They come
       from `LAG` over the inline `receipt_key` ordinal and the `observed_at`
       column, so no payload is read;
     - deletes its `events.jsonl` rows outside the retained set;
     - strips manifest `exec`.

     Nothing references these rows: `captured_event`, `current_capture` and
     `attachment_exit_seq` point only at captured and attachment-exit rows.
     Revision triggers already exempt transcript rows, so the prune wakes no
     Desktop refresh.
   - **Inspection Processes:** deletes finished `lf` Processes older than 7 days
     that nothing references. That means no child or attachment, and no row in
     `session_events`, `graph_processes`, `graph_process_steps`,
     `task_workflows`, `task_workflow_moves`, `waves`, `ci_incidents`,
     `agent_sessions` (both columns), `relocated_task_processes` or `steer_*`.
     Batches of 10k.
   - **Reclaim:** when `freelist_count > page_count / 2`, the prune runs `PRAGMA
     auto_vacuum=INCREMENTAL; VACUUM` once, which takes about 3 s at 500 MB, well
     under the 15 s writer timeout. After that, each pass ends with `PRAGMA
     incremental_vacuum`. A busy VACUUM is logged and retried next pass.
6. **Install copies are clones.** `_copy_store_for_candidate` becomes
   `store::copy::snapshot`. `backup_before_migration` uses `snapshot` and always
   writes a fresh clone, so the reuse path goes, and with it `valid_backup` and
   its whole-file `integrity_check`.
7. **Backups are pruned when their release settles.** `settle_switch`
   (`install.rs:2352`) removes the fingerprinted `loopflow.db.backup-*`
   generations the migration writer produced, the set `prune_migration_backups`
   recognizes. After a committed migration, `prune_migration_backups` keeps one
   generation, the one just made, instead of two. Home rotation's
   newest-per-database rule stays for hand-named backups.

No schema migration. The revision-trigger clauses that exempt transcript rows
stay while the prune deletes old ones. The follow-up below removes them.

## Measures (`cp -c` clone of Jack's store, 2026-10-24, release-build prune)

| | Before | After |
|---|---|---|
| `loopflow.db` | 9.8 GB | 550 MB (first prune plus VACUUM: 245 s) |
| `session_events` | 8.0 GB table + 0.9 GB indexes | 265 MB |
| `processes` | 936,568 rows, 409 MB + 253 MB indexes | 87,076 rows, 49 MB |
| Preflight `foreign_key_check` | ~100 s | 0.28 s |
| `lf install preflight --json` | minutes | 5.1 s (debug build) |
| `lf history usage --weekly` | 53 min (0.14.15) | 107 s (debug build) |
| Preflight / backup store copy | backup API, minutes | APFS clone, < 1 s |
| `~/.lf` migration backups | 9.1 GB (`backup-0.14.12.001_release-…`) | none after settle |

What stays, and why:

- Usage envelopes: 140k rows, 81 MB. The reducer takes the peak over every
  observation (see `scratch/questions.md`).
- Native `usage` rows: 34 MB.
- `processes` inflow: about 25k Desktop polls a day (`task file`, `task diff`).
  At steady state that is about 175k rows for 7 days, roughly 60 MB.

Outside this Task: `accounts/codex` (17 GB of per-account Codex homes) and
`copies/` (18 GB of draft-store copies, already rotated by `disk.rs`).

## Contract

- **Source of truth:** `session_events` holds facts.
  `runs/<capture>/events.jsonl` holds the verbatim stream. Provider files hold
  conversation content.
- **Absent states:**
  - A rotated stream file: `--events` prints the retained facts plus a stderr
    note.
  - A capture not yet pruned reads as it does today.
  - A pruned capture keeps its silences through the backfill.
  - A capture whose recorder died keeps what it already retained.
- **Operational boundary:** each prune transaction holds the write lock well
  under the 15 s writer timeout. Pruning Jack's store takes about 5 minutes of
  background IO, once. Steady-state hourly passes over facts take under 1 s.
- **Affected surfaces:**
  - `lf history show --events`.
  - `lf session history` (prints fewer rows).
  - `lf history usage --weekly`.
  - `lf disk clean`, which now also prunes the store.
  - Install preflight and settle.
  - Unchanged: Desktop history, because the summary path already excludes
    bodies.
- **Exclusions:**
  - Codex/Claude account homes and `copies/`.
  - Desktop's polling cadence, and whether its polls should be Processes.
  - Usage-envelope compaction.
  - `lfd*.db` leftovers.
- **Testing:** a branch build without drafts opens the shared store directly.
  Run prune fixtures and probes under a custom `LF_HOME` holding a `cp -c`
  clone, never on `~/.lf`.

## Delete — do not maintain

- `StreamedHistory` delta and diff coalescing (`session_record.rs:449-560`) and
  its tests. `is_provider_increment` stays only if `events.jsonl` writing uses it.
- `input_event_times` (replaced by `input_silences`).
- `_copy_store_for_candidate`, the backup-API branch of `backup_before_migration`,
  `valid_backup`.
- `MIGRATION_BACKUPS_KEPT = 2`.
- **Follow-up Task (file with the PR):** once Jack's Machines report no
  unpruned captures, delete the prune's capture pass and the revision-trigger
  transcript exemptions (a draft migration).

## Forbidden outcomes

- A second store for payloads, whether compressed files, a sidecar database or
  a truncated-body column.
- A "keep while open" mode beside the type rule.
- Dual-writing transcript rows during a transition.
- A release migration that deletes transcript rows under `BEGIN EXCLUSIVE`.
- A preflight that vacuums or keeps its private copy.

## Slices

1. **This slice: recorder and readers.** Approach steps 1–4. Focused test: a
   recorder test feeds provider_output, conversation item and turn_completed
   envelopes plus a 700 s gap. SQLite keeps turn_completed, the attempt facts
   and one silence; `events.jsonl` keeps everything. Graph failure detection and
   `context_cost` still see their inputs.
2. **Retention prune** (step 5). Focused test: a seeded store with a legacy
   capture, a referenced and an unreferenced old Process, and a recent
   inspection Process. The prune deletes exactly the unreferenced old rows,
   backfills silences, and `foreign_key_check` stays clean.
3. **Install** (steps 6–7). Focused test: the migration backup is a clone and
   settle removes it (existing `migrations.rs` backup and `install.rs` settle
   fixtures).

## Done when (gate, headless)

- With inherited `LF_*` unset and `< /dev/null`:
  `cargo test -p loopflow --lib session_record context_cost session_events retention migrations install`
  passes, and so does `cargo test -p loopflow --test integration` for the
  history and install suites.
- Under `LF_HOME=<dir with cp -c clone of ~/.lf/loopflow.db>`, the branch build's
  `lf disk clean` prunes the clone. Then `lf history usage --weekly --json`
  matches its pre-prune output (same steps, tokens, silence counts), and the
  file is ≤ 600 MB.
- In the same home, `lf install preflight --json` finishes in under 5 s.
- After delivery, measured on Jack's store: size after the first hourly prune,
  and the time of the next `lf install`.

## Remaining

- Gate: the affected suites in "Done when". The clone probe's debug-build
  preflight took 5.1 s against the 5 s target; the gate re-times it with a
  release build. Its current-week silences were 60 vs 79 before the prune
  because 108 captures were under 24 h old; the gate's comparison runs after
  they settle, or excludes that week.
- With the PR: file the follow-up Task (delete the capture pass and the
  revision-trigger transcript exemptions once Machines report no unpruned captures).
- After delivery: Jack's store size after the first hourly prune, and the next
  `lf install` time.

## Proposed Wave memory update (infrastructure, not applied)

Replace "Storage footprint (LOO-390)"'s open question ("Next condition is Jack's
retention choice") with LOO-570's answer: SQLite keeps facts; the verbatim
stream lives in `runs/<capture>/events.jsonl` under Home rotation; a background
prune (not a migration) converts legacy captures and expires unreferenced
inspection Processes; install copies are APFS clones and settle removes
migration backups. Lesson: freeing GBs of overflow pages is random-read IO, so
it can't run under a release migration's `BEGIN EXCLUSIVE`.

## Check results

- 2026-10-24 kickoff: read-only probes on `cp -c` clones of `~/.lf/loopflow.db` (figures above).
- 2026-10-24 clone probe (release prune on a `cp -c` clone of Jack's store): 5,631 captures, 2,222,659 stream rows, 849,534 Processes pruned and VACUUM in 245 s; 9.8 GB → 550 MB (`session_events` 265 MB, `processes` 49 MB, 87,076 rows); `foreign_key_check` 0.28 s clean, `integrity_check` ok; second pass 55 ms; debug-build `lf install preflight --json` 5.1 s; `lf history usage --weekly` 107 s after (debug) vs 53 min before (installed 0.14.15); steps and tokens identical, current week 60 vs 79 silences: 108 captures under 24 h old await the prune's settle window, so their silences appear on the next pass after they settle.
- 2026-10-24 implement: `cargo test -p loopflow --lib -- retention session_record::tests::history context_cost session_events migrations::tests::{a_committed,release_migration,backup_snapshot,divergent_repair}` 27 passed.
- 2026-10-24 compress: `retention::prune` derives its own cursor path; `cargo test -p loopflow --lib -- retention` 2 passed, clippy clean.
- 2026-10-24 realign: no upstream changes since `cc91c3f3ff`; plan matched the code, so nothing was rerun.
- 2026-10-24 concept review: no change. Facts in the store, streams in `runs/`, prose in provider files: one source each, and the code matches. The retention rule exists twice (`retained_in_history` and the prune's `RETAINED` SQL); the follow-up Task deletes the SQL copy along with the capture pass.
