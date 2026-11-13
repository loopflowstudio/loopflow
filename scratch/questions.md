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

## Remaining

1. **`task_ship_events`**: replace the seven `ship_*` columns with an approval/stop
   log (who, when, process); the move carries the latest approval.
2. **`pool_machines`**: move `machines.pool`, `cost_usd`, `expires_at`.
3. **`people`**: one row per person, keyed by planning-provider identity.
4. `relocated_task_processes` not folded: relocation keeps Process `cwd` as
   history, so the receipt is what keeps their membership. Folding needs a
   path-history column on `task_checkouts`; open whether that is worth it.

## Open questions (each reversible; the simpler option is built)

- **What the move record replaces.** `session_moves` is the queue
  `lf session move -a` leaves for a busy Session's Stop hook (another agent, not
  another Machine) and stays. `agent_sessions.moved_to_machine_id` also marks
  Session-only moves (`lf --machine M resume <session>`), which `task_moves`
  (keyed by Task) can't hold, so it stays for now. Does a Session-only move get
  its own record, or does the marker stay?
- **Parallel stalls.** Whether the stalls of those three groups under the
  default thread count predate this branch is unchecked; gate's nextest run
  answers it.
- **Version skew.** A released lf still running after install reads a move's
  claim (written here, naming mini) as its own claim gone missing and may claim
  again; released lf writes entries with no `writer_machine_id`, which this
  build reads as written elsewhere. Window is live pre-install processes only.
- **Writer field optional on the wire.** Markers released lf delivered lack
  `writer_machine_id`, so it deserializes absent as null; Task 538340 makes it required.
- **People without a planning provider.** Under the Git journal there is no
  provider user; what keys that person, given no new Git-email keys?
- **Auto-Ship across a move.** Docs say the move carries the latest approval.
- **Names.** `task_checkouts`, `task_moves` (`sent`/`arrived`),
  `task_ship_events`, `people`, `pool_machines`.
