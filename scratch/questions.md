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

- `task_ship_events` (approved, carried, restarted, stopped, withdrawn; at, Process, person) replaces the local `ship_*` columns: every
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
  `cost_usd`, `expires_at`; index `tasks_parent_pr` (main's, on the old
  column). (`shipped_at` stays.)

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
  `agent_sessions.moved_to_machine_id` stays for Task-less Sessions only
  (seventh pass).
- Tests: `resume_task_follows_a_task_session_moved_alone` →
  `resuming_a_task_conversation_on_mini_moves_its_task`;
  `a_repeated_task_send_carries_the_conversation_that_already_left` (tested
  the deleted branch) deleted, its conversation check moved into
  `a_move_that_did_not_arrive_shows_in_status_and_the_same_command_delivers_it`.
- Checks: lib `ops::human_session` + `task_moves::` 51 pass; integration
  `machine_commands::` 49 + the two changed tests pass; clippy clean.
  Gate runs `documented_commands::` for the machines.md paragraph.
- Compress: the Task-conversation check sits in `session_move::move_session`;
  `relay` moves only Task-less Sessions and returns `()`. Clippy clean;
  integration `machine_commands::` resume/move tests 4 passed.

## Done (2026-11-13, sixth pass): the Machine a Task leaves records the move

- `moves::send_task(store, issue, to, conversation)` writes `sent` with the
  payload before returning; a repeat naming the same `to` returns that
  payload. `claim_for` refuses a Task `to` already owns (its claim-reuse
  branch and `send_task`'s `sent_before` are deleted).
- `relay_task` no longer reads its own `sent` record. When the route already
  says `to`, the source is the writer of the move's claim (`sender`), unless
  this Machine recorded the arrival. It asks that source, locally or over SSH,
  and then calls `session send --task X --to M --arrived` (hidden) on a remote
  source, best-effort, so the source stops showing `sent, not arrived`.
  `record_move_arrived` on the receiving Machine also settles any earlier
  move this Machine sent, so a stale row there never replays.
- `move_session` passes the named Session: `session send <id> --task X`
  (id no longer conflicts with `--task`) carries it, and it arrives as the
  Task's primary on M.
- Found while testing: a conversation moving back to the Machine it left
  failed in `provider_conversation::select` ("conversation capture changed
  before publication"); a reopened Session whose input was published skips
  publishing.
- Swift `TaskOwner.personId` (`decode(String?.self)`), `personKey` documented
  as the display fallback; `roadmap_snapshot.json` carries one person id;
  Swift JSON in tests and `MockWaveFixture` gained `person_id`.
- Tests: `a_task_sent_without_its_reply_delivers_its_conversation_when_repeated`,
  `resuming_a_tasks_other_conversation_on_mini_carries_that_one`,
  `task_moves` unit test extended (third Machine, then back).
- Checks (2026-11-13): integration `machine_commands::` 51, `dto_fixtures::`
  + `documented_commands::` 55; lib `ops::human_session` + `task_moves::` +
  `lf::tests::` 94; Swift `swift test` on the eight suites whose owner JSON
  changed (DTOFixtureTests, WaveStandup, RegistryQuery, TaskHistoryFilter,
  WorkCache, WorkNavigation(Proof), WorkDestination) 248; clippy clean. Gate
  runs `server::schema::tests::` (Clap help for `session send` changed) and
  `transport_tests`.
- Compress: one `send_from` builds the SSH `session send --task` call for
  both the send and `--arrived`; the resend message sits in its branch.
  Clippy clean; integration `machine_commands::` move/resume/sent 11 passed.

## Sync onto main 295f257b8e (2026-11-13)

- Combined preflight `OwnershipFacts` and store-only ended-Machine handback
  with person ids, writer ids and move-as-claim entries. Main's new rental
  test calls `move_claim`.
- `cargo test -p loopflow --lib task_ownership::`: 12 passed after fixing the
  new test's two old `handoff_task` calls; linker warned about the size of
  `__eh_frame`. `cargo fmt` clean. Gate owns static analysis and broader suites.

## Done (2026-11-13, seventh pass): a Task's conversation resumes where its Task runs

- `send_task` completes the leaving conversation with
  `complete_task_conversation`: no `moved_to_machine_id`, and the row is bound
  to the Task (with the Task's Wave, which the ancestry trigger requires) so
  it still maps to the Task after its checkout here is pruned.
  `complete_moved_session` (the marker) is only for Task-less Sessions.
- `work_route::moved_session`: a Session `moves::conversation_task` maps to a
  Task forwards to the Task's route (ownership log) when it runs elsewhere and
  resumes here when it runs here; others follow the marker as before.
- Migration clears `moved_to_machine_id` on Sessions with a `task_id` or a
  `task_primary_sessions` row. A pre-migration non-primary conversation whose
  checkout is gone keeps its marker (nothing maps it to a Task any more).
- Test `a_handed_back_tasks_conversation_resumes_here` (rental row for mini
  ended on the laptop, then `lf resume <session>` stays on the laptop).
- docs/dev/machines.md: one sentence under "The conversation goes only with its Task".
- Checks: integration `machine_commands::` resume/move/sent 15 passed; lib
  `ops::human_session` + `store::migrations::` 181 passed; clippy
  `--all-targets -D warnings` clean (covers the post-295f257b8e sync).
- Compress: the slice was already minimal (`complete_task_conversation` keeps
  its Task binding; a pruned checkout otherwise loses the mapping). Only
  `moved_session` tidied. `machine_commands::a_handed_back` + `resuming_a_task`
  3 passed; clippy clean.

## Done (2026-11-13, eighth pass): a move is a row from its claim

Closes the concept review's one gap (a kill between the claim and `sent` left
"Runs on mini" with no row, and a repeat refused).

- `task_moves` gains `leaving` (`claim_id`, `stopped`), written with the move's
  claim in one transaction (`begin_task_move`, which replaced
  `move_claim_keeping_conversation`). `leave` stores the stopped Graph on the
  row once `stop_graphs` returns (`record_move_stopped`).
- A repeat of `lf task move X --machine M` on a `leaving` row naming M reads
  the claim back and carries on leaving (commit, push, take), keeping the
  stored stopped step; a row naming another Machine refuses with its retry
  line. `claim_for`'s "already runs on" stays for a Task with no row.
- `hand_back` deletes the leaving row in its transaction. `record_move_sent`
  replaces the leaving row; the conversation completes after `sent` is
  recorded, and again (idempotently) on a sent repeat.
- `lf task status`: move line + retry line (wording now under "One move"); relay
  prints `Carrying on X's move to mini` for both repeats. machines.md updated.
- A retry that fails after an earlier attempt stopped the Graph hands back and
  drops the stored step, as a first attempt failing after its stop did.
- Tests: lib `a_move_leaves_from_its_claim_until_it_is_sent_or_handed_back`;
  integration `a_move_killed_while_leaving_shows_in_status_and_the_same_command_carries_it_on`.
- Checks: lib `task_ownership::` `task_moves::` `store::migrations::` 145 passed;
  integration `machine_commands::` move tests 11 passed; clippy clean. Gate runs
  `documented_commands::` (machines.md).
- Compress: one read, `task_move_in_flight` → `MoveInFlight { stage: Leaving
  { claim_id, stopped } | Sent { payload } }`, replaces `task_move_leaving` /
  `task_move_sent` and their two structs; `describe_move` builds the status
  line and the refusal for a move leaving elsewhere. Lib `task_moves::` +
  the leaving test 2 passed; integration `machine_commands::` move tests 5
  passed; clippy clean.

## One move, one status verb (concept review → implement, 2026-11-13)

Built: status and the refusal print `Moving to mini since <move began>: not
sent` or `…: sent, not arrived`, plus the retry line. The stored state keeps `leaving`.
Compress: `record_move_sent(task, payload)` only turns the leaving row into
`sent`, keeping `at`, and errors with none. The brief's insert fallback and
target-change reset were unreachable: `send_task` always begins the move
(`begin_task_move`) before sending, and a sent repeat returns early. Lib
`task_moves::` + `task_ownership::` 14 passed; integration `machine_commands::`
move tests 11 passed; clippy clean.

## Sync onto main 0afb302f52 (2026-11-13)

`query_plans.rs` combines main's refreshed measurements and foreign-key checks
with `task_checkouts` and `task_primary_sessions`. Main's stricter checks
needed: `task_checkouts_parent_pr` (partial, in the migration) for the
foreign-key lookup main indexes as `tasks_parent_pr`; MEASURED rows for
`tasks_abandoned` (259 abandoned Tasks on the Machine store) and the retained
`tasks_primary_session` (`319 1`, dropped in the merge),
`task_checkouts_parent_pr` (`11 2`). Lib `store::query_plans::` 9 and
`store::migrations::` 131 passed. `tasks_parent_pr` joins LOO-877's drop list.

## Remaining

Every Machine-local column the brief names (`worktree`, `branch`,
`base_commit`, `parent_pr_id`, `checkout_machine_id`, `primary_session_id`,
the `ship_*` set, `machines.pool/cost_usd/expires_at`) has no production
reader left outside the pre-`task_checkouts` fallbacks in
`read_task_branches`/`read_task_worktree_ownership` (store/sqlite.rs); the
remaining mentions are pre-migration fixtures.

`session_moves` (an agent switch at the Stop hook) and
`relocated_task_processes` (a repository directory moved on disk) stay: the
brief listed them with Machine moves, but neither crosses Machines.

Left for gate, collected from the passes above:
`two_people`, `transport_tests`, `session_lifecycle_tests`, `served_reads`,
`task_graph_launch_tests` (fixtures changed); `server::schema::tests::` (Clap
help for `session send`); `documented_commands::` (machines.md, edited again this pass); a nextest run
that answers the parallel-stall question below.

## QA (2026-11-13)

Report: `scratch/qa.html`. Fixed: `lf task list` and `lf machine list` read a
move in flight (`Moving to mini since …: sent, not arrived`) before ownership,
which names mini from the claim on.

## Done (2026-11-13, ninth pass): QA's two defects

- Retry on the target: `lf session receive --check --from <source>` records a
  `checked` `task_moves` row on mini; arrival turns it `arrived`.
  `session_move::sender` reads it first when it is mini's latest record, so
  `lf task move X --machine <mini>` run on mini asks the source and delivers.
  Test `machine_commands::a_move_retried_on_its_target_asks_the_machine_it_comes_from`
  (run with mini's own `lf`, laptop added on mini). machines.md: mini needs the
  source added.
- Not fixed: `lf --machine mini task move X --machine mini` forwarded from the
  laptop carries the laptop's personal settings (`config::remote::PROFILE_ENV`),
  whose Machine list replaces mini's and never names the laptop. That run now
  fails with `X is moving here from <laptop>, which did not send it; the same
  move run there carries it on` instead of `already runs on`; machines.md says
  so. QA's exact command therefore still does not deliver. Open decision:
  should a forwarded run use the target's own Machine routes (merge or prefer
  mini's list over `LF_PERSONAL_PROFILE`'s), or should the laptop run a
  forwarded retry of a move it sent itself, locally?
- Ship log bounded: `restarted` stops at `SHIP_RESTARTS_COUNTED` (6, where
  `repair_backoff` stops growing); a step completing deletes them (`progressed`
  is gone); a new `carried` row replaces the last. `ship_log_in` reads from the
  latest `approved`/`stopped` (a subquery searching `task_ship_events_task`). Backfill
  caps at 6. Test `a_ship_process_failing_all_day_keeps_its_log_short`
  (48 restarts → 8 rows) and `query_plans::auto_ship_log_reads_and_writes_plan_by_index`;
  MEASURED gains `task_ship_events` rows from the Machine store's 396 backfilled rows.
- Checks: integration `machine_commands::` 54, `documented_commands::` 4; lib
  `store::migrations::` + `task_moves::` + `task_work::` + `ops::task::ship::` +
  `lf::tests::` 194, `store::query_plans::` 9, `server::schema::tests::` 7;
  clippy `--all-targets -D warnings` clean.
- Compress: lists share `task_ownership::move_line`; the partial index
  `task_ship_events_start` went (a Task's log is a few rows). lib `query_plans::`
  + `ship` 102 ok, integration `machine_commands::a_move_` 3 ok, clippy clean.

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
  carrier and restart count joined the log as `carried`/`restarted`.
- **Primary Session not on `task_checkouts`.** Directed there, but
  `ensure_task` opens a Task conversation in the repository when the Task has
  no checkout, and a `task_checkouts` row means a checkout here (`worktree NOT
  NULL`; `unstarted` reads its absence). A Machine-local table keeps both.
- **Names.** `task_checkouts`, `task_moves` (`sent`/`arrived`),
  `task_ship_events`, `people`, `pool_machines`. `leaving` stays a third
  `task_moves` state beside the brief's "sent or arrived";
  the payloadless-`sent` alternative is in `294de82873:scratch/concept-review.html`.
