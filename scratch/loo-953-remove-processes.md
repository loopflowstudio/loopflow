# LOO-953: no Process table; liveness on Sessions and Graph runs

## Status (2026-11-17, handoff attach deleted)

LOO-936 merged as #1965 (`fb14b674c9`) and #1964 with it; synced at
`ac93e4c0e7`, resolved against the LOO-936 branch history (main's tree
equals its tip). Inventory rescanned after the sync (79 statements; "Inventory").
Synced again onto #1958 (LOO-935, `1b888be85b`) at `e0a7ee14df`. It brought
`-- leaving:` marks and their checks: `scripts/schema_page.py --check`
(regenerated `SCHEMA.md` for this branch's new columns) and
`check_migrations.py`'s named-columns rule (`task_conversations`' `SELECT
candidates.*` now names its columns). #1958's `unused_store_members.sql`
already marks `processes.{viewed,native_input,viewer_hold,viewer_hold_at}`
and `tasks.ship_{,approved_,stopped_}lf_process_id` leaving → LOO-954, so
slice 6 does not drop those. When the journal stops, this Task's migration
marks `-- leaving: processes <contract Task>` (and each column the change
stops using), and `scripts/due_removals.py` dates the contract.

Built: slice 2a ("Slice 2a as built", QA in `scratch/qa.html`); the
renames to `Execution`, `ExecutionResult`, `ExecutionStart`,
`GraphExecution`, `LF_EXECUTION_ID` ("Direction from neighbouring Tasks");
CI repair runs on `ci_incidents.repair_{started_at,pid,os_started_at}`
(reserve and handoff write them, settlement clears them, liveness by
`run_evidence`); `detached::start_lf` returns `Started { id, pid,
os_started_at }`, so `engine::launch` waits on its child without the store;
the owner items, carriers, the Session lifetime, the attachment cut and the
handoff deletion below.

Next, in order (why this order: "Concept review after the Session
lifetime cut"): (1) the lf-row readers: `task_open_process_pairs` and
`task_state_sql`'s `open_processes` arm as live Sessions + Graph runs,
retirement by terminal (Graph runs gain `held_terminal_id`),
`process_starter`, `run_task_graph`'s parent check (`graph.rs:83`), and the
Rust readers the SQL scan misses (list under "Inventory"), plus the lf-row
reads the attachment cut left: `agent_parent` (deleted: the trace rides
in `AgentCaller`, see "Concept review after the handoff deletion"), `pending_inputs`'
`attached` join (an input ends when its lf's row does), `native_exit`'s
launcher end; (2) the expand migration's foreign-key rebuild together with
stopping the journal writer **and the owner liveness pass** (see "Concept
review after the carrier slices"); (3) slices 4–6.
Quotes and the OS-scan measurement: `questions.md`.

**Handoff attach deleted (third cut of slice 3).** Per the concept review
after the attachment cut (79026de79f; every production attach replaced the
provider, the non-replacing path was test-only):

- `attach_in`, `attach_session`, `claim_session_attachment` take no
  `replace_provider`; every attach mints a new agent and token in one
  `UPDATE`, ending a replaced agent that exited or never spawned. The
  non-replacing branch and `session_activity`'s token rebinding are gone.
- `SessionAttachment.provider_lf_process_id` → `launcher_lf_process_id`
  (serde keeps `provider_lf_process_id`: kept exit records and held hosts
  carry it). Kept, not dropped: it outlives release (`lf_process_id` is
  `None` then) and `native_exit`/`transport` name the launcher with it.
- Fence: `require_current_actor_in` = current `agent_process_id` and
  attached; `finish` completes on `attached_lf_process_id IS NOT NULL`;
  `agent_parent` drops the provider == origin filter. The launcher-vs-attached
  check in `native_provider_attachment` was unreachable and is deleted.
  `agent_launcher_lf_process_id` stays while `native_exit` ends the launcher.
- Tests: handoff-only ones deleted (`stale_orphan_observation_cannot_settle_a_transferred_agent`,
  `native_cleanup_cannot_kill_the_same_agent_after_takeover`,
  `pi_stale_attachment_cannot_send_or_stop_the_provider`,
  `a_delayed_codex_start_keeps_request_capture_across_a_b_a_takeover`);
  the rest keep their non-handoff behaviour on replacement or a single
  attachment (attachment/steer/program-status/session-exit/runtime close/
  capture receipt/native move/remote client/opencode and Claude stop,
  `session_steers` context capability). Docs: `agent-api.md`,
  `dev/reference-models.md` no longer describe a handoff.

Check: storage lib 384/384 except the one handoff test (fixed, rerun
passes); execution `store|harness|process|session_record|transport::registry_tests|terminal_title|ops::human_session|observe` 609 run, the 10 handoff failures fixed and rerun green;
loopflow `server::{boundary,wake}`, `subscription_measurement`,
`public_context_capability_rejects_children_and_replacement`,
`documented_commands::` pass; `cargo clippy --all-targets -D warnings` clean.
`binary(session_lifecycle_tests)`, `process_ownership_tests`,
`session_cli_tests`, `work_open_tests` (callers only dropped the argument) → gate/CI.

**Concept review after the handoff deletion (2026-11-17): next implement direction.**

The expected result holds: no `replace_provider`, no non-replacing branch,
no `session_activity` rebinding. What the deletion now implies, folded into
step (1), the lf-row readers:

- **An attachment's lf is its launcher.** Every attach writes
  `attached_lf_process_id = agent_launcher_lf_process_id`, and release only
  clears the attached one. Same agent ⇒ same launcher, so
  `agent_parent`'s attachment read is a no-op: its answer is always
  `caller.origin_lf_process_id`. The released token stays (it is the
  compare-and-swap value the next attach passes as `expected`).
- **`agent_parent` reads nothing.** The agent edge needs only the launcher's
  trace, which the launcher knows when it writes `LF_AGENT_CALLER`. Put
  `trace_id` in `AgentCaller` (serde-optional for retained provider
  environments: an absent trace mints a fresh one, as an unresolved caller
  does today) and delete `agent_parent`; the journal's agent edge then takes
  parent and trace from the env. This replaces "convert `agent_parent`'s
  trace" in step (1) with a deletion.
- Stale handoff prose fixed at realign (`SteerAdapter`, `record_native_turn`,
  `lock_session_dispatch`, `native_exit`'s context, `terminal_title`). "Handed-off
  exit" in `transport`/`native_exit` means the launcher's exit record passed to
  its host, not an attachment handoff, and stays.
- `SessionAttachment.lf_process_id: Option` + `launcher_lf_process_id` encode
  one lf plus "held". Left as is: the column pair goes with slice 6, and the
  bridge writes released readers' `attached_lf_process_id` from it.

No open product choice.

**Attachment cut as built (second cut of slice 3).**

- `agent_sessions` carries the whole agent: `agent_launcher_lf_process_id`,
  `agent_spawn_state`, `agent_command`, `agent_interactive`,
  `agent_endpoint`, `agent_inbox`, `attachment_token`,
  `attached_lf_process_id`, `attached_pid`, `attached_os_started_at`,
  `attached_terminal_id` (prefixed names; the plan said `endpoint`, `inbox`,
  `terminal_id`). Backfilled; indexes `agent_sessions_attached`,
  `agent_sessions_unended_agent`. lf writes no agent row: `attach_in`,
  `SessionTransfer`, settlement, launch/exit/identity, endpoint, inbox and
  terminal write the Session. `attach_in` writes a new agent and its token
  in one statement.
- Bridge, both halves in SQL: old → new (`session_agent_from_process_*`,
  `session_agent_from_attachment` when the agent changes and the token does
  not, `session_attached_terminal_from_process`), new → old
  (`process_agent_from_session_insert` when agent and token change together,
  `_update`, `process_terminal_from_session`). Every copy's `WHERE` compares
  each column it writes. The bridged insert keeps `agent_sessions`' and
  `steer_*` foreign keys satisfied until the expand rebuild.
- Holder liveness: `AttachmentHolder { pid, os_started_at }` (none when
  released); `journal::holder_evidence`, `session_agent_evidence` (a
  reservation reads Dead with a dead holder or none). The projection's
  `holders` map hides a dead holder's terminal; `prepare_resume` refuses a
  holder that is not Dead; `reap_in` settles reservations and orphans by
  `holder_dead`, which also finishes a dead holder's attachment.
  `agent_evidence` reads `agent_state` (current agent only).
- `AgentProcessState` is flat (no `AgentProcess` inside); `agent_process(id)`
  reads the bridged row and is test-support only.
- One agent per Session: `prepare_resume`'s beside path, `runs_in_its_terminal`
  and `PreviousAgentDisposition::Running` are gone; `open_session` stops a
  bare `lf <skill>` running elsewhere (`docs/sessions.md` row updated);
  `attach_in` refuses to replace a spawned agent with no observed exit and
  ends a reservation it replaces.
- Deleted: ancestry repair (`UNRECORDED_ANCESTRY`, `UnrecordedAncestry`,
  `unrecorded_agent_ancestry`, `release_agent_ancestry`,
  `release_unrecorded_ancestry`, `repair_unrecorded_ancestry_in`,
  `prune_agent_processes`, `check_lineage`'s repair hint) with its tests,
  and `live_agent_inventory_upgrade_skips_other_processes`.

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

Checks (2026-11-17, attachment cut): storage `migration_runner::`,
`query_plans::`, `sqlite::{sessions,processes,provider_transitions,steers,
revisions,session_events}::`, `canonical_ids`, retention/task_work/
task_retirement/forget/durable/program_status/transport/planning_write →
pass (238); execution `store::`, `process::`, `harness::{agent_process,
native_session,native_exit,claude,codex,dispatch}`, `ops::human_session::`,
`capture_tests`, `registry_tests`, `fact_writer`, `terminal_title`,
`transport::` → 529 pass after fixture fixes, including the new
`a_reservation_under_a_killed_holder_is_not_live_and_the_next_pass_ends_it`;
`loopflow` `server::{wake,boundary}::`, `subscription_measurement`,
`session_steers::`, `session_cli_tests::`, `context_launch_tests::`,
`work_open_tests::` (`--test-threads 4`), `binary(process_ownership_tests)`,
`binary(session_lifecycle_tests)`, `documented_commands::`,
`graph_tests::authored_graph_records_each_skill_as_one_session` → pass;
clippy → clean. Not this change (gate): `work_open_tests::task_location_uses_the_named_plan_outside_a_checkout`
fails at `dfa49c4321` too ("not found"); `transport::registry::tests::an_interrupted_paste_ends_before_the_next_view_types`
timed out twice under load and passes alone (holder `exit.json`, no store);
`work_open_tests` time out when 90 terminal fixtures run at once. Gate also
runs `binary(two_people)`, `binary(transport_tests)` and `test(performance_tests::)`
for the trigger changes.

**Concept review after the Session lifetime cut (2026-11-17): next implement direction.**

A Process row is two things: an lf row (written by the journal) and an agent
row (written by the attachment writers). The agent row left first (built
above), because lf rows were read through the agent's attachment.

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
- **Attachment.** `attach_in` mints a new agent and token on every claim
  (no handoff; "Handoff attach deleted"). Fences: `require_current_actor_in`
  (current agent, attached not NULL), token match in `record_session_activity`,
  `session_liveness`, `session_turn_origin`, `TURN_LIVE`, `available_sql`.
  `steer_adapters`/`steer_batches` keyed by `agent_process_id` mean "this
  agent"; `steer_batches.attachment_token` has no reader (slice 6).
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

## Inventory: 37 production statements naming `processes` (2026-11-17, attachment cut)

Method: `python3 -I scratch/process_sql_scan.py rust` (55 statements, 15
joined, 21 writes), test files and fixtures dropped by hand. Composed SQL the
scan misses: `task_state_sql`'s `open_processes` arm (`durable.rs:420`) goes
through `task_work::task_open_process_ids`. Gone onto owners since base
`4a0691a9a0`: `completed_graph_steps`, `latest_task_graphs`,
`task_of_process`, `task_close_candidates`, `session_projection_inputs_in`,
`session_terminal`, `live_process`, `agent_process_identity`, `readiness_in`,
`pr_repair_lifetimes`, `graphs_in`, `entry_in`, `process_mark`,
`child_process_after`, `graph_child_after`, `failure_cause`, and the whole
**S** set (every agent and attachment statement in `processes.rs`,
`sessions.rs`, `session_events.rs`, `steers.rs`, `provider_transitions.rs`;
`agent_process(id)` stays as a test-support read of the bridged row).
Fates: **R** → lf-row readers change, **J** → journal-stop change, **D** →
slice 4 deletion, **F** → fixture.

| File (`loopflow-storage/src/`) | Function:line | Fate |
| --- | --- | --- |
| `sqlite.rs` | `open_existing_processes`, `validate_process_schema`, `record_hosted_process`, `process_is_recorded`, `record_process_in` | J (journal writer; `attach_in` still lands the attaching lf's row) |
| `sqlite/retention.rs` | `forget_unread_rows` 132 (LOO-936: clears `attachment_exit_seq`), `process_receipt_select` 233 ×2, `close_dead_processes` 266, `forget_processes` 291 | J → liveness pass over owners |
| `canonical_ids.rs` | `preflight` 52, 87 | D arm |
| `query_plans.rs` | 51 | D arm |
| `sqlite/sessions.rs` | `pending_inputs` (`attached` lf join) | R |
| | `session_owner` | D |
| | `terminal_retention` 882 | R (Graph run's `held_terminal_id`) |
| `sqlite/task_work.rs` | `task_open_process_pairs` 673 (and `task_state_sql` through it) | R (live Sessions + unended Graph runs) |
| | `lf_process_ids` 122 | D (`processes[]` DTO) |
| `sqlite/graph_inventory.rs` | `process_under_graph` 302 (`run.rs` capacity wait) | R (a step knows from `GRAPH_STEP_ENV`) |
| | `graph_execution_facts` 642, 647, 686 | J (receipt and lf-row arms) |
| | `test_graph` 734 | F |
| `sqlite/machine_records.rs` | 16 (rental export) | D |
| `sqlite/repositories.rs` | `relocate_repository` 138 (`relocated_task_processes`) | D |
| `sqlite/processes.rs` | `agent_parent` (launcher trace) | R |
| | `held_terminal_processes` 867, `process_starter` 900 | R |
| | `FORGETTABLE`; `process_query`; `unfinished_processes_except`; `process_receipt_locator`; `journal_trace_locator`; `resolve_process` | J/D |
| | `test_lf_process` 1062 | F |
| `loopflow/src/server/subscription_measurement.rs` | `attach` | F |

Rust readers of an lf row through `SqliteStore::lf_process` (not SQL
literals, so not in the scan), all **R** unless marked:
`lf/commands/graph.rs:83` and `run.rs:126` (caller's Graph = own row's
parent; both read the inherited parent id instead); `task_create.rs:141,148`
(`--run` waits on the creation Graph's carrier row: wait on `task_graphs`
`run_pid` / `carrier_evidence`); `task_view.rs:130` `step_log` (step command
and outcome: the step's `Execution`); `journal::lf_evidence` (remaining callers: `owner_running` in
`human_session.rs`, `ops/task.rs:3107`, journal tests) and `live_process_pid`
(`end_earlier_owner`, which reads the legacy `session_owner`, D);
`native_exit`'s launcher end (J); `process_entry_evidence` → D with `lf ps`.

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

Deleted: the handoff attach (`replace_provider` and its non-replacing branch).
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
   `prepare_resume` beside path ends. Test: an lf replaces an exited agent →
   the old holder's next fenced write is refused; steer reaches the new agent.
4. **Delete `lf ps`, doctor strays, deletions** (CLI, DTOs, Swift,
   operate skills, docs). The liveness pass lands with the journal stop
   (concept review after the carrier slices). Test: `lf graph list --json`
   shows a SIGKILLed run ended.
5. **Bridge test.** Released-shape writer SQL (copied from the 0.16.0
   statements) drives an old Graph runner's step discovery against a new step
   → found; old launcher's endpoint update → visible in `lf session list`.
6. **Mark leaving and file the contract Task**: `-- leaving:` marks in
   `process_rows_on_owners.sql` name it; after `STAGING_DAYS` (30), export `processes`
   to JSON lines (`-- exports:`), drop it, the bridge triggers,
   `relocated_task_processes`, `transport_lf_process_id`,
   every remaining foreign key to `processes`
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
- `prepare_resume` stops leaving a second agent running; `lf resume` of a
  bare `lf <skill>` running in another terminal stops it there, as it does
  for any other client (built 2026-11-17).
- `attach_in` refuses to replace a spawned agent with no observed exit
  instead of leaving it running unlisted; only fixtures reached that path.
- A holder that recorded no OS process reads Unknown, so it cannot be taken
  over; fixtures say who holds an attachment with `test_set_holder`.

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
