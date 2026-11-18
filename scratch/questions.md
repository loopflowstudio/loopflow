# LOO-953 quotes and open choices

Copied from LOO-936 (`git show f80183a113:scratch/questions.md`), only what
bears on Process removal. Speaker: Jack Heart, 2026-11-16.

## What was said

- On per-kind Process tables: "do we need both?", "what are hte process tables
  for", "should that be proc.db then or something", "now that we've moved to
  having a machine server and processes dont even map 1:1 with agent sessions
  i am unconvinced something like lf top and lf ps even really makes sense".
- On "A Machine-wide view of what's running ... becomes a list of live
  Sessions and Graph runs from the server, not a Process tree": "Check".
- On bare `lf -b <skill>` runs the server does not hold: "These should
  probably register themselves in some db somewhere", then "maybe that is the
  process table all over again hmm", then "on the other hand can we just
  filter regular lf to ps?", then "go for it" (check it).
- On "rows are the source (pid and start time on the Session or Graph run),
  the OS is the check, no registry and no Process table, environment scan as
  a diagnostic": "all seems fine."
- On `outcome`: "i just wanted to flag to you as somethign that needs a better
  design"; "and like maybe lf processes and agent processes shouldnt be in the
  same table?"
- On attachment placement and Task state: "(1) on the session (2) scond"
  (attachment token and attached Process on `agent_sessions`; Graph and ship
  records carry their own end, result and error).
- On `lf history`: "what im most interested is being able to investigate
  things like tokens and time by wave or by skill or other things like that".
  LOO-936 builds that as `usage.db` turn rows.

- 2026-11-17, on whether `lf ps` keeps a refreshing view: "i thikn we delete
  lf ps?". On `lf graph show <RUN_ID>` vs `lf graph runs show <RUN>`: "what
  else would it accept".

## What the OS process list showed (2026-11-16 18:40 PST, macOS)

`lf ps --json` listed 219 Processes, all alive. `LF_STEER_SESSION` named the
Session on 46 of 46 `codex -c` and 23 of 24 `claude --name`. 21 showed no
`LF_*` variable: all 8 `lf holder` and 13 `lf` a person started. 278 OS
processes carried `LF_PROCESS_ID`, `LF_TRACE_ID` or `LF_HUMAN_SESSION`; 128 of
them were not in `lf ps` (inherited children: `browse` 50, python 24,
`codex-code-mode-host` 15, `claude` 10, `codex` 10, `uv` 4). Read from an
agent's Bash with no sandbox; a sandboxed read is untested.

## Settled earlier (history in `git show e38387f2d2:scratch/questions.md`)

Doc-writing questions (`lf ps` refresh, idle Sessions, `lf graph show
<RUN_ID>`) and slice 2a calls (step-first failure cause, a separate
adopt-this-id variable, dev-only skew accepted) are settled in the plan's
concept review after slice 2a.

## Sync checks (2026-11-16)

- `cargo test -p loopflow --lib graph_tests::authored_graph_records_each_skill_as_one_session -- --exact`: passed; initial compile exposed main's Task summary reference to `graph.process`, repaired to `graph.run`. Gate/CI run suites.
- `cargo test -p loopflow-storage --lib sqlite::task_summaries::tests::an_abandoned_task_keeps_one_summary_of_its_run -- --exact`: passed with the owner-record fixture; gate/CI run suites.
- `cargo fmt` and `cargo clippy --all-targets -- -D warnings`: passed.

## Rename checks (2026-11-16)

- `cargo build -p loopflow --bin lf`, `cargo fmt`, `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo test -p loopflow --lib graph_tests::authored_graph_records_each_skill_as_one_session -- --exact`: passed.
- `cargo test -p loopflow-storage --lib sqlite::task_summaries::tests::an_abandoned_task_keeps_one_summary_of_its_run -- --exact`: passed.
- `cargo test -p loopflow-storage --lib sqlite::graph_inventory::` (5) and `cargo test -p loopflow-execution --lib harness::task_updates::` (8): passed. Gate/CI run suites.
- `lf graph` now records `cwd` from the Graph's working directory and `repo` from the journal's main checkout (`journal::current_repo`), where both came from the journal context before; the two cwd values are the same path.

## Compress checks (2026-11-16)

- `cargo clippy --all-targets -- -D warnings`: passed after `RunStart::reserved` and bool `registration`.
- `cargo test -p loopflow --lib graph_tests::authored_graph_records_each_skill_as_one_session -- --exact` (1) and `task_graph_launch_tests::` (56): passed.

## QA (2026-11-16)

- `lf graph show <RUN_ID>` failed with "graph not found" though docs and `task_operate` use it; now falls back to the recorded run. Walkthrough in `scratch/qa.html`.
- Docs and operate skills already describe `lf ps` as live Sessions and Graph runs and `lf machine doctor` strays; neither is built, and both wait on LOO-936 (unmerged; origin/main `4852348e5c` has no LOO-936 commit). Call: keep the docs untrimmed. One Task is one PR, and this branch merges only after the Session slices land.
- `lf graph show <RUN_ID> --json` steps now carry `error` (DTO `GraphStepProcess.error`, Rust and Swift, fixtures and schema).
- `lf task status` labelled the interrupted carrier (`lf task run`, a Process with no run row) "failed" from `processes.outcome`. Process lines now take their label from the row's end, exit and error by `RunResult::of`, so it reads `interrupted`; a Process line repeating a Graph run's line is omitted. `processes.outcome` itself and the remaining Process lines go with the Process-line slice.
- Checks: `cargo test -p loopflow-storage --lib sqlite::graph_inventory::` (5), `cargo test -p loopflow --test integration dto_fixtures::` (52), `cargo test -p loopflow --lib server::schema::tests::` (8), `task_graph_launch_tests::an_interrupt_during_an_operation_stops_the_graph_before_its_next_step`, `scripts/test_desktop.sh --filter 'DTOFixtureTests|TaskGraphProofTests|DesktopHeadlessTests'` (82), clippy and fmt: passed.

## Slice 2a review (2026-11-17)

- Fixed: a reserved step whose start failed (admission, terminal output,
  `configure_child`) or whose wait failed was left with no pid and no end, so
  the Task read "liveness unknown" until another run. The spawner now ends it
  with the error; `journal::step_evidence` takes a reserved step's liveness
  from its Graph run, so a killed runner's reserved step reads dead.
  Test: `journal::tests::a_reserved_step_ends_with_the_graph_that_would_start_it`.
- Fixed: `agent_limit_recoveries` now has a foreign key to
  `graph_process_steps` (`process_rows_on_owners.sql:51`). `_note_limit_recovery`
  (`agent/mod.rs:289`) writes for any lf, so a bare `lf -b <skill>` hitting a
  limit logs "could not note limit recovery" every time; the migration drops
  the one such row in the live store (1 of 323). The note now inserts only
  for a recorded step (released lf still logs the warning until it updates).
- The spawner's `finish_run(...)?` failing a Graph on a busy store: settled
  as warn-and-continue in the plan's concept review after the owner slices.
- Minor: `GraphStepProcess.error` decodes a missing key (siblings do too);
  step `outcome` is now `"unknown"` where it was `null`; `latest_task_graphs`
  and `graphs_in` order by `graph_processes.rowid` (`ORDER BY started_at, rowid`
  would say it).
- Missing tests: spawner-written end for a child that died without writing
  one; `graph_failure_cause` driving the repeated-failure stop.

## Rename checks (2026-11-17)

- `cargo build -p loopflow --bin lf`, `cargo clippy --all-targets -- -D warnings`: passed. `sqlite::graph_inventory::` (5), `journal::tests::a_reserved_step_ends_with_the_graph_that_would_start_it`: passed.
- Call: the plan said "both `RunStart`s"; `ops::task::RunStart` is where a Task run sets out in its Graph, not an execution's start, so it keeps its name.

## Owner slices (2026-11-17)

- Checks: `cargo clippy --all-targets -- -D warnings` passed; `loopflow-storage` `run_owners_carry_their_process_lifecycle_after_upgrade` (activation and repair backfill and bridge), `an_activation_reads_pending_until_its_lf_writes_its_end`, `sqlite::graph_inventory::` (5); `loopflow-execution` `ops::pr_landing::`, `os_process::detached::`, `engine::launch::` (37): passed. Gate: `binary(land_tests)` and `binary(release_tests)` reach CI repair; CI runs them.
- Call: a ci-fix run that dies unsettled reads dead from its pid and leaves no repair window; before, its Process row's `completed_at` closed the window. Settlement on the next landing check writes `repair_finished_at`.
- Call: `engine::launch`'s registry-recovery test was deleted with what it tested (report wait reading the store); a dead child reading "ended without a result" from its pid replaces it.

## Carrier slices (2026-11-17)

- Call: `finish_run(TaskCarrier)` clears `run_pid, run_os_started_at` and keeps `task_graphs.lf_process_id`, against the concept review's "clears `lf_process_id`": `row.run` needs the id to resume a stopped run from its cursor (`ops::task` `(None, Some((cursor, _)))`) and to show it stopped.
- Call: `task_state_sql` reads a stored pid (`run_pid` / ship event `pid` not NULL), not OS liveness; SQL cannot call `process_identity_evidence`. A SIGKILLed carrier reads moving until the liveness pass, as its Process row does today.
- Call: ship failure cause is the Task's latest Graph run's `graph_failure_cause`, else the ship carrier's own error (its own error alone lost the agent's exit in `task_ship_restarts_after_a_failure_wait_instead_of_on_every_read`); `failure_cause`'s child walk is deleted.
- Call: `lf task run` marks `ShipCarrier` up front, since arrival at `end` may hand the Task off with it as carrier; a no-op end write when it never ships.

## Session lifetime (2026-11-17)

- Call: the Session's agent columns are filled by triggers from the agent row for this lf too, not by Rust writers, until the attachment moves onto the Session. The agent row's writers (`attach_in`, `replace_agent`, `finish`, `commit`, settlement, retention) carry the attachment in the same statements, and a replaced or detached agent has no Session to write; moving them is the attachment cut.
- Call: retirement-by-terminal waits for `task_open_process_pairs` to become live Sessions + Graph runs, since Graph runs have no `held_terminal_id` yet.

## Test-order leak from main (seen at the 2026-11-17 sync onto 1b888be85b)

Under plain `cargo test -p loopflow-execution --lib ops::human_session::`,
`resuming_an_ended_conversation_launches_its_provider_as_the_session` (#1958)
fails with "Native launch has no admitted invocation" when
`resume_refuses_a_run_and_leaves_its_provider_running` (#1954) ran first in
the same process: `journal::with_process` sets the global `PROCESS_STARTED_AT`,
so `is_cli_process()` stays true and `with_runtime` skips admission. Each
passes in its own process (nextest). Fix: run the `with_process` test in an
isolated child process, as AGENTS.md asks of process-global tests.

## lf-row readers (2026-11-17)

- Call: a route approval (a Task run reaching `end`) records no caller Session, so "Auto-Ship approved by" names the person even when an agent started the run. The Graph run's own `caller_session_id` could supply it; left until someone notices.
- Call: open work counts only open Sessions (completed Sessions with an unended agent are strays for doctor) and unended Graph runs; an lf command running in a checkout that is neither (e.g. `lf task land`) no longer blocks retirement or landing.
- Call: a Graph run whose step outlives it no longer holds the Task's work; the step's own Session does if it runs an agent.
- Call: Desktop shells mark membership with an inherited `LF_HELD_TERMINAL`; a person can unset it in a shell and their work then blocks retirement, as an unknown process would.

## Journal stop (2026-11-18)

- Call: `binaries.rs` (install's and doctor's superseded-binary report) read live lf rows, which stop existing; it now reads `recorded_lfs()` (owner rows with a pid and each Session's attached lf) plus unfinished Process rows for a released lf. The plan had listed no fate for it.
- Call: a read-only Graph step (`graph show …`) admits no Process, so it once wrote its end only through `observe_process`; `with_process` now creates the context at exit when `LF_EXECUTION_ID` is set, and nothing else.
- Call: the CI repair arm stays out of the liveness pass; no Process end ever ended an incident's repair, and readers check its pid inline.
- Open for slice 4: `lf ps` lists only released-lf rows and bridged agent rows now, since nothing else writes Process rows.

## Slice 4b names (2026-11-18, headless)

- `lf session history show` is served as `session.history.get`, not the
  planned `session.history`: that kind already serves `lf session history`.
  Reversible; `session.show` was the other candidate.
