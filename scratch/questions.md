# LOO-874 plan and open questions

Docs written for this Task: `docs/dev/machines.md` (Own Tasks, checkout paragraph,
move Failures, Auto-Ship, pools), `docs/dev/reference-processes.md`,
`docs/dev/reference-models.md`, `docs/dev/reference.md`, `docs/dev/reference-code.md`.
Migration: `rust/loopflow/src/store/migrations/task_machine_local.sql` (edit in place).
Contract Task: LOO-877 (538340, "Drop the Task, ownership and Machine columns
LOO-874 replaced"). It drops only what the merged migration replaced.

## Done (2026-11-12)

- Ownership entries name their writer (`task_ownership.writer_machine_id`,
  `TaskOwnershipEntry.writer_machine_id`); `written_here` is still written for
  released lf and no longer read. Migration backfills local entries.
- `handoff` and `recorded` read as `claim` (serde alias); a move writes a claim
  naming the target, written here. `OwnershipKind` is `claim | unclaim`.
  `handoff_task` → `move_claim`, `OwnershipChange::Handoff` → `Move`.
- `task_moves` (`sent` with payload → `arrived`): `lf task move` records the
  sent Move, a retry of the same command redelivers it (no stop/commit/push) and
  continues the Graph; receiver records `arrived`; `lf task status` prints
  `Moving to mini since …: sent, not arrived` + retry line.
- Checks run: lib `task_ownership::`, `task_moves::`, `rentals_tests::`,
  `revisions::`, `store::migrations::`, `server::schema::tests::`, `lf::tests::`;
  integration `dto_fixtures::`, `documented_commands::`, the move/ownership cases
  in `machine_commands::` incl. the new retry test; clippy; all pass.
- Swift has no mirror to change: no Swift type decodes ownership entries
  (`swift/` names neither `writer_machine_id` nor `handoff`); only
  `tests/fixtures/dto/task_status.json` and `task_lifecycle.json` changed.
- Compress: a move received on this Machine records `arrived` once (the sender
  no longer adds a second row). `cargo test --test integration --
  machine_commands::task_move …` 8 passed.

## Done (2026-11-12, second pass): `task_checkouts`

- Table (task, worktree, branch, base_commit, parent_pr_id) in the migration,
  backfilled from local `tasks` rows; unique `worktree`, `rtrim(worktree,'/')`
  and `branch` indexes; `task_checkout_to_pr_*` triggers replace
  `task_placement_to_pr`; revision triggers bump `planning` and `task_updates`.
- Every production reader and writer moved: `TASK_COLUMNS` LEFT JOINs it,
  `task_work` membership reads `TASK_CHECKOUTS`, placement/restack/stack,
  retirement, relocation, location, shipping, process filters, Task state
  (`unstarted` = no checkout here). `TaskCheckout.machine_id` deleted with its
  "Machine unknown"/"belongs to another Machine" branches. Old `tasks` columns
  are no longer written; Task 538340 drops them.
- Unmigrated readers (`read_task_branches`, `read_task_worktree_ownership`)
  fall back to the old columns when `task_checkouts` is absent (Task 538340).
- `lf task status` prints `Not checked out on <host>` (indented under a
  `Runs on` line) and no path; test
  `machine_commands::a_machine_without_the_checkout_shows_no_path_for_it`.
- Wire shapes unchanged: `TaskWorktreeSnapshot.machine_id` and
  `TaskSnapshot.machine_id` now always name this Machine when present.
- `task_pr_placement_*` and retirement-cancel triggers recreated against
  `task_checkouts`; backfill treats a worktree with no Machine as this one's.
- EXPLAIN QUERY PLAN on a 10,000-Task / 50,000-Process store: every new join
  searches an index (`task_checkouts_branch`, `task_checkouts_root`, primary
  key); `TASK_COLUMNS` by repository scans `tasks` as before.
- Checks (2026-11-13): clippy `--all-targets -D warnings` clean; lib store::,
  ops::run::, ops::task::, ops::human_session::, waves, desktop, server::events,
  harness::task_updates, status_tests:: pass (store::tests::,
  store::migrations::tests:: and harness::task_updates:: stall when their
  tests run in parallel on this Machine; each passes alone); integration
  machine_commands::, dto_fixtures::, documented_commands::, global_commands::,
  work_open_tests::, cli_discovery::, build_cleanup::, performance_tests::,
  context/settings launch, task_initialization pass; transport_tests and
  session_lifecycle_tests binaries pass. Gate reruns affected suites once.

- Compress (2026-11-13): `lf task status` prints the move/elsewhere line and
  `Not checked out` in one match; `move_claim_keeping_conversation`,
  `retry_task`. `cargo test --test integration -- machine_commands::` 50 passed;
  clippy clean.

## Sync onto main 87f0e3c, then 709e029 (2026-11-13; the second brought only a ruff bump)

- Conflicts in `children.rs`, `task_ownership.rs`, `task_retirement.rs`:
  main's `RepoFacts` (`me`, `planning_connected`) kept, branch names
  (`move_claim`, `move_in`) kept; the retirement sweep's abandoned-checkout
  arm reads `tasks INDEXED BY tasks_abandoned` (a partial index the migration
  adds) cross-joined to `task_checkouts`, beside main's `hosted` CTE.
- Compress after the sync: nothing left to cut in the merged files.
- Main's `store/query_plans.rs` fixture now writes `task_checkouts`;
  `task_checkouts` joins `GROWING`, with stats from the measured 838 checkouts.
- Main's `tasks_abandoned_checkout` index (`workload_indexes.sql`) indexes
  columns nothing reads now; Task 538340 drops it with them.

## Done (2026-11-13, third pass): ship log, pools, people

- `task_ship_events` (approved, carried, restarted, progressed, stopped,
  withdrawn; at, Process, person) replaces the local `ship_*` columns: every
  reader and writer (`task_work.rs` ship fns, `task_state_sql` carrier,
  process pruning) reads the log; backfilled from the old columns. `tasks.shipped_at`
  stays: it is the planning field (`task_shipped_at`) peers share. A move
  carries the standing approval (`MovedTask.approval`); arrival appends it
  with no carrier, so recovery restarts the ship process there.
- `pool_machines` (machine, pool, `cost_usd`, `expires_at`) replaces the
  `machines` columns in `rentals.rs`; backfilled.
- `people` (`linear:<viewer>` or `journal:<planning user key>`, name);
  `machines.person_id` is who the Machine belongs to (Linear viewer recorded in
  `assert_assignee`, journal key at provisioning and backfill; Linear wins);
  `task_ownership.person_id` and `TaskOwner/TaskOwnershipEntry.person_id` on
  the wire; the log shows `people.name` over the entry's name; matching ids
  decide "is this you" before the Git email does. No new email keys.
- EXPLAIN QUERY PLAN, 10,000 Tasks / 40,000 ship events: the state carrier
  subquery and log read search `task_ship_events_task`, pruning searches
  `task_ship_events_process`, pools `pool_machines_pool`, the owner name join
  the `people` key.
- Checks (2026-11-13): clippy `--all-targets -D warnings` clean; lib
  task_work::, task_ownership::, rentals, revisions::, query_plans::,
  store::migrations:: (131), ops::task::ship::, ops::task_ownership::,
  processes::, ops::human_session::, planning_peers, server::schema::tests::,
  waves `planning_failure_keeps_the_stored_workflow_readable` pass; integration
  dto_fixtures::, machine_commands:: (50), documented_commands:: pass.
  `server_schema.json` regenerated. Gate runs `two_people`, `transport_tests`.
- Compress: `ShipEvent` reads through serde like `OwnershipKind`; `ship_in`
  appends one approval branch; `Person: Default` in `hand_back_in`. Kept the
  move's empty `carried` row: recovery never reads an unknown (foreign)
  carrier as dead, so without it the moved Task would never restart shipping.
  Lib task_work::, task_ownership::, ops::task::ship::, store::migrations:: 161
  passed; clippy clean.
- Drop list for LOO-877: `tasks.ship_lf_process_id`, `ship_restarts`,
  `ship_approved_lf_process_id`, `ship_stopped_at`, `ship_stopped_lf_process_id`,
  `ship_stopped_person`, index `tasks_ship_process`; `machines.pool`,
  `cost_usd`, `expires_at`. (`shipped_at` stays.)

## Done (2026-11-13, fourth pass): Task primary Sessions

- `task_primary_sessions` (task → session, both unique, cascading) replaces
  `tasks.primary_session_id`, backfilled where the Session exists here. Every
  reader and writer in `store/sqlite/sessions.rs` moved (Task-primary filter,
  summary flag, `task_primary_in`, ensure/choose upserts, workspace
  admission, `primary_scope`), plus lib and integration fixtures. Arrival
  binds through `ensure_primary_session`, so the receiving Machine writes its
  own row. Test: `a_released_tasks_primary_session_stays_its_conversation_here`.
- On LOO-877's drop list (steer d5ca6933): `tasks.primary_session_id` and index
  `tasks_primary_session`.
- Checks (2026-11-13): lib store::sqlite::sessions, task_work::, selections,
  store::migrations::, ops::human_session 228 pass (incl. the upgrade probe,
  now asserting the `task_primary_sessions` index); integration
  machine_commands::, dto_fixtures::, work_open_tests::, documented_commands::
  139 pass; clippy clean. Gate runs session_lifecycle_tests, transport_tests,
  served_reads and task_graph_launch_tests (fixtures changed).
- Compress: `store/query_plans.rs` counts `task_primary_sessions` as growing,
  with the 299 measured primaries moved off `tasks_primary_session`; summary
  flag and inventory filter search its `session_id` index. Lib query_plans 6,
  server::boundary 13 passed.

## Done (2026-11-13, fifth pass): a Task's conversation moves only with its Task

- `moves::send` on a Task's conversation (its primary Session, else the one
  Task it belongs to: `moves::conversation_task`) runs `send_task`.
  `lf --machine M resume <session>` (`session_move::move_session`) runs
  `move_task` for one, so the move records `task_moves` and continues the
  Graph; a Task already on M moves nothing.
- Deleted `moved_task_conversation`, `task_conversation_moved_to` and the
  `leave` marker-retry branch; `resume --task` routes by ownership only.
  `agent_sessions.moved_to_machine_id` stays a Session fact.
- Tests: `resume_task_follows_a_task_session_moved_alone` →
  `resuming_a_task_conversation_on_mini_moves_its_task`;
  `a_repeated_task_send_carries_the_conversation_that_already_left` (tested
  the deleted branch) deleted, its conversation check moved into
  `a_move_that_did_not_arrive_shows_in_status_and_the_same_command_delivers_it`.
- Checks: lib `ops::human_session` + `task_moves::` 51 pass; integration
  `machine_commands::` 49 + the two changed tests pass; clippy clean.
  Gate runs `documented_commands::` for the machines.md paragraph.

## Remaining

Every Machine-local column the brief names (`worktree`, `branch`,
`base_commit`, `parent_pr_id`, `checkout_machine_id`, `primary_session_id`,
the `ship_*` set, `machines.pool/cost_usd/expires_at`) has no production
reader left outside the pre-`task_checkouts` fallbacks in
`read_task_branches`/`read_task_worktree_ownership` (store/sqlite.rs); the
remaining mentions are pre-migration fixtures.

1. Swift does not decode `person_id` (Codable ignores it); add it where the
   app shows owners.

## Open questions (each reversible; the simpler option is built)

- **Parallel stalls.** Whether the stalls of those three groups under the
  default thread count predate this branch is unchecked; gate's nextest run
  answers it.
- **Version skew.** A released lf still running after install reads a move's
  claim (written here, naming mini) as its own claim gone missing and may claim
  again; released lf writes entries with no `writer_machine_id`, which this
  build reads as written elsewhere. Window is live pre-install processes only.
- **Writer field optional on the wire.** Markers released lf delivered lack
  `writer_machine_id`, so it deserializes absent as null; Task 538340 makes it required.
- **People under the Git journal.** Built: the journal's planning user key
  (`journal:<key>`), the identity its refs already use. A Machine with neither
  Linear nor a provisioned key knows no person; its entries carry only the Git
  email, as before.
- **A person with both.** Linear's id replaces the journal key on the Machine;
  their journal-era entries keep the old id. Merging rows is open.
- **First claim on a Machine.** The Linear viewer is learned after the first
  claim's assignee write, so that claim names no `person_id`.
- **Seven ship columns.** `shipped_at` is shared planning, so it stays; the
  carrier and restart count joined the log as `carried`/`restarted`/`progressed`.
- **Primary Session not on `task_checkouts`.** Directed there, but
  `ensure_task` opens a Task conversation in the repository when the Task has
  no checkout, and a `task_checkouts` row means a checkout here (`worktree NOT
  NULL`; `unstarted` reads its absence). A Machine-local table keeps both.
- **Which conversation goes.** A Task move carries the Task's chosen
  conversation (`task_conversation`: primary, else most recent human input).
  Resuming a second open conversation of the same Task on M moves the Task
  with the other one, and the forwarded resume finds no such Session on M.
  Rare (two open conversations on one Task); not handled.
- **Lost reply from a remote source.** When the coordinator sends a Task owned
  on a third Machine and the SSH reply is lost, the source has completed the
  conversation and no `task_moves` payload exists anywhere; a repeat sends the
  Task without it. The deleted marker branch covered this.
- **Names.** `task_checkouts`, `task_moves` (`sent`/`arrived`),
  `task_ship_events`, `people`, `pool_machines`.
