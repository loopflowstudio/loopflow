# LOO-953: no Process table; liveness on Sessions and Graph runs

## Status (2026-11-18, journal stopped; next: slices 4–6)

LOO-936 (#1965), #1964 and LOO-935 (#1958) are merged and synced in. Built,
in order: slice 2a (Graph runs and steps own their results), the owner and
carrier slices (activations, CI repairs, Task and ship carriers), the Session
lifetime and attachment cuts of slice 3, the handoff deletion, the lf-row
readers, and now the journal stop below. History of each pass, with its
checks: `git show fbcf0091f2:scratch/loo-953-remove-processes.md`.

Next, in order: (3) slices 4–6 below, slice 5 testing only the old → new
bridge. Contract Task: LOO-965 ("Drop the Process table LOO-953 marked
leaving"). Quotes and the OS-scan measurement: `questions.md`.

**Journal stop as built (2026-11-18).** No lf code inserts a Process row;
only the SQL bridge does, for a released lf.

- The journal records no rows and no receipts. `record_end` writes a run's end
  on the owners it marked (`run_owners`) at its terminal event and opens the
  store only then; a command owning none writes nothing. Deleted:
  `record_process_in`/`_within`, `record_hosted_process`,
  `process_is_recorded`, `open_existing_processes`, `observe_process` (and
  its three `lf.rs` callers), `parent_is_recorded` (an inherited parent is
  kept whenever the trace is inherited), the receipt type and every receipt
  function, `ReceiptCost` (and wt timing's receipt samples),
  `journal_trace_locator`/`process_receipt_locator`/`process_for_receipt`,
  `FORGETTABLE` and its tests. `agent_caller()` reads `LF_AGENT_CALLER` before
  admission, so `lf open` needs no observation. A read admits no Process, so
  `with_process` creates the context at exit only for a minted Graph step
  (`LF_EXECUTION_ID` set), which then writes its own end and error.
- `attach_in` no longer lands the attaching lf's row; the agent row a
  released reader joins takes its trace from the agent id when its launcher
  has no row.
- Synthetic holder rows are gone (`terminal_holder::client::{record, adopt,
  settle_departed, holder_process}`); nothing read them except `lf ps`.
- Liveness pass (`sqlite::retention::running_owners` → execution's `prune` →
  `end_dead_owners`): unended Graph runs and steps, activations, Task and ship
  carriers with a pid, and Session agents with a pid; dead (or started before
  boot) ends through `finish_run_in` with no exit and no error, a Session agent
  as `exited` (headless runs complete). Re-read in the write transaction, so a
  row changed since observation is left. `close_dead_processes` and
  `forget_processes` deleted; `RetentionReport.ended_owners` replaces their
  counts. CI repairs are left out: nothing ends them on a Process end today
  either. Index `graph_process_steps_unended`.
- `lf binaries`/doctor/install skew inventory reads `recorded_lfs()` (owner
  rows with a pid, plus each Session's attached lf) and, for a released lf,
  unfinished Process rows; deduped by pid.
- `lf ps`/`top`/`process prune` (slice 4 targets) lost their receipt arms only.
- Migration: the generated block at the end of `process_rows_on_owners.sql`
  rebuilds `agent_sessions`, `ci_incidents`, `desktop_shells`,
  `session_events`, `steer_adapters`, `task_graphs`, `tasks`, `waves` without
  their `processes` keys, keeping every column, index and trigger
  (`scratch/rebuild_without_process_keys.py` regenerates it; output matches).
  `steer_batches` is skipped: it has no writer and leaves with LOO-954.
  Marks: `-- leaving: processes`, `relocated_task_processes`,
  `agent_sessions.transport_lf_process_id` → LOO-965. No new → old trigger for
  Graph lf rows; `process_agent_from_session_*` and
  `process_terminal_from_session` stay.
- Cost on a `.backup` copy of the real store (364,671 `session_events`,
  22,379 Sessions, 1,395 Tasks): `session_events` rebuild 4.3 s,
  `agent_sessions` 0.3 s, `tasks` 0.09 s.
- Fixtures: `recorded_graphs` and the Task-run `ran` wait read Graph runs,
  steps, carriers and ship events (the carrier records no exit code, so a
  run's code is its shipping's, else its last Graph run's); step `argv`
  became the compiled `label`.

Checks (2026-11-18): storage `migration_runner|query_plans|sqlite::{retention,
processes,sessions,graph_inventory,task_work,durable,steers}|canonical_ids|
revisions|task_retirement|fixture_tests` → 218 pass after three fixture
fixes; execution `journal|store|harness|process|ops::{task,graph_process,
pr_landing,project,human_session,wt_timing}|transport::registry|
session_record|terminal_holder` → 719 pass after four fixture fixes;
loopflow `graph_tests|task_graph_launch_tests|explain|world_recipes|
server::{boundary,wake}|subscription_measurement|commands::{top,binaries,
machine}|doctor_tests|store_contention|binary(agent_tests)` → 137 pass;
`task_graph_launch_tests|graph_tests|top` rerun → 86 pass at load 77.
`completion_preserves_retained_session_input_and_unknown_process_history`
failed once in a parallel run (`desktop task --json` exited non-zero with no
stderr, load ~70) and passed alone twice and in the rerun: gate reruns it.
clippy, fmt, `check_migrations.py`, `schema_page.py --check` → clean.
Gate/CI: `binary(session_lifecycle_tests)`, `process_ownership_tests`,
`session_cli_tests`, `work_open_tests`, `binary(two_people)`,
`binary(transport_tests)`, `test(transport::)`, `test(performance_tests::)`,
`binary(lf)`, `local_planning`.

**Decisions still live** (concept reviews, 2026-11-17/18; detail in git):

- No new → old lf-row bridge: a released Graph runner stops at its next
  step's schema check (v0.16.0 `validate_current_schema()` after each step).
- `transport::held_in()` reads `LF_HELD_TERMINAL` alone.
- A carrier's end keeps `lf_process_id` (the next run sets out from it); one
  `lf task run` writes its pid on `task_graphs` and, after handoff, its ship
  event.
- One record type for "one lf run" (`Execution`); `LF_EXECUTION_ID` (adopt
  this id) and `LF_PROCESS_ID` (the lf you run under) stay two variables.
- `lf ps` and `lf top` are deleted (2026-11-17: "i thikn we delete lf ps?");
  `lf session list` and `lf graph list` show what runs. `lf graph show
  <RUN_ID>` stays ("what else would it accept").
- Wire names still saying Process (`GraphStepProcess`,
  `work.graph_processes`, `graph_process.rs`) rename with the Process-line
  slice in one DTO change; `RecordKind::GraphProcess*` goes with the export
  in slice 4.

## Problem and outcome

`processes` holds a row per `lf` invocation and per provider process (184,423
rows on 2026-11-16; 238 unfinished, 76 of them agents) and does three jobs:
registry of what runs, result of what another record points at, log of what
ran. After: each owner record carries its own pid, OS start time and result;
"alive" is one OS lookup against the owner's pid; no log in the store.

## Demo

```bash
lf session list               # live Sessions, each OS-confirmed; no call tree
lf graph list                 # Graph runs, live ones marked from their own pid
lf task status LOO-953 --json # Graph state from graph_processes' own pid and exit
sqlite3 -readonly ~/.lf/loopflow.db "select count(*) from processes"   # stops growing except bridged rows
lf machine doctor             # "strays": OS processes with LF_* vars no live row claims
```

## Readers of `processes` left (scan 2026-11-18: 30 statements)

`python3 -I scratch/process_sql_scan.py rust` (writes `/tmp/loo953/scan.json`;
create the directory first). Production: the bridge and
`forget_unread_rows`' `attachment_exit_seq` clear (J, both leave with the
table); `binaries.rs`' released-lf arm, `canonical_ids` preflight and
`query_plans` arms (LOO-965); slice-4 deletions: `lf ps`/`top`/`process`
(`top.rs`, `unfinished_processes_except`, `process_query`, `resolve_process`),
history (`processes`/`ProcessCursor`), `lf_process_ids`, rental export
(`machine_records.rs`), relocation (`relocate_repository`,
`relocated_task_processes` reads in `task_work.rs`), doctor's
`check_lineage`. Everything else is a fixture.

## Target records

Process ids stay as opaque UUIDs (`LF_PROCESS_ID`, Graph run id, step id,
agent generation id); no table holds them. Column sets, each with its reader:

| Owner | New columns | Readers |
| --- | --- | --- |
| `agent_sessions` | `attachment_token`, `attached_lf_process_id`, `attached_pid`, `attached_os_started_at`; `agent_started_at`, `agent_pid`, `agent_os_started_at`, `agent_ended_at`, `agent_exit_code`; `endpoint`, `inbox`, `terminal_id` (`agent_process_id` stays as the generation id) | fences, Session state/available, reach, stop/reap, `end_earlier_owner`, interrupt check in `native_exit` |
| `graph_processes` (Graph run) | `started_at`, `ended_at`, `pid`, `os_started_at`, `exit_code`, `error`, `caller_session_id`, `repo`, `cwd`, `held_terminal_id` | Graph inventory, `lf task status`, failure cause, terminal retention |
| `graph_process_steps` | `started_at`, `ended_at`, `pid`, `os_started_at`, `exit_code`, `error` | step detail, `completed_graph_steps`, failure cause |
| `task_graphs` (carrier) | `run_pid`, `run_os_started_at` (cleared with `lf_process_id`) | Task moving, retirement, interrupt |
| `task_ship_events` approved/carried | `pid`, `os_started_at`, `ended_at`, `exit_code`, `error`, `caller_session_id` | Task shipping, "started by", carrier failure |
| `ci_incidents` | `repair_started_at`, `repair_pid`, `repair_os_started_at` | `pr_repair_lifetimes`, landing liveness |
| `waves` | `project_activation_started_at`, `_pid`, `_os_started_at`, `_ended_at`, `_exit_code`, `_error` | `readiness_in` |
| `held_terminals` | `holder_pid`, `holder_os_started_at` | holder dedupe and settle |

pid and OS start are cleared at exit. Agent states: reserved =
`agent_started_at` set, no pid, not ended; running = pid set; ended =
`agent_ended_at` set (no pid and no exit code = lf did not see the exit).
Run result, one function for every owner: `ended_at` NULL running; exit 0
succeeded; 130 with no error interrupted; other exit failed; ended with no
exit unknown. Wire DTOs keep `outcome` as that derived value.

## Chosen approach

1. **Spawners mint ids.** The Task-run loop, `RunningGraph::spawn`, `lf ship`
   and the CI repair launcher mint the child's id, write the owner row with
   pid and start at spawn, and pass the id as `LF_RUN_ID` (adopt this id;
   `LF_PROCESS_ID` stays "the lf you run under"). The child
   writes its own end, exit and error at exit; the spawner writes them if the
   child died first. Deletes `process_mark`, `child_process_after`,
   `graph_child_after`, `process_under_graph` (a step knows from
   `GRAPH_STEP_ENV`). `failure_cause` becomes: the last failed step's error,
   else the Graph run's (a run's error wraps its step's). `process_starter` becomes the run's
   `caller_session_id`, else the person on the ship event.
2. **The journal records no rows.** `.lf/journal/traces/<trace_id>/` and
   `lf.log` stay as the log of what ran; receipts and both locators go.
   Agent-issued commands are tool calls in the provider transcript.
3. **Attachment and agent on the Session.** `attach_in` and the
   `SessionTransfer` methods write `agent_sessions` only; every fence reads
   one row. The `require_current_actor_in` parent check becomes
   `attached_lf_process_id` = caller's launcher, valid because every
   production attach replaces the provider; the `prepare_resume` "beside"
   path stops keeping a second live agent (the old one is stopped, as
   replacement does) — check in slice 3 that no test depends on two.
4. **Liveness pass on the Machine server's clock** (the clock LOO-936 adds
   for prune). For every owner row with a pid: dead → close it (`ended_at`,
   exit NULL), clear pid; a Session whose attachment is released with a live
   provider is reaped as `reap_in` does today. Reads still check the OS
   inline, so a dead pid never shows as running between passes. Replaces
   `close_dead_processes`, `forget_processes`, receipt pruning and
   `lf process prune`. 238 unfinished rows today → a few dozen owner rows per
   pass, no full-table scan.
5. **What runs on this Machine.** `lf session list` and `lf graph list` list live Sessions and Graph runs; `lf ps` is deleted
   (plus Task carriers and ship carriers under their Task) from the owner
   rows, OS-confirmed, as a served read so remote clients get it through the
   server. `lf top`, `lf process list/top/prune` go; `lf process show <run>`
   becomes `lf graph show <run>`. `lf machine doctor` gains a stray scan: OS
   processes carrying `LF_STEER_SESSION`/`LF_PROCESS_ID` that no live row
   claims, children folded under their claimed parent; it reports, never
   kills.
6. **`lf history`.** Delete `lf history list`, the Process form of
   `lf history show`, the Processes part of `lf history export`, and
   `lf context --process`. No replacement log file: usage questions ("tokens
   and time by wave or by skill") are LOO-936's `usage.db`.
7. **Wake-ups.** `task_update_processes_*`, `store_revision_processes_*`,
   `store_revision_agent_process_update` go. Owner tables' existing triggers
   advance `graphs`/`planning`/`task_updates`/`sessions`; `agent_sessions`'
   update trigger also advances `task_updates` when the Session is a Graph
   step's (replaces the agent arm of `task_update_processes_update`). The
   `processes` revision domain keeps only `held_terminals`/`desktop_shells`;
   rename it `terminals`.

## Upgrade skew (released lf live after install)

Live on 2026-11-16: 162 unfinished `lf` rows (53 `session resume`, 69 in held
terminals, 82 launchers of a captured input, 4 Graph runs, 4 steps), 76
agents. After `lf install`, every new command runs the new binary (agents
resolve `lf` from PATH; steps via `resolve_lf_binary`); only processes already
running keep the old code, and they read mostly their own Session.

What breaks without a bridge: an old launcher's endpoint,
exit and re-token land only in `processes`, so new readers show its Session
stale. A new lf taking over an old-held Session repoints
`agent_sessions.agent_process_id` to an id with no `processes` row, so the old
holder's fence fails closed (correct).

**Decision: a SQL bridge in the expand migration, nothing in Rust.**
- old → new: triggers on `processes` insert/update copy agent rows into the
  current Session's columns (only while `agent_sessions.agent_process_id`
  matches and the token is the one being replaced) and `lf` rows that an owner
  points at into that owner's result columns.
- new → old, agent rows only (`process_agent_from_session_*`,
  `process_terminal_from_session`), which old holders' fences read. Graph
  runs and steps need none: a released runner stops at its next step's
  schema check (concept review after the lf-row readers).
- Loops: SQLite's `recursive_triggers` is off, and each trigger writes only
  when the value differs.
- The migration backfills every new column from `processes` for all rows
  (results for finished runs, pids for live ones), so no reader needs
  `processes` from the first new invocation.
- Justification and end: this is the one adapter, it is SQL, it writes only
  rows that old binaries read (no agent-issued or read-only commands), and the
  contract migration drops it with the table.

Rejected: install waiting for or stopping old work (install still cannot see
live processes reliably, and refusing a context is the 2026-11-04 failure);
the new lf writing the old rows from Rust (a second writer in every path, for
30 days).

## Delete — do not maintain

Deleted already: the handoff attach, ancestry repair, the journal writer and
receipts and locators, `FORGETTABLE`, `close_dead_processes`,
`forget_processes`, synthetic holder rows, tree walks (approach 1), the
`session_owner` reader.

Left for slice 4: `lf top`, `lf ps` call trees (`top.rs` `ActivityReader`),
`lf process` namespace (`list/top/prune/show`), `lf history list`, history
`--parent/--caller/--outcome` filters, `lf context --process`,
`ProcessCursor` export, doctor's `check_lineage`, the
`relocated_task_processes` writer and readers, Task Work `processes[]` and
`lf_process_ids`, `process_page` DTO and `LfProcess.swift`, the
`process_locators_*` triggers' readers. Left for LOO-965: the
`processes` arms in `canonical_ids` preflight and `query_plans`,
`binaries.rs`' released-lf arm, `SqliteStore::record_process`
(test-support) and the fixtures that seed `processes`. Each deletion takes
its exclusive tests and fixtures (`process_ownership_tests.rs` keeps only
what tests Session ownership).

Must survive: Session state/available, steer delivery and idle wake, stop and
reap, Graph retry and repeated-failure stop, ship "started by", CI repair
lifetimes, project activation readiness, terminal retention, `lf task
interrupt`, store-failure best-effort.

## Forbidden outcomes

A reader of `processes` in new code; a Rust writer of `processes`; a second
registry (receipts, an env-scan source); Task state reading the OS from SQL;
two live agents on one Session; a `Legacy*` type.

## Slices (one PR, after LOO-936)

1–3 and the journal stop are built (Status). Remaining:

4. **Delete `lf ps`, doctor strays, deletions** (CLI, DTOs, Swift, operate
   skills, docs; the list above). Test: `lf graph list --json` shows a
   SIGKILLed run ended (the liveness pass is built and tested at store level:
   `retention_lifecycle_tests`). Still untested: `graph_failure_cause`
   driving the repeated-failure stop.
5. **Bridge test.** Released-shape writer SQL (copied from the 0.16.0
   statements): an old Graph runner's end on `processes` → its Graph run
   ended; an old launcher's endpoint update → visible in `lf session list`.
   No step-discovery test.
6. **Contract Task** LOO-965 (filed 2026-11-18): after `STAGING_DAYS` (30),
   export `processes` to JSON lines (`-- exports:`), drop it, the bridge
   triggers, `relocated_task_processes`, `transport_lf_process_id`, unused
   locator indexes. The foreign keys already came off in the expand.

## Acceptance at gate (headless, private `LF_HOME`)

- `cargo nextest run -p loopflow-storage -p loopflow-execution --lib` and the
  slice tests above pass.
- `python3 -I scratch/process_sql_scan.py rust` → only the bridge, the
  LOO-965 arms and fixtures name `processes` once slice 4 lands.
- A fixture Task run end to end (`lf task run` with stand-in provider) leaves
  zero `processes` rows; `lf task status --json`, `lf session list --json` and
  `lf graph list --json` agree on what is live.
- `store::query_plans`: every repointed read plans `SEARCH`.
- Measure on a `.backup` copy of the real store: rows into `processes` per
  hour before/after the migration (3,067 + 1,144/day before; bridged only
  after) and `processes`-domain revisions per second.

## Exclusions

Turn rows and usage reports (LOO-936). Renaming `agent_sessions`/
`graph_processes` columns named `lf_process_id` (LOO-937 or the contract).
Dropping other dead columns (LOO-935). `lf server stop` keeps its own path.

## Direction from neighbouring Tasks (2026-11-17)

- **Names.** LOO-924 (#1966) adds `loopflow_execution::harness::Run`, a
  launch's lifetime. It keeps the name: "Leave it; LOO-953 picks other
  names." Built 2026-11-17: storage `Run` → `Execution`, `RunResult` →
  `ExecutionResult`, `RunStart` → `ExecutionStart`, `GraphRun` →
  `GraphExecution`, `LF_RUN_ID` → `LF_EXECUTION_ID`. `GraphStep`,
  `ops::task::RunStart` (where a Task run sets out, a different thing) and
  `Write::GraphRun` keep their names. Prose keeps "Graph run".
- **No launch-state column.** On showing bare vs managed in `lf session list`
  / `lf ps`: "Not needed". LOO-924 derives it from the Session row
  (`SqliteStore::is_bare`); nothing to carry over.
- **#1966 also changes:** `Holder`, `Place`, `transport::holder`,
  `WorkSource::is_managed` deleted; `Admitted` → `Recording`;
  `AgentConfig.agent` is typed `config::Agent { harness, model }`. Expect
  conflicts in `agent/mod.rs`, `transport.rs`, `harness/native_session.rs`,
  `process/runtime.rs` when it reaches this branch.
- **Agent columns are built here.** `agent_sessions.agent_pid`,
  `agent_os_started_at`, `agent_ended_at`, `agent_exit_code` land in the
  Session slice as in "Target records"; LOO-959, stacked on this branch,
  reads them for quit/died (an earlier steer that LOO-959 would add them was
  withdrawn).

## Decisions taken headless (reversible)

- `lf process show` folds into `lf graph show <run>` rather than staying.
- No replacement Process log beyond journal traces and `lf.log`.
- Ship result columns live on the starting ship event row, not a new table.
- `prepare_resume` stops leaving a second agent running; `lf resume` of a
  bare `lf <skill>` running in another terminal stops it there, as it does
  for any other client (built 2026-11-17).
- `attach_in` refuses to replace a spawned agent with no observed exit
  instead of leaving it running unlisted; only fixtures reached that path.
- A holder that recorded no OS process reads Unknown, so it cannot be taken
  over; fixtures say who holds an attachment with `test_set_holder`.
- (2026-11-18) The skew inventory (`binaries.rs`) moved onto owner rows plus
  released-lf Process rows rather than going blind; nothing else in the plan
  covered it.
- (2026-11-18) A Task carrier records no exit code of its own; nothing reads
  one but fixtures, which take the shipping's or the last Graph run's.
- (2026-11-18) `wt` timing samples dropped their receipt cost instead of
  timing the owner end write.
- (2026-11-18) Released receipt files under `runtime/exec-processes/` are left
  on disk; no lf reads them, and LOO-965 can delete the directory.

## Docs written for this change (describe intended behavior)

`docs/telemetry.md` (What's running now, What happened, Is Loopflow healthy,
Reference), `docs/troubleshooting.md` (What went wrong, Doctor, outcome_unknown,
Disk, Stray agent processes, Many lf processes), `docs/getting-started.md`,
`docs/surfaces.md`, `docs/sessions.md`, `docs/orchestration.md`,
`docs/agent-api.md`, `docs/index.md`, `docs/ops.md`, `docs/work.md`,
`docs/glossary.md`, `docs/security.md`, `docs/lf-reference.md` preamble;
builtin skills `repo_operate.md`, `task_operate.md`, `wave_operate.md`.
`docs/dev/*` still describe the Process table; slice 4 updates them with the
code. These docs predate the 2026-11-17 call to delete `lf ps`: 16 mentions
in 11 files (`docs/{work,ops,lf-reference,troubleshooting,agent-api,surfaces,getting-started,sessions}.md`
and the three operate skills; `grep -rn "lf ps" docs rust/loopflow-definitions/src/builtins`)
still describe a new `lf ps`, and the telemetry.md sample's columns (scope,
kind, harness or graph, title or step, pid, state, age) have no command to
belong to. Slice 4 moves them to `lf session list` / `lf graph list`.
