# LOO-937: Store each Task fact once, with shared facts apart from one Machine's

## Outcome

Each fact about a Task has one home, and nothing reads a second copy. A store
test fails the moment code reads or writes a member marked leaving, so a
duplicate can't quietly come back. A Task's work is its Sessions and its Graph
runs; each records its Task when it starts, and membership reads that column
instead of matching paths. (`processes` and `relocated_task_processes` left
with #1974; LOO-965 drops them.)

What people notice:
- A Task's Sessions and Graph runs stay with it when its repository moves on
  disk, and when a checkout path is later reused by another clone.
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
| `tasks.issue_title` vs `task_linear_observations.last_title` | 14 | **not a copy** (slice 3): the definition the agent was last steered with |
| `tasks.issue_title` vs `task_creation_intents.title` | 68 | **not a copy** (slice 3): the original create request, for callers with no receipt |
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
  bookkeeping about the observation itself: `planning_provider_revision`
  and the old `planning_completed` boolean. (`pm_snapshot_synced_at` stays:
  slice 3 found it is the stamp of the accepted observation, see below.)
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
- `task_linear_observations.last_title/last_description/last_revision` and
  `task_creation_intents.title/description` looked like copies and are not
  (slice 3, below): both stay.

## Inventory (reviewed before code changes)

Kept, merged or removed, and why. "Leaving" means: stop reading and writing
now, mark `-- leaving: <member> LOO-877` (or the Task named), drop after the
30-day window (`scripts/check_migrations.py` `STAGING_DAYS`).

### `tasks`: what a Task is (shared, no Machine facts)

| Column | Verdict | Why |
|---|---|---|
| id, repository_id, project_id, issue_identifier, external_issue_id, created_at, updated_at | keep | identity, binding to the provider (`external_issue_id` is how `pm_items` joins) |
| issue_title, issue_description, planning_state, planning_completed_at, planning_due_date, planning_assignee, planning_rank, planning_deleted_at | keep | desired values; edits and the queue write them (`planning_write.rs:86-96`) |
| planning_revision | keep | the Task's edit revision: `edit_task` refuses a stale one (`task_content.rs` `validate_edit`) |
| workspace_slug, abandon_reason, abandoned_at, completion_request, completion_error, paused, shipped_at | keep | Task lifecycle |
| planning_url, planning_branch_name, planning_team_id | keep | the Task's URL, branch and team, shown and carried to peers on every Machine; the branch is written by lf under Git planning (`record_task_branch_in`) |
| planning_provider_revision | **leaving** | the provider's clock for one observation = `pm_items.revision` |
| pm_snapshot_synced_at | keep | when the observation lf accepted into the Task was read; `tracker_items.observed_at` moves on reads acceptance skips (needs_refresh, unaccepted Project). Display only (`tracker_synced_at`) |
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
| `projects.planning_provider_revision` | **leaving** | write-only; `tracker_projects.revision` |
| `projects.planning_initiatives/planning_teams` | keep | exchanged Project facts (`WorkValue::ProjectInitiatives/Teams`) |
| `projects.pm_snapshot_synced_at` | keep | readiness reads it as the accepted observation: `IS NOT tracker_projects.observed_at` makes the Project unavailable (`project_selection.rs` `readiness_in`) |
| `task_linear_observations.last_title/last_description/last_revision` | keep | the steer ledger: what the agent was last told. `ingest_task` writes `tracker_items` before `apply_tracker_event` runs (`ops/tracker_events.rs`), and inventory reads rewrite it without steering, so the prior `tracker_items` row is already the new title |
| `task_linear_observations.last_success_at/degraded_reason/updated_at` | keep | observation health per Task; no other home |
| `task_creation_intents.title/description` | keep (LOO-938) | the repeat-check's original request; `creation_receipts` exists only for `lf task create`, while `create_task` has ~10 receiptless callers (harness task updates, native sessions, runtime, tracker drain). The table is LOO-938's (one queue) |

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
| `task_checkouts`, `task_ship_events` | keep | LOO-874's homes |
| `task_primary_sessions` | keep until LOO-877 | the Task primary's one home, read and written by this build and released lf alike; read through `primary_scope_sql(s)`. LOO-877 moves it onto `agent_sessions.primary_scope='task'` with its writers |
| `task_prs.branch/base_commit/parent_pr_id` | keep for now | trigger-copied from `task_checkouts` and enforced equal (`task_machine_local.sql:105-140`); PR delivery reads them for merged PRs too, where the checkout is gone. Not a free duplicate; questions.md |
| `pr_landings.worktree` | keep | a landing is supervised on one Machine (`supervisor_machine_id`) and may have no Task (`lf land` in any checkout). Execution table already |
| `agent_sessions.task_id` | keep, backfill | the only Session→Task link |
| `graph_processes.task_id` | keep, backfill | the only Graph run→Task link; fixed at launch |
| `processes`, `relocated_task_processes` | leaving (LOO-965, #1974) | not this Task's |

## Source of truth after the change

- Task (shared): `tasks`. Observed provider Task: `pm_items`. Queue: `tracker_actions` (LOO-925/938).
- Checkout here: `task_checkouts`. Primary conversation: `task_primary_sessions` (LOO-877 moves it). Auto-Ship: `task_ship_events`.
- A Session's Task: `agent_sessions.task_id`. A Graph run's Task: `graph_processes.task_id`.
- Order: `tasks.planning_rank` (desired), `pm_items.rank` (observed).

## Session and Graph run membership

One rule in the store (`task_work::starting_task_in`), applied once when the
row is written. The launch decides first and names the Task it found
(`select_execution_work` → `task_checkout_owner`); the store
fills in only what the launch left empty. Both ask the same store answer, so
the launch and the store agree on nested worktrees and shared roots.

> the Task it names, else the Task whose checkout is its directory's Git
> worktree on this Machine, else the Task of the Session that started it.

Checkout before caller: a command run inside a checkout is that Task's work
whoever ran it (live store, 2026-11-17: the caller-first order gave 6,687
Processes to the calling agent's Task; 5,672 `lf session serve`, plus `commit`,
`pr publish`, `pr land`, `lf --task LOO-921 ship` from LOO-959's agent).

- `record_graph` applies all three clauses (`ExecutionStart.caller_session`).
- Session creation (`resolve_ancestry_in`) applies the first two; the Task of
  the agent that started it arrives named, as `work_source='declared'`,
  through `LF_AS`, which the launch reads after the checkout
  (`select_execution_work`). `'inherited'` has no production writer.
- No adoption: placing a checkout gives work already inside it no Task
  (concept review 5c). A Session joins by starting there, or by `lf session
  bind` (as primary). A taskless Graph run already there stays out of the
  Task's history and still blocks a second run (`task_open_graph_pairs`).
- Readers: `session_membership`, `session_tasks` and `task_graph_pairs` read
  `task_id` only. Three path matches remain, all about what occupies a
  directory now: `task_open_graph_pairs` for unended runs that recorded no
  Task, `checkout_open_work` for a checkout no Task holds, and
  `task_checkout_owner` (by Git root) for a launch and for provider conversations lf has no
  Session for (`human_session/turns.rs`).
- Backfill in `task_facts_once.sql`: the 3 primaries from
  `task_primary_sessions`, the deepest checkout for Sessions naming nothing
  (22,342 live Sessions: 0.19 s), then Graph runs with no Task by the rule.

## A duplicate can't come back

The storage crate's test stores (`open_ephemeral_prepared` under
`cfg(test)`) install an SQLite authorizer (`leaving_guard.rs`) after
migrations apply. It denies reads and updates of every leaving column, and
any access to a leaving table, printing `store SQL names leaving member
<member>`. The list comes from the `-- leaving:` marks in the migrations the
binary already embeds; no `build.rs` step. It lets through what SQLite does
on its own, which reaches the authorizer looking like top-level SQL: trigger
bodies (accessor set), reads of foreign-key columns (key checks when a parent
row changes) and deletes from tables a parent cascades into. So a direct read
of a leaving FK column (`tasks.ship_lf_process_id`, `tasks.parent_pr_id`,
`tasks.checkout_machine_id`, `tasks.primary_session_id`, `machines.pool`)
or of an all-FK table (`relocated_task_processes`) is not caught.
`STILL_READ` lists marks whose own comment says production still reads them:
`agent_sessions.work_source` (LOO-836) and `tasks.regressed_in` (LOO-730).
Migration-replay and old-release fixtures open their own connections.
Checked: putting `t.worktree` back in `TASK_COLUMNS` fails 5 of 11
`task_work::` tests with `tasks.worktree`.

## Acceptance check at gate

```bash
cargo test -p loopflow-storage --lib task_work::              # membership by task_id, move and path reuse
cargo test -p loopflow-storage --lib leaving_members_are_unreadable
```

Never open a copy of the live store with `lf` (reads run recovery against the
real checkouts). QA applied the migration to a `VACUUM INTO` snapshot with
`sqlite3` and compared a private home built by the installed lf before and
after the branch opened it (`scratch/qa.html`, `.lf/qa/server.md`).

Report before/after in the PR: tables, columns, store size, `pub fn` count in
`loopflow-storage` (900 at kickoff), SQL call sites outside it (`rg` count,
method stated).

## Absent and error states

- Task with no provider binding: no `pm_items` row; URL and team absent, the
  branch present when lf recorded it under Git planning (as today).
- Session or Graph run in no checkout, naming no Task, started by no Task's
  Session: `task_id` NULL.
- Checkout removed, then path reused: membership unchanged, which today needs
  the frozen pairs.

## Operational boundary

One write lock for hundreds of agents: starting a Session or a Graph run adds
one indexed ancestor probe (depth ≤ path components) and, for a run, one
primary-key read of the calling Session.

## Exclusions

- The outgoing queues and `task_creation_intents` as a table: LOO-938.
- Column and table renames to product words: LOO-939.
- Moving `processes` out of the main store: not this Task (LOO-936 moved
  Session history only).
- Dropping anything: LOO-877 and the final review Task (LOO-940) file the
  contracts.

## What goes

- Path matching in Task membership: `checkout()`, `TASK_CHECKOUTS`, the
  path-or-id `session_membership`/`session_tasks`/`task_open_graph_pairs`.
- `sqlite.rs:379-410` pre-`task_checkouts` registry fallback.
- Reads/writes of `tasks.planning_provider_revision/planning_completed`,
  `projects.planning_provider_revision`.
- `task_orders` and `commit_task_order`/`task_order_inputs`/`task_order_in`,
  `TaskOrder` key's manual position.
- `pm_projects.task_order_json` reads/writes.
- `task_primary` on the Session wire record (Swift derives `taskPrimary` from
  `primary_scope`, which `primary_scope_sql` fills from `task_primary_sessions`).

Survives: every Task's title, description, state, rank, URL, branch,
every Process/Session association, conflict answers.

## Slices

1. **Sessions and Graph runs record their Task.** *Built
   (`task_facts_once.sql`, was `process_task.sql`).* The Process half built
   first (`processes.task_id`, its backfill pass and cursor table) is deleted:
   #1974 stopped writing `processes`. What stands: `starting_task_in` for
   Session creation and `record_graph`; readers on `task_id` only
   (adoption was built here, then removed by `f8a5ef1f1f`). A Session or run in a nested checkout belongs to
   the deepest Task only, and a Task keeps its work after its checkout goes.
   Demo: `work_keeps_its_task_when_its_repository_moves_and_its_path_is_reused`
   (a run and a conversation started in a checkout with no `--task`, the
   repository moved, the old path taken by a second clone; each Task lists
   exactly its own). Upgrade: `upgrade_records_the_task_each_session_belonged_to_by_path`,
   `upgrade_gives_graph_runs_that_named_no_task_theirs`.
2. **The guard.** *Built (`leaving_guard.rs`). `cargo nextest run -p
   loopflow-storage --lib`: 390 passed under it.* The first run failed 140
   tests; nearly all were SQLite's foreign-key checks and cascades (exempted,
   see above), the rest `work_source`/`regressed_in` (still read by design,
   `STILL_READ`) and three test-side readers. Production SQL it caught: none
   beyond the two planned. The `ship_lf_process_id` prune clause is gone;
   the migration nulled that column (190 live rows, every one already a
   `task_ship_events` row since task_machine_local; v0.16.0 writes it only in
   a test), so no Process stays pinned by it. The pre-`task_checkouts`
   registry fallback is gone from `read_task_branches` and
   `read_task_worktree_ownership`, with the two old-schema fixtures; one test
   seeds a released registry with `task_checkouts`. Guard only covers the
   storage crate's own tests: `test-support` stores in other crates stay
   unguarded so feature unification can't change the CLI. Every later slice
   lands under it: slice 3 marks its columns leaving and the Task tests say
   what still reads them.
3. **Observation bookkeeping.** *Built (`task_facts_once.sql`, commit
   "A Task is completed when its state says so"). `cargo nextest run -p
   loopflow-storage --lib`: 391 passed under the guard.* `tasks.planning_completed`,
   `tasks.planning_provider_revision` and `projects.planning_provider_revision`
   are leaving. Every read asks `planning_state` (live: `completed` exactly
   when the boolean is 1; the migration fills the state for any row relying
   on the old fallback). Released lf and the retirement triggers still read
   the boolean, so `tasks_completed_follows_state_{insert,update}` keep it
   equal to the state; a test-side write of the boolean alone is undone by
   them, which is why fixtures moved to `planning_state`
   (`saved_selection_work.sql` included). `Disposition` keeps its
   `planning_completed` key: the Git journal and released peers parse it;
   the writer stores a stateless completion as `completed`
   (`released_lf_sees_the_completion_a_state_decides`). Contradicted and
   kept: the steer ledger, the intent's title copy, both
   `pm_snapshot_synced_at` (inventory rows above say why).
4. **One order.** *Built (`task_facts_once.sql`, commit "One Task order").
   `cargo nextest run -p loopflow-storage --lib`: 391 passed under the guard,
   which now refuses `task_orders` and `tracker_projects.task_order_json`
   (`leaving_members_are_unreadable`). Swift `DashboardWorkspaceTests`: 15
   passed headless.* `task_orders`, `tracker_projects.task_order_json` and its
   released mirror `pm_projects.task_order_json` are leaving. The Working
   board and the roadmap sort by `TaskOrder::key`: Project rank, Project, Task
   rank, id; Tasks with no Project last (README says so). The "waiting first"
   tiebreak is gone. A tile Move is `SqliteStore::reorder_task(task, to)`:
   `planning_order::move_in` gives the Task `to`'s position among its siblings
   through `reorder_in`, so it is an `order` action for a tracker Project; a
   Move onto another Project's Task moves the Task into that Project at that
   place (a Project change, then an order;
   `a_task_move_changes_its_rank_in_every_plan_and_a_drop_in_another_project_joins_it`).
   Across repositories it is `InvalidData` with the reason, which `lf desktop`
   reports as an argument error and Desktop shows after re-reading
   (`aRefusedMoveReturnsTheRowAndSaysWhy`). The execution
   `reorder_task`, `commit_task_order`, `task_order_inputs`, `task_order_in`
   and the `task_orders` JSON validation are gone. `observed_order` reads the
   Project's `tracker_items` by rank; `observe_in` stops writing a list, and
   its omission check compares against that derived order.
   Learned: `tracker_projects` is the live table and `pm_projects` a
   two-way trigger mirror for released lf, so the plan's
   `pm_projects.task_order_json` is really `tracker_projects.task_order_json`.
   Released lf on the same store still writes its own list on each Linear
   observation and reads it back; between its observations the list it reads
   can be stale (questions.md). Also repaired:
   `task_concepts_share_lifecycle_activity_membership_and_order` had a phase
   setting `planning_state='completed'` with the boolean still 0, "a conflicting
   planning label"; slice 3 made that one fact, so the phase and its entry in
   `tests/fixtures/dto/task_concepts.json` are gone (Swift reads phases by name).

5. **A Task's primary: one home, `task_primary_sessions`.** *Built
   (`06c6145399`, compress `8cc3ba3f9c`); details under "Earlier" item 5.*
   This build and released lf read and write the same table, so nothing
   disagrees in the window. `primary_scope_sql(s)` derives the wire
   `primary_scope='task'`; nothing stores it. LOO-877 moves the mark onto
   `agent_sessions` with its writers.

## Docs

Match the code: `swift/README.md` (Working order, standalone Tasks last, drag
changes rank, conversation membership), `docs/surfaces.md`,
`docs/dev/reference.md`, `docs/dev/reference-models.md` (primary scope),
`rust/loopflow-storage/SCHEMA.md` (generated by `scripts/schema_page.py`).

## For LOO-877 (recorded on that Task, 2026-11-18)

Drop `task_primary_sessions` with its three mirror triggers and make
`agent_sessions_primary_task` unique; drop `task_orders`,
`tracker_projects.task_order_json`, `pm_projects.task_order_json`,
`tasks.planning_completed` with `tasks_completed_follows_state_{insert,update}`,
`tasks.planning_provider_revision`, `projects.planning_provider_revision`;
ship the release whose peer protocol accepts a `Disposition` without
`planning_completed`, then stop sending it.

## Built: `lf wt create --stack` (human review 2026-11-23)

Answers (Jack Heart): "a -- there should be a stack flag though", "and we can
instruct to use the stack flag as necessary", "put the stack flag in here and
the instructions as well".

1. **`lf wt create <name> --stack`.** Without it nothing changes: the new
   worktree's branch starts from main. With it, the branch starts from the
   branch of the checkout it is run in (the current Task's branch), and the
   new checkout records that branch as its base, the way a stacked Task
   follows its parent, so `lf sync` follows it. Help text in Clap; `--plan`
   shows the base it would use.
2. **Instructions.** `LOOPFLOW.md`'s child-agent paragraph: a child whose
   piece builds on this Task's work gets `lf wt create <name> --stack`; one
   doing unrelated work gets plain `lf wt create`. One sentence; the
   paragraph is paid for on every run.
3. Tests: `--stack` from a Task's checkout creates a branch whose base is the
   Task's branch head and holds its commits; without the flag the base is
   main. `documented_commands` passes with the new example.

Built. The record is Git config on the new branch,
`branch.<name>.loopflow-base`, beside `loopflow-persistent`: the new checkout
has no Task, so `task_checkouts.parent_pr_id` can't carry it, and a branch
upstream is overwritten by every `push -u`. `git::worktrees::stacked_base`
reads it (None once the base branch is gone, so the checkout follows main
again); `lf sync` with no target merges it when no Task stack applies, and
`pr_target` targets it. On the default branch or a detached HEAD, `--stack`
has nothing to stack on and creates from main. `checkout.create` carries
`stack` on the wire (Rust, Swift, fixture, schema). LOOPFLOW.md's child-agent
paragraph names `--stack`; `docs/ops.md` has the example; goldens
regenerated. Tests: `sync_tests::a_checkout_created_with_stack_builds_on_this_branch_and_sync_follows_it`
(plan shows the base, `--json` base, HEAD holds the parent's commits, plain
create starts at main, a later parent commit arrives through `lf sync`);
`server::schema::tests::`, `server::wire::`, `lf::commands::write::tests::`,
`machine::wire::`, `documented_commands::`, `golden_prompt` passed. Not run
here: clippy, Swift `ServerProtocolTests` (the change touched
`RegistryQuery.swift`) — gate runs both. Next: "one answer for a checkout's base" below, then gate.

## Built: one answer for a checkout's base (concept review on f07ab06545)

Choice made for review (page `scratch/concept-review.html`, question 1a):
"what this checkout builds on" has three homes and `lf sync` and
`lf pr open` read different subsets. Sync asks the Task stack, then
`loopflow-base`, then main; publish (`publication_base` → `pr_target`) asks
the same plus the branch's Git upstream when it names another branch
(`WorktreeState.base_branch`). The upstream clause is the earlier attempt
`--stack` replaces: lf's push (`push_with_upstream_if_needed`) runs a bare
`git push` once an upstream is set, so an upstream of `origin/parent` is
refused or pushes onto the parent. Passed over: teaching sync the upstream
too (keeps the broken push), and leaving two readers.

1. One function in `ops` (e.g. `checkout_base(repo, default_branch)`):
   the Task stack's live parent, else `stacked_base` unless its PR merged
   (`branch_pr_merged`), else the default branch. `run_sync_in`'s default
   target and `publication_base` both call it; `resolve_pr_target` keeps
   only the push-an-unpushed-base step.
2. Delete `pr_target`'s `list_worktrees` upstream clause,
   `WorktreeState.base_branch` and what fills it (`LocalBranch.upstream` if
   nothing else reads it), `worktree_list_preserves_namespaced_upstream_branch`
   and the two `base_branch` asserts in `worktrees.rs` tests.
3. Tests: `sync_tests::a_checkout_created_with_stack_builds_on_this_branch_and_sync_follows_it`
   and the stacked-Task sync/publish tests pass unchanged; `git::worktrees::`
   and `ops::pr::` focused. Then gate.

Built. `ops::checkout::checkout_base(repo, default)` returns
`CheckoutBase::Stacked(StackedSync)` or `CheckoutBase::Branch(name)` (the
`loopflow-base` branch until its PR merges, else default). The stack's branch
is read lazily (`branch()`, `sync_ref()`), so `lf sync main` still detaches a
child whose parent closed unmerged instead of erroring on the parent first.
`sync_ref` keeps `origin/<parent>` for a Task parent and the local branch
otherwise. Publication pushes an unpushed `Branch` base (`push_unpushed_base`)
and now falls back to the repository's default branch, not a literal `main`.
Deleted: `pr_target`, `resolve_pr_target`, `WorktreeState.base_branch`,
`LocalBranch` (now `branch_heads` returning heads), the upstream test and asserts.

Found on the way: on macOS five stacked `sync_tests::` failed before this
slice too (since the Git-root rule, f93814c6d0). The Task fixture recorded the
tempdir as spelled (`/var/folders/…`); `lf`'s cwd reads back as
`/private/var/…`, and `checkout_owner_in` resolves only the queried path, so
`lf` saw no Task. Production records Git's resolved toplevel, so the fixture now
records `canonicalize()`d checkouts (`recorded_checkout` in
`loopflow-execution/tests/support/mod.rs`). Passing locally: the 13 stacked or
detach `sync_tests::`, `git::worktrees::`, `ops::pr::`, `ops::checkout::`,
`task_handoff_tests::`, `task_initialization_tests::`, `worktree_tests::`,
`task_pr_range_tests::`. Standalone binaries sharing the fixture
(`task_pr_authority_tests` and others) are left to gate/CI.
Compress (`f05b4f0dfe`): `branch_pr_merged` lives beside `checkout_base`;
`CheckoutBase::Stacked` is boxed (clippy `large_enum_variant` had failed
`loopflow-execution`). Clippy clean; `ops::pr:: ops::checkout::` 52 and
`sync_tests::` 60 passed. Next: gate.

## Built: one Git-root rule (human review of the ownership split, 2026-11-23)

Answer on the page's question 1: "i think I prefer (c)": Git-root equality
everywhere. A Task's work is what runs in its checkout's Git worktree (any
subdirectory of it); a Git worktree nested inside it is not the Task's, for
ownership or membership. Questions 2-4 follow (choices made, not asked): bare
`lf resume` there says "no associated work conversation" (2a, as built); bare
`lf task status` there says the checkout has no Task (3b); run liveness is
LOO-1075 (4a).

1. **One rule: the Task whose checkout is this directory's Git root.**
   `task_checkout_owner(root)` is the one store answer; `task_holding` and
   its deepest-containing walk go, with every membership caller
   (`starting_task_in`, `counted_task`, `run.rs` select, `handoff.rs`
   `--design`, provider conversations, `implicit_binding`) asking by Git root.
   Counterexample to settle first: the store cannot see Git. A live start
   resolves the root before it records (launch already does); the
   migration's backfill, which has only stored paths, keeps its
   deepest-checkout rule for history written before it, and says so. A
   nested `task_checkouts` row is its own root and still owns itself.
2. **Instructions steer agents away from nested worktrees.** In
   `rust/loopflow-definitions/src/builtins/LOOPFLOW.md`, the child-agent
   paragraph: a harness subagent works in this checkout; a separate checkout
   comes from `lf wt create <name>`, beside this one, never from the
   harness's own worktree tools (Claude Code's `EnterWorktree` and
   `isolation: worktree` nest one under `.claude/worktrees/`, which belongs
   to no Task). Keep it to two sentences; LOOPFLOW.md is paid for on every run.
3. Tests: a Session and a Graph run started in a nested worktree get no Task;
   one started in a subdirectory of the checkout gets the Task; `lf land` and
   bare `lf task status` in the nested worktree answer "no Task".

Built. `SqliteStore::task_checkout_owner(dir)` is the one answer: the Task
whose checkout is the Git worktree holding `dir` (exact root, as spelled and
as resolved, lower id on a shared root). The store finds the root itself with
`loopflow_storage::address::worktree_root`, a filesystem walk for `.git` that
`git::worktree_root` now calls, so the counterexample dissolved: no caller has
to pass a root. A directory no worktree holds is its own root, the only answer
for a path that is gone (fixtures with missing paths ask at the root).
`task_holding`, `deepest_checkout` and execution's `task_holding` are gone;
`run.rs` select, `handoff.rs --design`, `task_status`, provider conversations
and `starting_task_in` ask the owner, and delivery reads `select_execution_work`
alone (owner, else declared). The migration's backfill keeps the deepest
checkout and says why. LOOPFLOW.md's child-agent paragraph points at
`lf wt create`; goldens regenerated. Tests: storage `task_work::` 9 (new
`a_directory_belongs_to_the_task_whose_checkout_is_its_git_worktree`);
execution `store::` 214, `ops::task::adoption::` 10 (nested status says "this
checkout has no Task", a subdirectory of the outer checkout is the outer
Task's), `ops::task::tests:: ops::run:: ops::human_session:: git::` passed;
`golden_prompt`, `documented_commands::` passed; clippy clean. Not run here:
`lf land` in a nested worktree end to end (`land_tests`, CI);
`pr_landing` already asked the owner by Git root.

## Built: the Session list reads the stored Task (concept review on 8e62c4f468)

`human_session/workspace.rs` `associate` no longer touches `task_ids`: a
listed Session's Tasks are the store's `session_tasks` (its
`agent_sessions.task_id`, Repository and Wave primaries excluded), so
`lf session list --task`, its JSON and Desktop's `names(task:)` read one
answer. `WorkspaceResolver` only places the Session (worktree root,
unavailable reason); `workspace.task_id` is the root's owner by
`task_checkout_owner`'s rule (lower id on a shared root), and the
"Multiple Tasks claim this checkout" message is gone. The `--wave` filter's
path clause (`in_task_checkout`) went with it: a Repository primary sitting
in a Wave Task's checkout no longer lists under that Wave. Raw conversations
keep their checkout-owner Task (`turns.rs`), since no Session records them.

Test: `human_session::tests::list_task_keeps_each_session_with_its_task_after_a_move_and_path_reuse`
(execution, since the storage demo can't call `list`): A's Session at the
reused path lists under A only, C's under C. `ops::human_session::` 52
passed (realign, 2026-11-23). Compress after it: a Task's Graphs and open
Sessions read `task_id` directly; `task_checkout_machine_keeps_session_filters_beside_other_machines`
had relied on path membership and now records each Session's Task
(`store::task_work_lifecycle_tests:: ops::human_session::` 57 passed). Concept review on `39ac96a0f9` (2026-11-23) found no mismatch left inside this Task then; the page on `f07ab06545` replaces it.

## Earlier: implement (concept review after the plan hub, 2026-11-22)

Main is merged through #1988 (`8892ba5671`, origin/main's head). The review is
`scratch/concept-review.html`; the answers, word for word, are in
`scratch/questions.md`. Status of its six changes:

1. **`tasks.planning_revision` stays** (1a, contradicted). It is not dead: it
   is `Task.revision`, the compare-and-set every Task edit checks
   (`task_content.rs` `validate_edit`: "Task changed; read its current
   revision before editing"), taken from callers by `lf desktop dashboard
   rename task:<id> --revision` and `edit_task`, and tested by
   `local_planning` (a stale revision is refused). Inventory row: keep, as
   "the Task's edit revision", not "the peer exchange's clock". Nothing built.
2. **A run's OS process: two processes, not two homes** (3b, contradicted).
   `task_graphs.run_pid` is the carrier, `lf task run`; `graph_processes.pid`
   is the `lf graph` child it spawns (`run_task_graph_for`), which registers
   later with its own `LfProcessId` (`RunOwner::TaskCarrier` vs `GraphRun`).
   `carrier_may_run` and retention ask about the carrier, which can be alive
   before the child registers or after it ends. One home means the carrier
   records itself in `graph_processes`, or runs the Graph in-process; either
   reshapes launch, larger than this Task (questions.md). Nothing built.
3. **Built** (`f8a5ef1f1f`). `write_task_checkout_in` no longer gives open
   taskless Sessions the Task; `lf session bind` is the explicit way (it makes
   the Session the Task's primary; a non-primary bind is in questions.md).
   Kept: `task_open_graph_pairs`' path clause, which only stops a second run
   overlapping one already in the checkout and adds nothing to history
   (questions.md). Tests: the four `admission_lifecycle_tests` expect the
   Session to stay taskless; `task_work::` 8 passed.
4. **Built** (`1605ad1d72`, since replaced by the Git-root rule above).
   One store answer for a directory's Task: deepest checkout, lower id on a shared root, `dir` matched as
   named and as it resolves. `counted_task` and bare `lf resume`'s
   `worktree_task` call it; the scan and the prefix match are gone, and so is
   `counted_task`'s Git subprocess. Counterexample found: fixtures recorded
   `/var/folders` paths while children ask with `/private/var`;
   `task_fixture_at` now records the resolved path, as Git does in
   production (106 of 106 live checkouts that exist are canonical). Still a
   third copy: `human_session/workspace.rs` `WorkspaceResolver` derives
   `task_ids` for the Session list from Git roots (questions.md).
   Tests: `the_task_holding_a_directory_is_its_deepest_checkout`,
   `ops::task::tests::` 35, `ops::human_session::` 51, `work_route` 6.
5. **Built** (`06c6145399`, 5.3b). `task_primary_sessions` is the Task
   primary's home again, read and written by this build and released lf
   alike; nothing writes `primary_scope='task'`. One expression,
   `primary_scope_sql(s)` (`coalesce(s.primary_scope, 'task' if a row names
   s)`), gives the scope wherever it is read: the session filter, the
   summary's wire `primary_scope` (Swift's `taskPrimary`), and the bound
   cwd update. "Which Session is the Task's primary" joins the table
   (`primary_session_in`, `task_conversation_here_sql`, `primary_scope()`).
   `mark_task_primary_in` is `INSERT OR REPLACE` of the row. Gone: the
   mirror triggers, the rebuild's `'task'` backfill, the leaving mark, and
   (compress, `8cc3ba3f9c`) the CHECK's `'task'` value, its `task_id` CHECK
   and index `agent_sessions_primary_task`: nothing writes the mark, so
   LOO-877 adds them with its writers; `query_plans.rs` measures
   `task_primary_sessions` again, as main does. Found on
   the way: the `agent_sessions` rebuild dropped main's `agent_socket`
   (`loo974_agent_socket`); it carries it now (SCHEMA.md regenerated).
   Fixed a fixture that still relied on path membership
   (`input_replacement_retains_workspace_and_task_membership`). Tests:
   storage `task_work:: leaving_members_are_unreadable sessions::
   session_recovery:: query_plans` 29 passed; execution `ops::human_session::
   store::session_metadata_tests:: store::sessions:: ops::task::tests::` 105
   passed; application served watch, `server::recovery::` and the three
   `context_launch_tests::` recovery fixtures 13 passed. Not run here: the
   standalone fixture binaries whose SQL moved to the table
   (`session_lifecycle_tests`, `work_open_tests`, `served_reads`,
   `machine_commands`, `task_graph_launch_tests`).
6. **Built** (`c957535ac8`). `'inherited'` leaves the CHECK; the rebuild
   maps the 750 live rows to `'declared'`; `WorkSource::Declared` reads the
   771 history events that say `inherited` (serde alias). Swift reads the
   value as a string; `server_schema.json` regenerated.

Demo at gate (4b): `lf desktop dashboard move working-tasks/task:<id>
--destination task:<id>` from the repository, then `lf task list` (the new
order) and `lf task show` (the rank). `lf task move` does not reorder and `lf
task status` shows no rank (QA 2026-11-22). Desktop's Working board after
merge and install. Auto stays a Machine fact
(2a), unchanged.

## Then: gate

QA on `60fcf050b9` (2026-11-23, `scratch/qa.html`) walked `lf wt create
--stack` against the fake GitHub and fixed three things: `lf pr open` in a
stacked checkout whose base was never pushed failed with `git diff
origin/<base>...HEAD` "bad revision" (it now pushes the base with an
upstream, as `lf sync <Task>` does for an unpushed parent); `lf sync` kept
merging a base whose PR had merged while its local branch remained (it now
asks `gh pr view`, the check `lf pr` already made, and follows main);
`--stack --persistent` dropped the stack. `served_reads` did not compile
(`Write::CheckoutCreate` gained `stack`). Tests:
`sync_tests::a_checkout_created_with_stack` (persistent case added) and
`sync_tests::a_stacked_checkout_publishes_onto_its_unpushed_base_and_follows_main_once_it_merges`
passed; clippy on `loopflow-execution` and `loopflow` clean.
`served_reads::server_operations_match_their_lf_commands` fails at its
`session bind` step (served `outlook` "starting", CLI "could_not_start": the
CLI read follows a start attempt); not this change's write, not checked on
main — gate.

QA on `71feca7db5` (2026-11-23, `scratch/qa.html`) found one more regression
of the same kind and fixed it: `lf commit`/`lf pr open` in a plain worktree
nested inside a Task's checkout acted for the outer Task ("Task … expects
branch \"ta\", but the worktree is on \"ti\""), because `resolve_managed_task`
used the launch selection, now containment. Delivery asks the Git root's
`checkout_owner`, else a declared Task;
`a_worktree_inside_a_tasks_checkout_becomes_its_own_tasks_checkout` covers it.
`ops::task::adoption::` 9, `ops::task::tests:: ops::run:: ops::human_session::`
98 passed.

QA on `86fe72ab65` (2026-11-22, `scratch/qa.html`) found one regression and
fixed it: `lf task create --worktree` refused a worktree nested inside another
Task's checkout ("<outer> already has a different checkout"), because
containment answered where the old `counted_task` compared Git roots; the
Git-root rule settles it, and
`a_worktree_inside_a_tasks_checkout_becomes_its_own_tasks_checkout` covers it.
Also repaired: `creation_adopts_only_the_selected_work` still expected
adoption to give the open conversation the Task (change 3 removed that).
`ops::task::adoption::` 9 and `ops::task::tests::` 35 passed.

Items 1-6 are settled. PR #1973; branch merges main through #2032
(`43e031870e`). The acceptance check last passed at `2448546730` (10 tests).

Run on that head (2026-11-18, clean `LF_*`):
`cargo test -p loopflow-storage --lib -- task_work:: leaving_members_are_unreadable`
— 10 passed. Not run since the merges of #1974 and #1977: every other suite.
CI on the PR has only run `macos-inputs` and `scratch-clear`; the rest show
skipped.

Gate runs, beside CI: the acceptance check above, `session_lifecycle_tests`,
`two_people`, `performance_tests::`, `session_cli_tests::`, `world_recipes::`,
`documented_commands::`, the Swift DTO fixtures and `DashboardWorkspaceTests`.

Seen failing before the #1974 merge, also at `600e0f9495`, not rerun since
(clean `LF_*` env):
`input_replacement_retains_workspace_and_task_membership`,
`screen_reads_are_compact_passive_and_match_cli`, two `work_open_tests`.
Order-dependent then (pass alone):
`resuming_an_ended_conversation_launches_its_provider_as_the_session`,
`task_identifier_repairs_all_generated_titles_from_a_replaced_editor_with_foreign_capture`,
`a_task_run_takes_its_waves_agent_without_flags`. Unconfirmed:
`served_wave_lifecycle_matches_cli_and_keeps_the_same_wave` ("server did not
start" under load ~70-84; fresh-store migration took 11-41s against a 20s
deadline). A test that fails on this branch and passes on main is this
Task's to fix at gate.

The three upgrade tests in `task_work::` each take over 60 s (65 s for the
ten): the cost is replaying to the current schema, not this migration's
backfill (0.19 s for 22,342 live Sessions).

Concept review of the first build (2026-11-18) and its four answers are in
questions.md; all four are applied or handed on (LOO-877, LOO-934).

For the PR body, larger than this Task: launch and store each define "the
checkout holding this directory" (Mismatch A; store side is `task_checkout_owner`
now, `WorkspaceResolver` remains, fits LOO-934).
