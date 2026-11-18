# LOO-953: no Process table; liveness on Sessions and Graph runs

## Status (2026-11-18, slices 4, 4b, 4c and 5 built; concept review after 4c: nothing to change; next: gate)

Realign 2026-11-18 (after 4c): `installation_tests.rs` reads no
`processes`; `journal::admit_process` warns `trace log unavailable`; no
`--processes`, `process.get` or `process.history` in rust, docs or swift.

LOO-936 (#1965), #1964 and LOO-935 (#1958) are merged and synced in. Every
slice of this PR is built: slice 2a, the owner and carrier slices, the Session
slices of slice 3, the journal stop, and slices 4, 4b and 5 below. History of each
pass, with its checks: `git show be7ccd8712:scratch/loo-953-remove-processes.md`.
Contract Task: LOO-965 ("Drop the Process table LOO-953 marked leaving").
Quotes and the OS-scan measurement: `questions.md`.

**Slice 4 as built (2026-11-18).**

- Deleted: `lf process list/top/prune/show` (`top.rs`, `process.rs`) with
  their `monitor`/`ps` aliases and saved-Graph translations, the
  `process.list` served read, `ActivitySnapshot` (Rust, Swift, fixture) and
  Desktop's unused `activity` subscription; `lf session history list`, the
  Process form of `history show`, Process paging (`ProcessFilter`,
  `ProcessCursor`, `ProcessPage`, `processes()`, `resolve_process`,
  `print_process`), `lf_process_ids` and Task Work `processes[]` (Rust,
  Swift, fixtures, `lf task status` Process lines), `LfProcess.swift`,
  `process_page.json`, the relocation receipts' writer and readers,
  `RecordKind::Process`/`GraphProcess*` and `CopyCounts.processes`, doctor's
  `attribution`/`identity`/`lineage` checks and `DoctorReport.rows`,
  `open_processes_read_only` (folded into `open_read_only`), the `--process`
  subject of context explanation, `unfinished_processes_except`,
  `recorded_process_evidence`.
- Attaching records `AttachingLf { lf_process_id, holder, terminal }`;
  `journal::current_holder` replaces `start_record`/`current_start_record`.
- Owner liveness lives in `process::os_process` beside `OsProcess`:
  `RecordedProcess::evidence` is the one rule, `step_evidence`,
  `carrier_evidence` and `session_agent_evidence` the three owner cases.
  The journal keeps the trace log only.
- `lf graph list` reads a run whose pid died as `stopped` (storage
  `graph_inventory` takes the caller's observation).
- `lf machine doctor` gains `strays` (`OsProcess::environment` returns only
  `LF_STEER_SESSION`/`LF_PROCESS_ID`; labels name the program, never argv).
- Orphan reaping stays on lazy recovery (`ops::recovery`), which
  `lf session list` now triggers; `reap_agent_processes` lost `--dry-run`.
- `history export` writes Sessions and events only; import skips Process
  and Graph rows a released exporter wrote.
- Docs and operate skills name `lf session list`, `lf graph list
  --runs --state current` and `lf graph show` in place of `lf ps`.

**Slice 4b as built (2026-11-18): user-facing names lose Process.**

- `lf graph list --runs` replaces `--processes` (no alias) through
  navigation, `lf task runs`, the `task.runs` read, docs, operate skills and
  fixtures; `lf graph show <id>` never took the flag, and fixtures stop
  passing it.
- Read kinds: `process.get` → `graph.run.get` (`Read::GraphRunGet`, Swift
  `.graphRunGet`); `process.history` → `session.history.get`
  (`Read::SessionHistoryGet`, Swift `.sessionHistoryGet`,
  `RegistryQuery.sessionHistoryText(id:)`). `session.history` was already
  `lf session history`, so the plan's name collided.
- Desktop's step "Read process history" button is deleted with its
  `historyQuery` plumbing: it asked Session history for a step's
  `lf_process_id`, a form slice 4 removed, so it always failed.
- Assertions counting `processes` rows (which no lf writes) are deleted:
  `install_preflight_records_no_process` (and its `test_task_installation.py`
  entry), the counts in `early_commands_exit…`, the release check,
  `inspection_starts_no_work`, and the `processes` half of `served_reads`'
  `changes()`; `store_revisions` still catches a read that writes.
- Follow-up Task at gate: `GraphProcess*` → `GraphRun*`,
  `work.graph_processes`, `graph_process.rs` (42 files, Rust and Swift).
  Concept review after 4c adds the journal's names for one lf run to the
  same Task, since `Execution` is that record's name: `ProcessContext`,
  `LfNode::Process`, `with_process`, `admit_process` ("admitted Process"),
  `process_attribution` ("Process attributed to no wave" in `lf.log`), and
  `ledger` for the store (`open_ledger`, `process_ledger`, "recording to
  ledger"). Internal and log-only, so not this PR.

**Slice 4c as built (2026-11-18).** Design: `git show 7b9a8e0638:scratch/loo-953-remove-processes.md`.

- `running_provider_uses_updated_install…` reads `graph_processes`
  (`graph='entry-proof'`, `caller_session_id` = the Session) and two
  `graph_process_steps` with `exit_code=0`; the prior install fails, so a
  step exiting 0 ran the replacement.
- `graph_live_across_switch…`: the `tools/` git shim appends
  `$PPID readlink /proc/$PPID/exe` inside a step; distinct pids other than the
  runner's, in order, must be [prior, replacement]. The runner keeping its
  lf is read from `/proc/<runner pid>/exe` after the switch, while step 1
  holds (the gate execs the install, so the pid is that lf). Assumes a
  step's lf spawns git directly, not through a shell.
- `journal::admit_process` warns `trace log unavailable: …` into
  `logs/lf.log`; stderr stays clean. Both `process_ownership_tests` string
  checks are deleted: help and parse errors exit before admission, so they
  could not fail; `stderr.is_empty()` on success stays.
- Checks: `binary(process_ownership_tests)` 8 pass; execution lib tests
  build; `clippy -p loopflow-execution --all-targets` clean. The two
  installation proofs need Docker (not running here): gate/CI's
  `task-installation` job.

**Compress after 4c (2026-11-18).** `reap_in` reports counts
(`settled`, `reaped` pids, `errors`) instead of per-agent action strings only
tests read; `ops::recovery` logs each reap error to `lf.log` (a failed
settlement was dropped before). `reap_detached` ends the duplicate on
`agent_sessions`, not through the `processes` bridge. Check:
`harness::agent_process::` 15 pass; clippy clean.

**Slice 5 as built.** `sqlite::processes::bridge_tests` drives 0.16.0's
`record_process_in`, `record_graph` and `record_agent_endpoint` SQL: a
released Graph runner's end ends its Graph run; a released launcher's
endpoint reaches `agent_endpoint` on its Session.

**Slice 6** is LOO-965. What it removes beyond the marked tables: the
bridge triggers, `binaries.rs`' released-lf arm (`unfinished_processes`,
`read_process`, `PROCESS_SELECT`), `canonical_ids`/`query_plans` arms,
`forget_unread_rows`' `attachment_exit_seq` clear, and the test-support
`record_process`, `lf_process`, `agent_process`, `processes_since` with the
`LfProcess`/`AgentProcess`/`ProcessEntry` types they return.

`python3 -I scratch/process_sql_scan.py rust` (2026-11-18): production
statements naming `processes` are the bridge's, `canonical_ids` preflight,
`query_plans`, `forget_unread_rows` and `PROCESS_SELECT` (binaries' arm).

Checks (2026-11-18): storage `graph_inventory|processes|task_work|
repositories|machine_records|canonical_ids|query_plans|migration_runner` →
163 pass; execution `process|harness::agent_process|store::retention|
machine_records|rental::recovery|store::task_work|read_stream|ops::context|
ops::run|journal|observe|store::session_projection|attachment_lifecycle|
ops::task|ops::human_session|ops::pr_landing` → 314 pass after one fixture
fix; `graph::` → 87 pass; loopflow `server::{service,subscriptions,schema}|
doctor|lf::tests|dto_fixtures|cli_discovery|global_commands|
task_abandonment|graph_discovery_tests|binaries|history|read` → 204 pass
after fixture fixes; `binary(served_reads)|binary(process_ownership_tests)|
subscribe::reads_record_no_process` → 22 pass; `documented_commands::` 4,
`binary(golden_prompt)` 1 (no golden change), bridge tests 2;
`scripts/test_desktop.sh --filter 'DTOFixtureTests|WorkObservationTransportTests|
RegistryQueryTests|DesktopHeadlessTests|DesktopLogTests|WorkSelectionTests|
WorkNavigationTests'` → 156 pass; clippy and fmt clean; server schema
fixture regenerated; `session_cli_tests::` 24 and the `local_planning`
public-run fixture pass after dropping their Process-row reads. Gate/CI:
`binary(session_lifecycle_tests)`, `work_open_tests`, `binary(two_people)`,
`binary(transport_tests)`, `test(transport::)`, `test(performance_tests::)`,
`binary(lf)`, the rest of `local_planning`, and the
journal stop's flaky-under-load
`completion_preserves_retained_session_input_and_unknown_process_history`.
`check_migrations.py` and `schema_page.py --check` were clean before slice 4,
which changed no migration.

Checks for 4b (2026-11-18): `server::schema|lf::tests|read::tests` 51,
`documented_commands|graph_discovery_tests|dto_fixtures|world_recipes|
cli_discovery` 104, `binary(served_reads)|binary(process_ownership_tests)` 21,
Desktop `RegistryQueryTests|DesktopHeadlessTests|DTOFixtureTests|
ServerProtocolTests` 114 pass; schema fixture regenerated; clippy, fmt clean.
`graph_tests::|task_graph_launch_tests::` (82) failed 3 then 1 under
parallel load, a different test each run (`a_held_command_stops…`,
`interactive_graph_keeps_input…`, `running_graphs_belong…`,
`task_create_run_starts_the_graph…`: "Task run ended: Graph … running");
each passes alone. None reads the renamed flag; flaky, for gate.

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
- User-facing names lose Process in slice 4b; the internal `GraphProcess*`
  type rename is a follow-up Task.

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

## Readers of `processes` left

`python3 -I scratch/process_sql_scan.py rust` (writes `/tmp/loo953/scan.json`;
create the directory first). Current result: Status, after slice 5.

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

Slice 4's list is deleted (Status). Left for LOO-965: the
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

All built (Status); slice 6 is LOO-965, filed 2026-11-18, after
`STAGING_DAYS` (30): export `processes` to JSON lines (`-- exports:`), drop
it, the bridge triggers, `relocated_task_processes`,
`transport_lf_process_id`, unused locator indexes.

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
- (2026-11-18, slice 4) Orphan reaping stays on lazy recovery rather than
  moving to the server's liveness pass; `lf session list` triggers it where
  `lf process` did.
- (slice 4) Doctor `strays` claims only owner rows, so a released lf still
  running in the skew window reads as a stray (a warning). macOS hides the
  environment of platform binaries (`/bin/sleep`, `/bin/zsh`), so the scan
  cannot see those; providers and node/python children were readable.
- (slice 4) `ContextExplanation.process` stays, bound only to the running
  lf (`inspecting_process`); `process.get` and `process.history` keep their
  wire kinds until the Process-line rename.
- (slice 4) A copy imported from a released exporter skips its Process and
  Graph rows instead of failing the give-back.

## Docs written for this change (describe intended behavior)

`docs/telemetry.md` (What's running now, What happened, Is Loopflow healthy,
Reference), `docs/troubleshooting.md` (What went wrong, Doctor, outcome_unknown,
Disk, Stray agent processes, Many lf processes), `docs/getting-started.md`,
`docs/surfaces.md`, `docs/sessions.md`, `docs/orchestration.md`,
`docs/agent-api.md`, `docs/index.md`, `docs/ops.md`, `docs/work.md`,
`docs/glossary.md`, `docs/security.md`, `docs/lf-reference.md` preamble;
builtin skills `repo_operate.md`, `task_operate.md`, `wave_operate.md`.
Slice 4 moved every `lf ps`/`lf process` command reference to `lf session
list`, `lf graph list --processes` and `lf graph show`, and rewrote the
LfProcess and AgentProcess rows of `docs/dev/reference.md`. Realign after
slice 4 rewrote `docs/dev/reference-models.md`' vocabulary, `processes` owner
row and "lf invocations" / Graph sections for owner rows;
`docs/dev/reference-processes.md` uses LfProcess only as the attached lf's
identity, which still holds.
