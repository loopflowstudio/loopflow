# Open choices (made for review)

- **Working board order.** Chosen: `task_orders` goes; Desktop's Working board
  sorts by Project rank, then Task rank, and a tile Move writes rank through
  `reorder_in` (so it reaches Linear as an `order` action). Passed over:
  keeping a board-only manual order as client screen state (like
  `work_pane_layouts` moving to Desktop files). Also dropped: the board's
  "waiting Tasks first" tiebreak, since a drag must visibly land where it was
  dropped. A Move onto a Task in another Project moves it there (answer 2
  below).
- **`task_prs.branch/base_commit/parent_pr_id`** stay: trigger-copied from
  `task_checkouts` while a PR is open, and the only record once it merges and
  the checkout is gone. Could become "the PR's branch" only, with open PRs
  reading the checkout; left for LOO-934.
- **Renames** (`issue_title` → title, `planning_*`, `agent_sessions` →
  sessions) are table rebuilds that keep every column; left to LOO-939.
- **Task URL, branch and team stay on `tasks`** (concept review). The kickoff
  plan moved them to `pm_items`; Git planning writes the branch itself
  (`record_task_branch_in`) and peers carry all three. Chosen rule: `tasks`
  holds every fact shown on any Machine, `pm_items` what the provider last
  said. Passed over: provider-only facts in `pm_items`, which loses the branch
  on Git-planning repositories.
- **The branch has three homes after this Task**: `tasks.planning_branch_name`
  (the Task's branch, shared), `task_checkouts.branch` (this Machine's
  checkout), `task_prs.branch` (the PR's head). Whether they can
  ever differ, and so whether one "the Task's branch" suffices, is LOO-934's.
- **Checkout before caller for a Session's or Graph run's Task.** Chosen from the live copy:
  a command run inside a checkout is that Task's work whoever ran it
  (`session serve`, `pr land`, `--task X ship` from another Task's agent).
  Passed over: the kickoff's caller/parent-first order, which gave 6,687 such
  Processes to the calling agent's Task.
- **No maintenance pass for Sessions.** Released `lf` still creates helper
  Sessions without a Task during the window; they miss their Task's list
  until... nothing assigns them. Chosen: accept it (helpers are PR-title and
  commit-message runs; Task Graphs name their Task). Passed over: a second
  maintenance pass for `agent_sessions`.
- **Guard reach.** Chosen: only `loopflow-storage`'s `cfg(test)` stores.
  Passed over: `test-support` stores too, which would catch execution and
  application SQL but put the authorizer behind a feature Cargo unifies into
  the CLI build.
- **Leaving list from embedded migrations**, not a `build.rs` emission: the
  binary already carries every migration's SQL; parsing it once in tests
  needs no generated file.
- **FK columns exempt.** SQLite reports its key checks as plain reads, so a
  direct read of a leaving FK column passes. Passed over: tracking the
  statement's write target to tell them apart (no statement boundary in the
  authorizer; subselects arrive between the write and the key check).
- **`Disposition.planning_completed` stays on the wire.** The column left;
  the exchanged key did not, because the Git journal and released peers
  require it (DTOs carry no defaults). Chosen: derive it from the state on
  read, honour it only when no state is given. Removing it needs a release
  that accepts its absence first; recorded on LOO-877.
- **Sync trigger for `planning_completed`.** Chosen over "stop writing":
  released lf on the same store reads the boolean for status, ownership and
  retirement, and existing retirement triggers fire on it. Passed over:
  rewriting those triggers onto `planning_state` now; the contract can.
- **Tasks with no Project on the Working board** sit after every Project.
  Passed over: before every Project, or interleaved by their own rank (ranks
  of standalone Tasks and of Projects are separate sequences, so interleaving
  has no meaning).
- **Released lf's order list during the window.** This build stops writing
  `tracker_projects.task_order_json`; released lf on the same store keeps
  writing it on each Linear observation and reads it for its omission check
  and an order action's base. Between its own observations that list can lag
  a rank this build accepted. Chosen: accept it (it repairs itself on the next
  observation). Passed over: a trigger rebuilding the list from
  `tracker_items` ranks.
- **Moving a done Task.** `reorder_task` ranks any undeleted Task, done ones
  included; the board offers only Working tiles, and `lf desktop dashboard
  move working-tasks/...` with a done Task now changes its rank instead of
  refusing.
- **Rank ties break by id** (QA). The board key already did; `siblings_in`
  and `project_members` used `created_at`, so a drag among rank-0 Tasks (73
  standalone loopflow Tasks on the live store) landed elsewhere. Passed over:
  `created_at` in `TaskOrder::key`, which needs it on the `TaskSummary` wire DTO.

- **A Task's primary is `agent_sessions.primary_scope='task'`** (Mismatch A,
  fixed in this PR at human review). `task_primary_sessions` is marked leaving;
  two triggers keep it and the mark in step for released lf until LOO-877
  drops both. Index `agent_sessions_primary_task(task_id) WHERE
  primary_scope='task'`, no `completed_at` filter: a Task has one primary, as
  the table's primary key said. `check_migrations.py` refuses a new unique
  index while older writers run, so it is plain now and every write clears the
  old mark first; LOO-877 makes it unique. Cost in the window, as QA saw it
  with the installed lf on a migrated home: an older lf leaves every Task's
  primary out of the Task's Sessions and cannot reopen it (`lf session resume`:
  "is not an unfinished interactive conversation of this Task"), whether it
  was running before the migration or started after. Passed over: keeping the
  mark in `task_primary_sessions` until LOO-877, which older lf reads
  correctly. Open for a person: whether that window is acceptable for work
  in flight on the day of `lf install`.
- **The board order someone dragged before upgrade is not carried into rank**
  (QA). `task_orders` is no longer read, so the Working board returns to rank
  order. Live store: two rows, 3 open Tasks named. Passed over: seeding ranks
  from it in the migration, which would send `order` actions to the tracker.
## Concept review answers (human review, 2026-11-18)

"1 (a) 2(a) 3(a) 4(a)", against scratch/concept-review.html's questions:
1. Wire `task_primary`: drop it in this PR.
2. Cross-Project drop on the Working board: moves the Task into that Project
   at that position.
3. The branch's three homes: LOO-934 decides whether one "the Task's branch"
   suffices.
4. `Disposition.planning_completed` on the wire: file the release that
   accepts its absence with LOO-877.

Applied: 1 and 2 in `a3639aa8f2`; 3 and 4 are recorded on LOO-934's and
LOO-877's Tasks.

- **A move takes any of the repository's Projects** (from answer 2). Dropping
  on another Project's Task moves it there, then ranks it; `lf task move`
  shares the path. The Wave's-configured-Project check stays on creation only:
  a board shows older Projects' Tasks, and refusing a drop into one would read
  as "ensure the Wave before starting new work". Passed over: keeping the check
  for `lf task move` alone, which gives one action two rules.

## Review against main after #1974 (2026-11-18)

On "This PR's Process half": "1) what is the best longterm solution? dont care
about this PR growing". Recommended and taken: drop it for Sessions and Graph
runs each naming their Task at start, by one shared rule (plan, "Next").
"2) i dont understand it", "3) dont understand": both follow from 1 (lf
commands in a checkout are not listed as Task work; LOO-965 owns
`relocated_task_processes`). "4) (b)": a fresh run from `implement`.

## Merge of main after #1974 (implement, 2026-11-18)

- **A run already in a checkout a Task then takes stays out of that Task's
  history.** Main says so (`open_graph_execution`: "prevents overlapping runs
  without making that Graph part of the Task's history"), and
  `adopting_a_checkout_with_a_running_graph_prevents_another_run` checks the
  Task is idle and runnable once that run ends. So `task_open_graph_pairs`
  keeps one path clause, for unended runs that recorded no Task; a run's
  `task_id` stays fixed from its start. Passed over: adoption giving the run
  the Task (as it does open Sessions), which left the Task Blocked on a run
  from before it held the checkout. Sessions and runs differ here; whether
  they should is open.
- **The migration's Graph-run backfill attaches earlier taskless runs by
  today's checkouts**, ended ones included, so a run that predates a Task's
  adoption of its checkout joins that Task's history on upgrade. Few rows:
  `graph_process_task` already paired runs by path, and `lf graph` in a
  checkout has named the Task since. Passed over: no backfill.
- **Steers during this pass** (Jack Heart, via the Task): "Cancel the previous
  steer: keep the migration name task_facts_once. Do not rename it." and "Bug
  in task_facts_once: the Graph-run backfill takes the caller Session's Task
  only when s.primary_scope IS NULL ... Use the membership rule". At that
  point in the migration no row can hold `'task'` yet (the CHECK is rebuilt
  later), so it selected the same rows; it now reads as the membership rule,
  and both the upgrade test and the membership test start a run from a Task
  primary.

## Concept review as built (0ef1ad038a, 2026-11-18)

- **Advance to gate without unifying the two checkout lookups.** The launch
  (`counted_task`, Git worktree root, error on two claimants) and the store
  (`deepest_checkout`, path ancestor, deepest wins) each define "the Task
  holding this directory". Chosen: say so in the plan and on the page, leave
  the code. Passed over: one storage function both call, in this PR; it
  changes what every launch resolves.
- Four questions on `scratch/concept-review.html` are unanswered: adoption
  for Sessions vs runs, where Mismatch A goes, the primary mark's upgrade
  day, `work_source='inherited'`.

## Sync with main (2026-11-22)

Kept main’s removal of the old Git planning exchange test. Conversation recovery now reads the Session’s Task primary mark, and the table rebuild carries its recovery counters and last start failure.
