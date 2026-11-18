# LOO-953: no Process table; liveness on Sessions and Graph runs

## Status (2026-11-17, Session lifetime built)

LOO-936 merged as #1965 (`fb14b674c9`) and #1964 with it; synced at
`ac93e4c0e7`, resolved against the LOO-936 branch history (main's tree
equals its tip). Still owed: rerun `python3 -I scratch/process_sql_scan.py rust`
and reconcile the inventory.

Built: slice 2a ("Slice 2a as built", QA in `scratch/qa.html`); the
renames to `Execution`, `ExecutionResult`, `ExecutionStart`,
`GraphExecution`, `LF_EXECUTION_ID` ("Direction from neighbouring Tasks");
CI repair runs on `ci_incidents.repair_{started_at,pid,os_started_at}`
(reserve and handoff write them, settlement clears them, liveness by
`run_evidence`); `detached::start_lf` returns `Started { id, pid,
os_started_at }`, so `engine::launch` waits on its child without the store;
the owner items, carriers and the first Session cut below.

Next, in order (why this order: "Concept review after the Session
lifetime cut"): (1) the attachment cut: `attachment_token`,
`attached_lf_process_id`, `attached_pid`, `attached_os_started_at`,
`endpoint`, `inbox`, `terminal_id` onto `agent_sessions`, the agent writers
moved onto the Session, readers of the attached lf's row repointed, a
reserved agent taking its holder's liveness, the `prepare_resume` beside
path ended; (2) the lf-row readers: `task_open_process_pairs` and
`task_state_sql`'s `open_processes` arm as live Sessions + Graph runs,
retirement by terminal (Graph runs gain `held_terminal_id`),
`process_starter`, `run_task_graph`'s parent check (`graph.rs:79`);
(3) the expand migration's foreign-key rebuild together with stopping the
journal writer **and the owner liveness pass** (see "Concept review after
the carrier slices"); (4) slices 4–6.
Quotes and the OS-scan measurement: `questions.md`.

**Session lifetime as built (first cut of slice 3).**

- `agent_sessions.agent_{started_at,pid,os_started_at,ended_at,exit_code}`:
  the current agent's lifetime, pid cleared at its end. Backfilled from the
  agent row; three triggers (`session_agent_from_process_{insert,update}`,
  `session_agent_from_attachment`) copy the agent row's lifetime onto the
  Session, for this lf and a released one. lf's agent writers still write
  the agent row, since it carries the attachment; they move with it.
- Readers on the Session's columns: the projection's identities
  (`session_projection_inputs_in`, so `session list`/summary liveness and
  `available`), `TURN_LIVE`, `available_sql`, `session_terminal`,
  provider transitions' `live_process`, `saved_session_connection`'s
  liveness, `agent_process_identity`. Still joining the agent row for
  attachment or reach: `TURN_LIVE`/`available_sql` token,
  `require_current_actor_in`, `idle_conversation_candidates`, summary
  terminal and preview.
- A Session's Graph is `of_run` over its Graph run's own columns:
  `SessionProjectionInputs.graph_runs` (unended runs its Sessions' captures
  name) are observed by `observed_result` in `Reading::acquire`;
  `of_process` is deleted.

Checks (2026-11-17, Session lifetime): storage `migration_runner::`,
`query_plans::`, `sqlite::{sessions,processes,provider_transitions,steers,revisions}::`
→ 185 pass; `run_owners_carry_their_process_lifecycle_after_upgrade` (Session
backfill and bridge) → pass; execution `store::`, `process::`,
`harness::agent_process::`, `ops::human_session::`, `capture_tests`,
`registry_tests`, `fact_writer` → 416/417, the failure
`process::runtime::state_tests::history_times_provider_work_from_its_first_recorded_attempt`
also fails at `38aa184ae5` (gate); new
`a_session_reads_its_agent_lifetime_from_its_own_row` and the extended
`session_metadata_reads_its_graph_from_the_process_that_captured_its_input`
(dead Graph pid → stopped) → pass; `loopflow` `server::wake::`,
`server::boundary::` → pass; clippy → clean.

**Concept review after the Session lifetime cut (2026-11-17): next implement direction.**

A Process row is two things: an lf row (written by the journal) and an agent
row (written by the attachment writers). They leave in two cuts, and the
order is forced, not chosen:

- **The attachment cut comes first, because lf rows are read through the
  agent's attachment.** The summary's terminal is the attached lf's
  `processes.terminal_id` while its row is unfinished (`sessions.rs:407`);
  `UNRECORDED_ANCESTRY` (`processes.rs:534`) and `attachment_in` treat a
  missing lf row as released; `session_turn_origin` (`session_events.rs:559`),
  `process_session` (`sessions.rs:841`) and provider transitions
  (`provider_transitions.rs:182`) match on `attached_lf_process_id`. If the
  journal stopped first, every attached agent would read released and lose
  its terminal. So the cut puts `attachment_token`, `attached_lf_process_id`,
  `attached_pid`, `attached_os_started_at`, `endpoint`, `inbox`,
  `terminal_id` on `agent_sessions`, and those readers read the holder's
  liveness from `attached_pid` through `journal::pid_evidence`, never from an
  lf row. `require_current_actor_in`'s parent check becomes
  `attached_lf_process_id` = the caller's launcher (approach 3).
- **A reserved agent takes its holder's liveness.** Reserved (started, no
  `agent_pid`) reads Unknown today; when its holder died before spawning,
  nothing ever ends it. As `step_evidence` does for a reserved step, a
  reserved agent's evidence is `pid_evidence(attached_pid, ..)`, and the
  liveness pass ends it (`agent_ended_at`, no exit) when that reads Dead.
  Test with the attachment cut: reserve an agent under a SIGKILLed holder →
  `lf session list` does not show it live, and the next pass ends it.
- **The bridge flips per column set.** After the cut lf writes no agent row;
  `session_agent_from_process_*` then fire only for a released writer
  (unchanged), and Session-update triggers write the agent row a released
  reader joins. Each trigger's `WHEN` compares every column it writes, so
  the pair cannot ping-pong.
- **Then the lf-row readers, then the journal stop.** In one change before
  the journal stop: `task_open_process_pairs` and `task_state_sql`'s
  `open_processes` arm become the Task's live Sessions (`agent_started_at`
  set, `agent_ended_at` NULL) and unended Graph runs; retirement by
  terminal (Graph runs gain `held_terminal_id` from `transport::current()`,
  which the journal passes to `record_process_within` today; `unsettled_work`'s
  ancestry walk, `held_terminal_processes` and `launcher_lf_process_id`
  reads go); `process_starter` from the ship event's `caller_session_id`;
  `run_task_graph`'s parent check compares the carrier id with the parent
  id the journal already inherits from the env, not a store row. The
  journal-stop change (foreign-key rebuild, liveness pass, new → old lf-row
  bridge) follows with the tests already named. A carrier waiting between
  retries has no live Graph run; it is still "moving" by `run_pid`, so the
  Task cannot retire then, as today.

No open product choice.

**Carrier slices as built.**

- `journal::observed_result(&Execution)`: `Running` whose `run_evidence`
  reads Dead → `Unknown`. `SqliteStore::project_readiness(wave, observe)`
  selects the activation's `Execution` once and builds the wire
  `ProjectActivation` from `observe(&run)`; `ops::project::readiness` passes
  `observed_result`, storage-only callers `Execution::result`.
  `project_activation_run` and the outcome patch are gone. Task updates read
  a Graph run's outcome through it too.
- Task carrier: `task_graphs.run_pid, run_os_started_at`, written by
  `start_run_in` from the carrier's `ExecutionStart`, cleared by arrival,
  `set_step_in` and `finish_run(TaskCarrier)`. **Deviation from the concept
  review:** the end keeps `lf_process_id`. `row.run` (cursor + carrier) is
  how the next `lf task run` sets out from where a stopped run stopped, and
  how `lf task status` shows it stopped; clearing the id would lose both.
  `TaskCarrier { id, pid, os_started_at }` replaces the Process row on
  `SavedTaskGraphRow`; `journal::carrier_evidence` (no pid → Dead) answers
  "running" for `lf task status` and the double-run refusal.
  `process_may_run` is deleted. `task_state_sql` and `task_close_candidates`
  read "moving" as `run_pid IS NOT NULL` — SQL cannot ask the OS; the
  liveness pass (approach 4) closes a SIGKILLed carrier, as
  `close_dead_processes` does for its Process row today.
- Ship carrier: `task_ship_events.{pid,os_started_at,ended_at,exit_code,error}`
  (`at` is its start); `ShipLog`/`TaskShipping.carrier: Option<Execution>`.
  `task_ship_events_launch_is_immutable` replaces the append-only trigger,
  `store_revision_task_ship_events_update` added, the revisions exemption
  dropped. `finish_run(ShipCarrier)` ends approved/carried rows by
  `task_ship_events_process`. Restart, "already shipping", stop-kill and the
  ship snapshot read `run_evidence`/`observed_result`; the failure cause is
  the Task's latest Graph run's `graph_failure_cause`, else the carrier's
  own error, so `failure_cause`'s Process walk is deleted. Arrival at `end` hands off
  with the carrier's pid read from `task_graphs` before it clears.
- The journal holds `run_owners: Vec<RunOwner>`: `lf task run` marks
  `TaskCarrier` and `ShipCarrier` (its run may hand the Task off at `end`),
  `lf ship` marks `ShipCarrier`; a step may also be a carrier.
- Bridge: released carriers get `run_pid` from their Process row on
  `lf_process_id` change, released ship events on insert; a Process end
  clears `run_pid` and ends ship rows. Backfill in the same migration.

Checks (2026-11-17, carriers): storage nextest
migration_runner/graph_inventory/revisions/project_selection/task_work/
query_plans/processes/durable/task_retirement/forget → 196 pass; execution
readiness_tests/task_updates/wave_lifecycle/journal/store::task_work*/
ops::task::ship/task_execution/pr_landing/store::tests → pass except
`journal::tests::a_held_write_lock_delays_an_process_by_one_receipt_wait`
(pre-existing, gate); `nextest -p loopflow --lib` graph_tests,
task_graph_launch_tests, world_recipes, explain `--test-threads 8` → pass
after the ship cause fix (`task_ship_restarts_after_a_failure_wait…` needs the
Graph run's cause), except
`task_ship_runs_every_remaining_step_standing_in_at_each_interactive_step`
failing once at line 2761 after 203 s under load 60 and passing alone in 16 s
(gate: treat as a flake to fix if it recurs); clippy → clean.

**Concept review after the carrier slices (2026-11-17): next implement direction.**

- **The liveness pass moves into the journal-stop change.** Every owner
  row that SQL reads as live (`task_graphs.run_pid`, ship events'
  `pid`, Graph runs and steps, activations, CI repairs) is closed after a
  SIGKILL only because `retention::prune` → `close_dead_processes` ends its
  Process row and a bridge trigger copies that end. `task_state_sql` and
  `task_close_candidates` read `run_pid IS NOT NULL`, so once the journal
  stops writing rows, a killed carrier leaves its Task "moving" and
  unretirable until someone runs it again. So the change that stops the
  journal writer also turns `prune`'s `dead` input from Process entries into
  owner rows with a pid whose `process_identity_evidence` is Dead, and
  closes each through `finish_run(owner, id, now, None, None)` (result
  `unknown`), clearing pid. `close_dead_processes` and `forget_processes`
  go in that change. Slice 4 keeps `lf ps`/doctor/deletions; its test
  (SIGKILL a Graph runner → next pass ends it `unknown`) moves with the
  pass, plus: SIGKILL a Task carrier → next pass, `lf task status` reads
  idle and the Task can retire.
- **One liveness function over a pid** (built): `journal::pid_evidence(pid,
  os_started_at)` (no OS start → Unknown) under `run_evidence` (no pid →
  Unknown) and `carrier_evidence` (no pid → Dead); ship's private one is
  `ship_carrier_evidence`.
- **Kept as built:** the carrier end keeps `lf_process_id` (the cursor's
  carrier is how the next run sets out); one `lf task run` writes its pid on
  both `task_graphs` and, after handoff, its ship event, and its end clears
  both. These are two records the same lf really owns (a run and a shipping),
  not a duplicate.

No open product choice.

**Owner slices as built.**

- A Project activation is a run on its Wave:
  `waves.project_activation_{started_at,pid,os_started_at,ended_at,exit_code,error}`.
  `record_project_activation(wave, id, ExecutionStart)` writes the start and
  clears the last end; `ops::project::readiness` (served by `lf wave status`
  and the roadmap) reads an unended activation whose pid is dead as outcome
  `unknown`, and Desktop shows that outcome before "no recorded outcome yet".
  The migration backfills from `processes`; `project_activation_start_from_process`
  bridges a released lf that records the id alone.
- `RunOwner { GraphRun, GraphStep, ProjectActivation }`
  (`graph_process.rs`); `finish_run(owner, id, …)` writes that one table. The
  journal holds `run_owner`: a minted id under `LF_GRAPH_STEP` starts as
  `GraphStep`; a Task run's Graph child marks `GraphRun` once it records
  itself; activation marks `ProjectActivation`.
- The three spawner end writes in `lf/commands/graph.rs` go through
  `record_spawner_end`: warn and continue. Test
  `graph_tests::a_step_that_dies_before_writing_its_end_ends_as_its_graph_saw_it`
  (a SIGKILLed step reads ended, outcome `unknown`). Still untested:
  `graph_failure_cause` driving the repeated-failure stop.

**Correction: foreign keys come off in the expand, not the contract.** Once
the journal stops writing rows (approach 2), every column still declaring
`REFERENCES processes` fails a new lf's write (seen in this pass's tests:
`waves`, `ci_incidents`). So the expand migration rebuilds `waves`,
`ci_incidents`, `task_graphs`, `agent_sessions`, `desktop_shells`,
`session_events`, `steer_adapters`, `steer_batches`, `tasks` without those
keys (a rebuild keeping every column is additive), in the same change that
stops the journal writer. Until then the two tests above insert the
journal's row for the referenced id. Slice 6 keeps only the drops of
`processes`, the bridge triggers and dead columns.

### Slice 2a as built

- `process_rows_on_owners.sql` rebuilds `graph_processes`,
  `graph_process_steps` and `agent_limit_recoveries` without `processes`
  foreign keys or the step-parent trigger; adds `started_at, ended_at, pid,
  os_started_at, exit_code, error` (+ `repo, cwd` on the run); backfills from
  `processes`; keeps launch columns immutable; bridge triggers copy a released
  lf's start/end from `processes` onto owner rows.
- `GraphRun` / `RunResult` / `RunStart` (`loopflow-storage/src/graph_process.rs`)
  replace `LfProcess` inside `GraphProcess` and its steps.
- Spawners mint ids: the Task-run loop and `RunningGraph::spawn` pass
  `LF_RUN_ID`; the journal adopts it as the child's own id. The step row is
  written at spawn with pid; the child writes its end in `ledger_insert`; the
  spawner writes `status.code()` if the child left none. Deleted:
  `process_mark`, `child_process_after`, `graph_child_after`. A spawner's
  reservation is `RunStart::reserved(at)`; the Task-run loop's
  `registration` answers a bool, since the loop already holds the id.
- `graph_failure_cause` (last failed step's error, else the run's) replaces
  the Process walk for Task-run retry; `journal::run_evidence` replaces
  `lf_evidence`/receipts for Graph runs and steps (Task execution, interrupt,
  landing, limit recovery). A plain `CommandExit(130)` records no error.
- Kept for now: `process_under_graph` (`run.rs` capacity wait; Sessions
  served by a transport owner lack the step env), `failure_cause` and
  `process_starter` (ship carriers), receipts for open execution in a Task's
  work (`task_open_process_pairs`).

Checks (2026-11-16, load average 60–95 from other checkouts):
`nextest -p loopflow-storage --lib` migration_runner/query_plans/graph_inventory/
revisions/task_work → pass (one `canonical_ids` timeout at 300 s under load in
an earlier run; CI); execution task_updates/journal/read_stream/human_session/
task_execution → pass; `nextest -p loopflow --lib` graph_tests,
task_graph_launch_tests, explain, world_recipes `--test-threads 8` → pass
(default parallelism timed out at load 95); `cargo clippy --all-targets` →
clean. Gate: `binary(two_people)`, `test(transport::)`, `test(performance_tests::)`
for the table rebuild.

### Concept review after slice 2a: next implement direction

Settled by the design (no product choice involved):

- **One record type for "one lf run"** (built, then renamed `Execution`):
  ship events, CI repairs and activations carry the same columns; one
  writer per owner table as owners are added.
- **Two env variables, two directions.** `LF_RUN_ID` (adopt this id, read
  once) and `LF_PROCESS_ID` (the lf you were launched under, read by
  agent-issued commands for the attachment fence) answer different
  questions; a child could not tell them apart if merged. Keep both; the
  2a question is closed.
- **The child's end write stops depending on the journal row.**
  `finish_owned_run` runs only after `record_process_within` lands. When the
  journal stops recording (approach 2), the owner end write is the child's
  only write at exit, best-effort with the journal's warn-once gap.
- **The bridge needs its new → old half before the journal stops.** The
  migration today copies old → new only; new children are still visible to
  an old runner because the journal still writes their `processes` row.
  The same change that removes the journal writer adds the owner-insert
  triggers that write the `processes` rows an old `child_process_after` /
  `graph_child_after` reads.
- **The Task-run loop still waits for its child to write the Graph run.**
  The spawner mints the id but cannot write the row (the child compiles the
  Graph). Keep it; `registration` only serves the let-go log, and the
  retry decision reads `graph_run_recorded`. Not a precondition worth
  removing yet.
- **Dev-only skew** (new runner, older installed child) leaves a step with
  the spawner's exit code and no error. Released installs never run an
  older child than their runner; accept.
- **`lf ps` is deleted**, with `lf top` (2026-11-17: "i thikn we delete lf
  ps?"). What runs on this Machine is `lf session list` (live state per
  Session) and `lf graph list` (Graphs that ran, live ones marked), both
  OS-confirmed from owner rows. Docs and operate skills that name `lf ps`
  move to those. `lf graph show <RUN_ID>` stays as built: today it takes an
  authored Graph's name, a run id is the only other thing it could take
  ("what else would it accept"); UUID and slug cannot collide.

Wire names still saying Process (`GraphStepProcess`,
`work.graph_processes`, module `graph_process.rs`) rename with the
Process-line slice, in one DTO change across Rust, Swift and fixtures, not
piecemeal. `RecordKind::GraphProcess*` in `machine_records.rs` names tables
and goes with the export in slice 4.

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

## What the code does now (facts that constrain this change)

Paths under `rust/`.

- **Writer.** Every `changes_work()` command records a row:
  `journal/mod.rs` `admit_process` → `ledger_insert` →
  `sqlite.rs` `record_process_in` (upsert). Read-only commands, `lf server`
  and `lf holder` are not recorded (`mark_read`, `lf/mod.rs:1141`). Store
  failures are already best-effort (warn once).
- **Receipts** `$LF_HOME/runtime/exec-processes/<id>.json` (pid, start) are a
  second registry read for liveness (`recorded_process_receipt`,
  `lf_process_evidence`). `journal_trace_locator` only picks the
  `.lf/journal/traces/<trace>/` directory for an inherited trace.
- **Liveness** is `process_identity_evidence` (`journal/mod.rs:1219`): pid +
  OS start within 3 s (Linux derives start from `etime`). Keep it; change what
  it is called on.
- **Graph steps and ship runs are child `lf` processes.** `RunningGraph::spawn`
  (`lf/commands/graph.rs:832`) takes `process_mark`, then polls
  `child_process_after` every 10 ms to learn the child's id; the Task-run loop
  (`graph.rs:195-260`) does the same with `graph_child_after`, then
  `failure_cause` walks children for the deepest error. `graph_process_steps`
  stores only node and iterations; start/end/exit live on the child's row.
- **Task runs.** `task_graphs.lf_process_id` names the carrier (`lf task run`,
  the retry loop), not the Graph. Task moving/shipping is
  `task_state_sql` (`durable.rs:407`): carrier or ship carrier
  `completed_at IS NULL`. Ship carriers are `task_ship_events.lf_process_id`
  (approved/carried). `tasks.ship_lf_process_id` is still in the schema with
  a `processes` foreign key but has no production writer or reader
  (`task_machine_local.sql` copied it into ship events); only `FORGETTABLE`
  names it, since #1961.
- **Attachment.** `attach_in` (`processes.rs:608`) re-tokens on every claim;
  every production attach passes `replace_provider=true`, so a new lf always
  gets a new agent id. Exception: `prepare_resume` (`process/attachment.rs:165`)
  leaves an old agent running "beside" for bare terminal launches. Fences:
  `require_current_actor_in` (current agent, matching parent, attached not
  NULL), token match in `record_session_activity`, `session_liveness`,
  `session_turn_origin`, `TURN_LIVE`, `available_sql`. `steer_adapters` and
  `steer_batches` are keyed by `agent_process_id` ("survives handoff, never
  replacement"); `steer_batches.attachment_token` has no reader.
- **Reach.** Codex `endpoint` (`record_agent_endpoint`, read by
  `saved_session_connection`, `agent_endpoint`, close-validate
  `agent_process.rs:429`); Claude `inbox` (`record_agent_inbox`, read only by
  `idle_conversation_candidates` → `server/wake.rs`).
- **Stopping.** `stop_session_agent`/`reap_in` signal the agent's process
  group from the agent row's pid; `end_earlier_owner` (`human_session.rs:1621`)
  kills an earlier `session serve` owner by receipt or row pid. Holders get a
  synthetic `["lf","holder","run"]` row (`terminal_holder/client.rs`), read to
  dedupe and settle, never to signal. The Machine server has no row.
- **Bare `lf -b <skill>`** (`run.rs:37`, `Holder::Caller`) writes
  `agent_sessions` and the agent row straight to SQLite; no server needed
  (confirmed; brief item). So "a Session records its pid" covers it.
- `agent_sessions.transport_lf_process_id`: no production writer; only
  `session_owner` (legacy `lf resume` owners) reads it.
- **Outcome** today: `finish_runtime` writes `failed` with `exit_code=130`
  and an error for `CommandExit(130)`, so "130 with no error interrupted"
  needs that writer to stop attaching an error to a plain interrupt.
  `native_exit.rs:146` and `terminal_holder/client.rs:351` also deviate.
- **Steps spawn the installed lf** (`resolve_lf_binary`, `os_process.rs:243`),
  not the parent's build: after `lf install`, an old Graph runner's next step
  runs the new binary. This drives the bridge below.

## Inventory: 98 production statements naming `processes` (base `4a0691a9a0`)

Method: `scratch/process_sql_scan.py` (string literals outside tests,
`#[cfg(test)]` stripped). 98 statements, 47 joined with another table, 24
writes (the brief's 101 used a slightly different cut). Fates:
**S** → Session columns, **G** → Graph run/step columns, **O** → another
owner's columns, **D** → delete with its feature, **F** → fixture.

| File (`loopflow-storage/src/`) | Function:line | Fate |
| --- | --- | --- |
| `sqlite.rs` | `open_existing_processes` 687, `validate_process_schema` 882, `record_hosted_process` 1476, `process_is_recorded` 1514, `record_process_in` 1528 | D (journal writer) |
| `canonical_ids.rs` | `preflight` 52, 87 | D (drop the `processes` arms) |
| `query_plans.rs` | 47 | D arm |
| `sqlite/ci_incidents.rs` | `pr_repair_lifetimes` 244 | O: `ci_incidents.repair_started_at`, repair pid |
| `sqlite/session_events.rs` | `record_session_activity` 395, `session_liveness` 435, `session_turn_origin` 578 | S |
| `sqlite/steers.rs` | `with_provider_context` 69, 94; `record_native_turn` 174; `record_agent_inbox` 211; `idle_conversation_candidates` 233; `claim_session_steers` 692 | S |
| `sqlite/task_work.rs` | `lf_process_ids` 119, `task_of_process` 142 | D (Task membership by Process cwd; `processes[]` DTO goes) |
| | `completed_graph_steps` 185, `latest_task_graphs` 213 | G |
| | `task_open_process_pairs` 647 | S + G (live work = Sessions + Graph runs) |
| `sqlite/task_retirement.rs` | `task_close_candidates` 65 | O: `task_graphs` carrier |
| `sqlite/sessions.rs` | `session_projection_inputs_in` 69, `TURN_LIVE` 168, `available_sql` 264, `summary_query` 368, `session_terminal` 773, `pending_inputs` 1106, `require_current_actor_in` 1899 | S |
| | `session_owner` 791 (transport legacy), `process_session` 816 | D |
| | `terminal_retention` 840 | G + S (Graph run's own `held_terminal_id`) |
| `sqlite/durable.rs` | `task_state_sql` 407 | O: `task_graphs` carrier, ship event carrier |
| `sqlite/provider_transitions.rs` | `live_process` 95, `observe_in` 173 | S |
| `sqlite/project_selection.rs` | `readiness_in` 347 | O: `waves.project_activation_*` |
| `sqlite/retention.rs` | `process_receipt_select` 267 (×2), `close_dead_processes` 300, `forget_processes` 325 | D → liveness pass over owners |
| `sqlite/graph_inventory.rs` | `graphs_in` 34, 38, 58; `entry_in` 110; `graph_execution_facts` 484, 489, 535 | G (535's receipt arm D) |
| | `process_under_graph` 225, `process_mark` 412, `child_process_after` 427, `graph_child_after` 445 | D (spawner mints the id) |
| | `test_graph` 574 | F |
| `sqlite/machine_records.rs` | 16 (rental export of Processes) | D |
| `sqlite/repositories.rs` | `relocate_repository` 138 (`relocated_task_processes`) | D |
| `sqlite/processes.rs` | `replace_agent` 134; `finish` 205, 217; `commit` 252, 282, 294; `attachment_in` 529; `agent_process_progress_in` 573; `attach_in` 637, 647, 664; `saved_session_connection` 1032; `agent_process_identity` 1082; `record_agent_process_launch` 1107, `_exit` 1136, `_identity` 1158; `agent_endpoint` 1194; `record_agent_endpoint` 1213; `record_session_connection` 1231; `begin_agent_settlement` 1448; `record_session_terminal` 1480; `release_session_attachment` 1616; `agent_parent` 1670 | S |
| | `agent_process_query` 300, 300, 308 (incl. `FORGETTABLE`); `process_query` 406, 478, 507; `unfinished_processes_except` 703; `process_receipt_locator` 782; `journal_trace_locator` 814; `resolve_process` 966; `unrecorded_agent_ancestry` 1366; `release_agent_ancestry` 1402 | D |
| | `held_terminal_processes` 841 | O: `held_terminals` holder pid |
| | `process_starter` 874; `failure_cause` 920, 932 | O/G: starter and error recorded on the run |
| | `test_lf_process` 1070 | F |
| `loopflow/src/server/subscription_measurement.rs` | `attach` 42, 56 | F (repoint) |

Schema: 15 foreign keys reference `processes` (`agent_sessions` ×2,
`ci_incidents`, `desktop_shells` ×2, `graph_process_steps`,
`graph_processes`, `session_events`, `steer_adapters`, `steer_batches`,
`task_graphs`, `waves`, `agent_limit_recoveries` and
`relocated_task_processes` with CASCADE, `tasks.ship_lf_process_id`; read
from the released store 2026-11-16). #1961 added `tasks.ship_lf_process_id`
and `desktop_shells.launcher_lf_process_id` to `FORGETTABLE` after dangling
references failed every migration's foreign-key check on one Machine;
`forget_tests` reads the schema, so a new reference must join the rule while
`forget_processes` lives. 11 triggers on it (revisions, locators, `task_update_*`,
canonical ids). DTO fields that carry Process data: `processes[]` in
`task_work`, `task_lifecycle`, `task_checkout`, `process_page`
(`swift/Loopflow/Models/LfProcess.swift`); `GraphStepProcess` and
`GraphProcessSummary` fields are read from Process columns today.

**Gap found in review (2026-11-17): Task retirement's ancestry walk.**
`unsettled_work` (`loopflow-execution/src/ops/task/retirement.rs:308`) lets a
Task retire while open work runs only if that work descends, by
`parent_lf_process_id`, from a Desktop shell's launcher
(`desktop_shells.launcher_lf_process_id`, read by `desktop::shells::hosts`) or
from an lf started inside a held terminal (`held_terminal_processes`). With no
Process rows there is no parent chain. Replacement: membership by terminal.
Open work is the Task's live Sessions and Graph runs. Each belongs to a shell
when its own `terminal_id` / `held_terminal_id` is one of the Task's
`desktop_shells.terminal_id`. Then `launcher_lf_process_id` and
`held_terminal_processes` are D, and the walk goes. Lands with the Session
slice; test: a Task with a Desktop shell running an agent retires and closes
the shell, and a Task whose Graph run is outside any shell does not.

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

What breaks without a bridge: an old Graph runner's next step runs the new lf,
writes no `processes` row, and `child_process_after` never finds it; an old
Task-run loop never sees its new Graph child; an old launcher's endpoint,
exit and re-token land only in `processes`, so new readers show its Session
stale. A new lf taking over an old-held Session repoints
`agent_sessions.agent_process_id` to an id with no `processes` row, so the old
holder's fence fails closed (correct).

**Decision: a SQL bridge in the expand migration, nothing in Rust.**
- old → new: triggers on `processes` insert/update copy agent rows into the
  current Session's columns (only while `agent_sessions.agent_process_id`
  matches and the token is the one being replaced) and `lf` rows that an owner
  points at into that owner's result columns.
- new → old: triggers on owner inserts/updates write the matching
  `processes` row (Graph run with `parent_lf_process_id` = the carrier that
  spawned it, step with parent = Graph run, agent with attachment), so an old
  runner's `child_process_after` and `graph_child_after` find new children.
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

Journal rows: `record_process_in`, `record_hosted_process`,
`process_is_recorded`, `ledger_insert`'s store arm, `FORGETTABLE`, receipts
(`write_process_receipt`, `remove_process_receipt`, `recorded_process_receipt`,
`prune_process_receipts_at`, `process_receipt_path`), both locators and
`process_locators_*` triggers. Tree walks listed under approach 1. Ancestry
repair (`unrecorded_agent_ancestry`, `release_agent_ancestry`, its doctor
check and `check_lineage`). `lf top`, `lf ps` call trees (`top.rs`
`ActivityReader`), `lf process` namespace, `lf history list`, history
`--parent/--caller/--outcome` filters, `lf context --process`,
`ProcessCursor` export, `session_owner` and `transport_lf_process_id`
readers, `relocated_task_processes` writer, Task Work `processes[]` and
`lf_process_ids`/`task_of_process`, `process_page` DTO and
`LfProcess.swift`. Synthetic holder rows. The `processes` arms in
`canonical_ids` preflight and `query_plans`. Each deletion takes its
exclusive tests and fixtures (`process_ownership_tests.rs` keeps only what
tests Session ownership).

Must survive: Session state/available, steer delivery and idle wake, stop and
reap, Graph retry and repeated-failure stop, ship "started by", CI repair
lifetimes, project activation readiness, terminal retention, `lf task
interrupt`, store-failure best-effort.

## Forbidden outcomes

A reader of `processes` in new code; a Rust writer of `processes`; a second
registry (receipts, an env-scan source); Task state reading the OS from SQL;
two live agents on one Session; a `Legacy*` type.

## Slices (one PR, after LOO-936)

1. **Expand migration** `process_rows_on_owners.sql`: new columns; rebuild the
   tables whose foreign keys name `processes` without those keys (a rebuild
   keeping every column is additive); backfill; bridge triggers; drop
   `task_update_processes_*` and revision triggers on `processes` (the bridge
   covers old readers' wake-ups through owner triggers). Test: seeded store
   at the released schema → migrate → every owner column matches its
   Process row.
2. **Runs own their results.** Spawners mint ids; Graph run, step, carrier,
   ship, repair and activation write start/pid and end/exit/error; journal
   stops recording; receipts go; `RunResult` derivation; `finish_runtime`
   writes no error for a plain 130. Test: a Graph whose second step fails →
   `lf task status --json` shows the step failed with its error and the Graph
   retry stops on the repeated cause, with `processes` empty.
3. **Session attachment.** Fences, reach, stop/reap on `agent_sessions`;
   `prepare_resume` beside path ends. Test: handoff from one lf to another →
   the old holder's next fenced write is refused; steer reaches the new agent.
4. **Delete `lf ps`, doctor strays, deletions** (CLI, DTOs, Swift,
   operate skills, docs). The liveness pass lands with the journal stop
   (concept review after the carrier slices). Test: `lf graph list --json`
   shows a SIGKILLed run ended.
5. **Bridge test.** Released-shape writer SQL (copied from the 0.16.0
   statements) drives an old Graph runner's step discovery against a new step
   → found; old launcher's endpoint update → visible in `lf session list`.
6. **File the contract Task**: after `STAGING_DAYS` (30), export `processes`
   to JSON lines (`-- exports:`), drop it, the bridge triggers,
   `relocated_task_processes`, `transport_lf_process_id`,
   `tasks.ship_lf_process_id`, every remaining foreign key to `processes`
   (rebuild `agent_sessions`, `desktop_shells`, `ci_incidents`, `waves`,
   `task_graphs`, `session_events`, `steer_*`, `tasks` without them),
   `steer_batches.attachment_token`, unused locator indexes.

## Acceptance at gate (headless, private `LF_HOME`)

- `cargo nextest run -p loopflow-storage -p loopflow-execution --lib` and the
  slice tests above pass.
- `python3 -I scratch/process_sql_scan.py rust` → only the expand migration's
  bridge and fixtures name `processes`.
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
- `prepare_resume` stops leaving a second agent running.

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
