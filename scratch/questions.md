# LOO-874 plan and open questions

Docs written for this Task: `docs/dev/machines.md` (Own Tasks, checkout paragraph,
move Failures, Auto-Ship, pools), `docs/dev/reference-processes.md`,
`docs/dev/reference-models.md`, `docs/dev/reference.md`, `docs/dev/reference-code.md`.
Migration: `rust/loopflow/src/store/migrations/task_machine_local.sql` (edit in place).
Contract Task: 538340 ("Drop the Task, ownership and Machine columns LOO-874
replaced"; Linear ID pending sync). It drops only what the merged migration replaced.

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

## Remaining

1. **`task_checkouts`** (largest cut): move `tasks.worktree`, `branch`,
   `base_commit`, `parent_pr_id`, `checkout_machine_id` readers/writers to a
   Machine-local row. ~30 files name `checkout_machine_id`; `store/sqlite/task_work.rs`,
   `children.rs`, `repositories.rs`, `task_retirement.rs` hold the SQL. Triggers
   to carry over: `task_placement_to_pr` (branch/base/parent → `task_prs`),
   `task_update_tasks_update` (worktree/machine → `task_updates`), unique worktree
   index. Then `relocated_task_processes` folds in (path updated in place).
   Acceptance: `lf --machine mini task status X` on a Machine that never had X
   prints `Not checked out on mini` and no path.
2. **`task_ship_events`**: replace the seven `ship_*` columns with an approval/stop
   log (who, when, process); the move carries the latest approval.
3. **`pool_machines`**: move `machines.pool`, `cost_usd`, `expires_at`.
4. **`people`**: one row per person, keyed by planning-provider identity.
5. EXPLAIN QUERY PLAN for new joins against a ~10k-Task store.

## Open questions (each reversible; the simpler option is built)

- **What the move record replaces.** `session_moves` is the queue
  `lf session move -a` leaves for a busy Session's Stop hook (another agent, not
  another Machine) and stays. `agent_sessions.moved_to_machine_id` also marks
  Session-only moves (`lf --machine M resume <session>`), which `task_moves`
  (keyed by Task) can't hold, so it stays for now. Does a Session-only move get
  its own record, or does the marker stay?
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
