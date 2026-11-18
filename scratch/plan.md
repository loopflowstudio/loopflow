# LOO-937: Store each Task fact once, with shared facts apart from one Machine's

## Outcome

Each fact about a Task has one home, and nothing reads a second copy. A store
test fails the moment code reads or writes a member marked leaving, so a
duplicate can't quietly come back. A Process and a Session record their Task
when they start; membership reads that column instead of matching paths, and
`relocated_task_processes` with its two-sided queries goes.

What people notice:
- A Task's Processes and Sessions stay with it when its repository moves on
  disk, and when a checkout path is later reused by another clone. Today that
  depends on a frozen-pair table and a `sqlite_master` probe on every read.
- `lf task status` and the Desktop Task list show the same title, URL and
  order on every surface, because there is one stored value per fact.
- Dragging a Task tile in Desktop's Working board changes its rank, the same
  order Linear and `lf task` show. Today the board keeps a separate order
  that nothing else sees.

## What the live store says (2026-11-17, `~/.lf/loopflow.db`, read-only)

1,391 Tasks, 1,286 `pm_items`, 89 tables, 736 columns, `tasks` 42 columns,
1.02 GB.

| Copy | Disagreements | What it is |
|---|---|---|
| `tasks.primary_session_id` vs `task_primary_sessions` | 78 | old column, leaving (LOO-935 marked it) |
| `tasks` checkout columns vs `task_checkouts` | 55 (brief) | old columns, leaving |
| `tasks.issue_title` vs `pm_items.name` | 4 | **not a duplicate**: desired vs observed |
| `tasks.planning_state` vs `pm_items.state` | 15 | same: desired vs observed |
| `tasks.planning_rank` vs `pm_items.rank` | 605 | same, and provider rank only seeds new Tasks |
| `tasks.planning_url/branch_name/team_id` vs `pm_items` | 13 / 19 / 13 | desired vs observed, like title: Git planning writes the branch itself |
| `tasks.planning_provider_revision` vs `pm_items.revision` | 421 | **true duplicate**: observation bookkeeping |
| `tasks.issue_title` vs `task_linear_observations.last_title` | 14 | change detector copy |
| `tasks.issue_title` vs `task_creation_intents.title` | 68 | idempotency-check copy |
| `projects.project_name` vs `pm_projects.name` | 4 | desired vs observed (same as Tasks) |
| `task_primary_sessions` row vs its Session's `task_id` | 3 | two homes for "which Task" |
| `agent_sessions` with no `task_id`, cwd inside a checkout | 2,659 | membership only by path |
| `processes` inside a checkout (by path) | 156,651 of 187,600 | membership only by path |

## The finding that reshapes the brief

`tasks` vs `pm_items` (and `projects` vs `pm_projects`) is not one fact stored
twice. `tasks` holds the value lf shows and edits (desired); `pm_items` holds
the last thing the provider said (observed), and it is the merge base:
`field_baseline` reads `pm_items.name` into `tracker_actions.base_json`
(`sqlite/tracker_actions.rs:512-619`), and `planning/rules.rs:112-200` decides
confirmed/conflicted from the three values. Collapsing them would break the
conflict answer LOO-925 (#1957) just shipped. So both stay, with names that say
which is which, and what gets removed is the copies inside each side:

- One rule decides what `tasks` holds: every Task fact a person sees on any
  Machine (title, description, state, rank, URL, branch, team). `pm_items`
  holds what the provider last said about each of them. What leaves `tasks` is
  bookkeeping about the observation itself: `planning_provider_revision`,
  `pm_snapshot_synced_at`, and the old `planning_completed` boolean.
  Counterexample that set the rule: `record_task_branch_in`
  (`children.rs:154-180`) writes `planning_branch_name` when no provider is
  connected, because "Git planning carries the Task's branch, so whoever runs
  it next checks out the same branch and finds the same PR". 4 live Tasks hold
  a branch with no `pm_items` row, and `task_issue_branch` feeds every new
  checkout (`ops/task.rs:1045`). URL, branch and team keep travelling with the
  peer exchange.
- The observed side keeps one representation of observed order: `pm_items.rank`
  within the Project. `pm_projects.task_order_json` is derivable from it
  (`linear_acceptance.rs:470-500` only accepts a list whose every item already
  matches `pm_items`), so it goes.
- `task_linear_observations.last_title/last_description/last_revision` are a
  third copy used only to decide whether to steer the agent with a new
  definition (`children.rs:1100-1116`). `pm_items` already holds the previous
  observation; the steer compares against it before `put_item` overwrites it.
- `task_creation_intents.title/description` are read only to check that a
  repeated create request matches (`children.rs:62-90`). `creation_receipts`
  already keeps a request fingerprint; the check uses it.

## Inventory (reviewed before code changes)

Kept, merged or removed, and why. "Leaving" means: stop reading and writing
now, mark `-- leaving: <member> LOO-877` (or the Task named), drop after the
30-day window (`scripts/check_migrations.py` `STAGING_DAYS`).

### `tasks`: what a Task is (shared, no Machine facts)

| Column | Verdict | Why |
|---|---|---|
| id, repository_id, project_id, issue_identifier, external_issue_id, created_at, updated_at | keep | identity, binding to the provider (`external_issue_id` is how `pm_items` joins) |
| issue_title, issue_description, planning_state, planning_completed_at, planning_due_date, planning_assignee, planning_rank, planning_deleted_at | keep | desired values; edits and the queue write them (`planning_write.rs:86-96`) |
| planning_revision | keep | the peer exchange's clock (`planning_peers.rs`) |
| workspace_slug, abandon_reason, abandoned_at, completion_request, completion_error, paused, shipped_at | keep | Task lifecycle |
| planning_url, planning_branch_name, planning_team_id | keep | the Task's URL, branch and team, shown and carried to peers on every Machine; the branch is written by lf under Git planning (`record_task_branch_in`) |
| planning_provider_revision | **leaving** | the provider's clock for one observation = `pm_items.revision` |
| pm_snapshot_synced_at | **leaving** | when the provider was last read = `pm_items.observed_at` |
| planning_completed | **leaving** | old boolean; `planning_state` is set on every row (0 rows rely on the fallback in `TASK_COLUMNS`) |
| worktree, branch, base_commit, parent_pr_id, checkout_machine_id, primary_session_id, ship_* (6) | already leaving (LOO-935) | finish the reads: `sqlite/processes.rs:350` prune check still reads `tasks.ship_lf_process_id`; `sqlite.rs:379-410` keeps a pre-`task_checkouts` fallback path for old registries — delete it |
| regressed_in | already leaving (LOO-730) | untouched |

Renaming `issue_*`/`planning_*` to the product's words is a table rebuild that
keeps every column; it belongs with LOO-939 (naming), not here. Noted in
`scratch/questions.md`.

### Provider side (shared observed copy)

| Member | Verdict | Why |
|---|---|---|
| `pm_items` | keep | observed Task, merge base. Gains nothing. |
| `pm_projects` (minus `task_order_json`) | keep | observed Project, merge base |
| `pm_projects.task_order_json` | **leaving** | observed order = `pm_items` of the Project by `rank`; `observed_order()` (`planning_order.rs:47`) derives it |
| `projects` provider-only columns (`planning_provider_revision`, `planning_initiatives`, `planning_teams`, `pm_snapshot_synced_at`) | check in slice 3 | same rule as Tasks: a Project fact people see on any Machine stays; bookkeeping about one observation leaves |
| `task_linear_observations.last_title/last_description/last_revision` | **leaving** | change detector reads the prior `pm_items` row |
| `task_linear_observations.last_success_at/degraded_reason/updated_at` | keep | observation health per Task; no other home |
| `task_creation_intents.title/description` | **leaving** | repeat-check uses `creation_receipts.fingerprint`. The table's export columns and the table itself are LOO-938's (one queue) |

### Order

| Member | Verdict | Why |
|---|---|---|
| `tasks.planning_rank` | keep: the one order | position within a Project or among standalone Tasks; syncs as an `order` action |
| `task_orders` | **leaving** | Desktop Working-board order, repository-wide, never synced. Board sorts by Project rank then Task rank; a tile Move calls `reorder_in` |
| `pm_projects.task_order_json` | **leaving** | see above |
| `pm_items.rank` | keep | observed rank |

### Execution (keyed by Machine)

| Member | Verdict | Why |
|---|---|---|
| `task_checkouts`, `task_primary_sessions`, `task_ship_events` | keep | LOO-874's homes |
| `task_prs.branch/base_commit/parent_pr_id` | keep for now | trigger-copied from `task_checkouts` and enforced equal (`task_machine_local.sql:105-140`); PR delivery reads them for merged PRs too, where the checkout is gone. Not a free duplicate; questions.md |
| `pr_landings.worktree` | keep | a landing is supervised on one Machine (`supervisor_machine_id`) and may have no Task (`lf land` in any checkout). Execution table already |
| `processes.task_id` | **add** | the Task a Process belongs to, written at insert |
| `agent_sessions.task_id` | keep, backfill | becomes the only Session→Task link |
| `relocated_task_processes` | **leaving** | membership no longer depends on paths |

## Source of truth after the change

- Task (shared): `tasks`. Observed provider Task: `pm_items`. Queue: `tracker_actions` (LOO-925/938).
- Checkout here: `task_checkouts`. Primary conversation here: `task_primary_sessions`. Auto-Ship: `task_ship_events`.
- A Process's Task: `processes.task_id`. A Session's Task: `agent_sessions.task_id`.
- Order: `tasks.planning_rank` (desired), `pm_items.rank` (observed).

## Process and Session membership

One rule, applied once, when the row is written. A Process's Task is, in
order:

1. the Task the launch names: a Graph (`record_graph_process`), a Session the
   Process creates with a Task (`insert_session_in`) or attaches to
   (`attach_in`) — written on the Process's row by that call;
2. the deepest `task_checkouts` row containing `cwd`;
3. its caller's: the caller agent Process's `task_id`, else the caller
   Session's `task_id` (`caller_session_id`, non-primary);
4. its parent Process's `task_id` (`parent_lf_process_id`);
5. else NULL.

The checkout outranks inheritance (built, changed from the kickoff order). On
the 187,600-row copy, caller/parent-first gave 6,687 Processes that ran inside
one Task's checkout to the Task of the agent that ran them: 5,672 `lf session
serve`, plus `commit`, `pr publish`, `pr land`, `lf --task LOO-921 ship` from
LOO-959's agent. Those are the checkout's Task's work. Inheritance covers what
runs outside any checkout (repo root, `/tmp`).

Measured before deleting the Session-event unions: 386 of 22,378 event-linked
(Task, Process) pairs were reached by no rule; nearly all the lf Process that
captured or started a Task's Session from outside its checkout (`lf session
resume --task EXP-49` from the repo root). Rule 1's Session writes cover those
going forward; the backfill reads `session_events` `captured`/`started` for
old rows. The 166 `observed`-only pairs (a Process that only wrote an
observation into a Session) are no longer its Task's work.

An agent Process copies `agent_sessions.task_id`. A Session's Task is the
named Task, else the deepest checkout containing its cwd at creation.

Writers: `record_process_in` (`sqlite.rs:1578`) and the agent insert
(`sqlite/processes.rs`) are the only production inserts; both resolve
`task_id` in the insert statement. Rules 2 and 3 are primary-key probes; rule 4
is the ancestor probe `session_tasks` already uses (`task_work.rs:89`).

Readers collapse to the column. `lf_process_ids` today unions path matching,
relocation receipts, and four Session-derived sets (`session_events`,
`history.session_history`, `history.turns`, agent attachments); it becomes
`SELECT id FROM processes WHERE task_id=?` plus the agent rows of the Task's
Sessions. `task_of_process` becomes a primary-key read. `session_membership`
reads `task_id` only. `has_relocations`, `relocated_process_membership`, the
freeze in `relocate_repository` (`repositories.rs:136`), the fixture writer in
`read_stream/events.rs:362-376`, and the `relocated_task_processes` and
`tasks.ship_lf_process_id` clauses of `FORGETTABLE` (`processes.rs:347-350`)
go.

Backfill: `agent_sessions.task_id` (2,659 rows) in the migration, with the
reconciliation rule below. Processes do not fit there. Measured on a
`VACUUM INTO` copy of the live store (2026-11-17): the deepest-checkout probe,
materialized into a temp table and applied with `UPDATE … FROM`, resolved
156,708 of 187,600 Processes in 28.6 s wall, 4.5 s CPU; a correlated-CTE
version ran over 10 minutes. Holding the shared write lock for 30 s at upgrade
stalls every agent, so the migration only adds the column, and the server's
maintenance pass applies the same five rules to `task_id IS NULL` rows in
batches of 5,000 ids per write transaction (oldest first, relocation receipts
applied first while the table exists), beside pruning. Rows the prune deletes
anyway are skipped.

No compat trigger. Released `lf` keeps inserting Processes without `task_id`
during the window; the same maintenance pass picks them up. (A trigger could
not reuse the probe anyway: SQLite rejects a `WITH` clause inside
`CREATE TRIGGER`.) Until a pass runs, such a Process is missing from its Task's
list; on a Machine running this build, only Sessions still held by an old
binary produce them.

A Process launched outside lf has no row, so path matching has no remaining
caller.

Reconciliation rule for the 3 primary Sessions whose `task_id` disagrees:
`task_primary_sessions` names the Task; the migration sets the Session's
`task_id` to it.

## A duplicate can't come back

`open_ephemeral` under `cfg(test)` installs an SQLite authorizer
(rusqlite `hooks` feature) that denies `SQLITE_READ`/`SQLITE_UPDATE` on every
leaving column and any access to a leaving table. `build.rs` already orders
migrations; it also emits the leaving list from `-- leaving:` marks. Every
store test then fails with the member's name if production SQL touches it.
It is installed after migrations apply, and it allows any access whose
accessor is a trigger: `store_revision_tasks_update`,
`task_update_tasks_update` and `cancel_task_retirement` still name leaving
columns, and denying them would fail every Task update rather than catch a
reader. Migration-replay and old-release fixture tests open through a path
without the authorizer. The leaving list is read from the `-- leaving:` marks
`scripts/due_removals.py` already parses, emitted by `build.rs`. Break it in your head: re-add `t.planning_url` to `TASK_COLUMNS`
and every Task read test fails.

## Acceptance check at gate

```bash
# membership survives a move, with no relocation table
cargo test -p loopflow-storage --lib sqlite::repositories::   # relocate keeps Processes and Sessions with their Task
cargo test -p loopflow-storage --lib task_work::              # membership by task_id
# the guard
cargo test -p loopflow-storage --lib leaving_members_are_unreadable
# against a copy of the live store
cp ~/.lf/loopflow.db /tmp/lf937.db && LF_HOME=… lf task status LOO-937 --json   # same title/url/sessions/processes as released lf
sqlite3 /tmp/lf937.db "select count(*) from processes where task_id is null and cwd in checkout"  # 0
```

Report before/after in the PR: tables, columns, store size, `pub fn` count in
`loopflow-storage` (900 now), SQL call sites outside it (`rg` count, method
stated).

## Absent and error states

- Task with no provider binding: no `pm_items` row; URL and team absent, the
  branch present when lf recorded it under Git planning (as today).
- Process in no checkout and no named Task: `task_id` NULL, same as today's
  non-match.
- Checkout removed, then path reused: membership unchanged, which today needs
  the frozen pairs.

## Operational boundary

One write lock for hundreds of agents: the process insert gains one indexed
ancestor probe (depth ≤ path components), after two primary-key probes. Check with `EXPLAIN QUERY PLAN` on
the 187k-row store; membership queries lose the `sqlite_master` probe and the
anti-join. New index `processes(task_id)` partial `WHERE task_id IS NOT NULL`.

## Exclusions

- The outgoing queues and `task_creation_intents` as a table: LOO-938.
- Column and table renames to product words: LOO-939.
- Moving `processes` out of the main store: not this Task (LOO-936 moved
  Session history only).
- Dropping anything: LOO-877 and the final review Task (LOO-940) file the
  contracts.

## What goes

- `relocated_task_processes` reads/writes; `has_relocations`,
  `relocated_process_membership`, the freeze in `relocate_repository`
  (its test at `repositories.rs:379-430` stays, asserting membership instead
  of frozen pairs), `events.rs:362-376` fixture inserts (test-only; the brief's
  "second path"), `processes.rs:347` and `:350` prune clauses.
- `sqlite.rs:379-410` pre-`task_checkouts` registry fallback.
- Reads/writes of `tasks.planning_provider_revision/planning_completed/
  pm_snapshot_synced_at`.
- `task_orders` and `commit_task_order`/`task_order_inputs`/`task_order_in`,
  `TaskOrder` key's manual position.
- `pm_projects.task_order_json` reads/writes.
- `task_linear_observations` title/description/revision columns and
  `seed_task_linear_observation`'s copy of them.
- `task_creation_intents.title/description` writes and the comparison.

Survives: every Task's title, description, state, rank, URL, branch,
every Process/Session association, conflict answers.

## Slices

1. **Process and Session record their Task.** *Process half built
   (`process_task.sql`); Session half next.* Built: `processes.task_id` +
   `processes_task`, relocation receipts copied in and the table marked
   leaving, the freeze in `relocate_repository` and `has_relocations` gone,
   `lf_process_ids`/`task_open_process_pairs`/`task_of_process`/performed-work
   read the column (plus agent rows of the Task's Sessions, which can be bound
   after they start), the prune clause for relocations gone. The backfill is
   `assign_process_tasks` in `retention::prune` behind a one-row cursor table
   `process_task_backfill` (a NULL row is examined once; drops with LOO-877
   once no released lf writes rows). On the copy: 161,718 of 187,600 assigned,
   38 batches, 10.8 s total, about 0.28 s write lock per batch (debug build);
   149,984 agree with path matching, 6,687 differ only by the precedence fix
   above being absent (now 0 by construction), 5,047 newly reached by
   inheritance. Insert-time probe is index seeks
   (`recording_and_reading_a_process_task_probe_indexes`).
   Session half built: `reserve_session_in` records the deepest checkout's
   Task on a Session that names no Task or Wave (`work_source='checkout'`,
   the Wave follows the Task), the same rule `lf <skill>` already used via
   `select_work`. Readers checked first: `session_task_ids` (prompt context,
   task updates, agent launch, moves) already counted checkout membership, so
   they see the same Task; LaunchKind keys on `bound`/primary, so a checkout
   Session stays a chat; `capture_in` now stamps the Task's Wave on the
   captured event, which `session list --wave` already counted by path. What
   the live store gains is helpers (1,563 "Generate PR title", ~145 commit
   messages, 715 `sync-conflicts`) recording the Task whose checkout they ran
   in, which path matching already gave them. `process_task.sql` backfills:
   the 3 primaries from `task_primary_sessions`, then the deepest checkout for
   every Session naming nothing; on a copy of the live 22,342 Sessions that is
   0.19 s and leaves 0 path-only members. `session_membership`,
   `session_tasks` and `agent_task` read `task_id` only; `checkout()` and
   `TASK_CHECKOUTS` are gone. No path matching is left in membership.
   Demo: `work_keeps_its_task_when_a_checkout_path_is_reused` (Processes and
   Sessions stay with the first Task when a second clone takes the path).
   Learned: recording at insert alone misses adoption, where `place_task`
   gives a Task a checkout a taskless Graph is already running in (the Process
   half had broken `adopting_a_checkout_with_a_running_graph_prevents_another_run`).
   `write_task_checkout_in` now gives unfinished taskless Processes and open
   taskless Sessions inside the new checkout its Task, once, by the
   `processes_unfinished`/`open_sessions` indexes. A Session in a nested
   checkout now belongs to the deepest Task only (it used to count for every
   enclosing one too), and a Task keeps its Sessions after its checkout goes.
   Left for gate: `session_lifecycle_tests`, `two_people`, `performance_tests::`,
   `session_cli_tests::` and the `loopflow` crate's fixtures, which insert
   `agent_sessions` rows by SQL; any that relied on a path match need a
   `task_id`.
2. **The guard.** Authorizer in test stores, leaving list from `build.rs`.
   Fix the reads it catches (the `ship_lf_process_id` prune clause, the
   registry fallback). Every later slice lands under it.
3. **Observation bookkeeping lives in `pm_items`.** Provider revision,
   synced-at, `planning_completed` leave `tasks`; same check for `projects`.
   The observation steer compares against `pm_items`; intents' title copy
   goes.
4. **One order.** Working board sorts by rank; tile Move calls `reorder_in`;
   `task_orders` and `task_order_json` leave. Swift Working board test
   updated.

## Docs (written ahead of the code)

`swift/README.md` (Working order, conversation membership, sidebar drag),
`docs/surfaces.md` (drag changes rank), `docs/dev/reference-code.md` and
`docs/dev/reference.md` (`processes.task_id`; `relocated_task_processes` and
`task_orders` gone). `rust/loopflow-storage/SCHEMA.md` regenerates from the
migration (`scripts/schema_page.py`) once slice 1 writes it.

Open: where do Tasks with no Project sit in the Working sidebar, before every
Project, after, or by their own rank interleaved? The README says "Tasks follow
their Project's rank, then their own" and leaves it unsaid.
