# Open choices (made for review)

- **Working board order.** Chosen: `task_orders` goes; Desktop's Working board
  sorts by Project rank, then Task rank, and a tile Move writes rank through
  `reorder_in` (so it reaches Linear as an `order` action). Passed over:
  keeping a board-only manual order as client screen state (like
  `work_pane_layouts` moving to Desktop files). Also dropped: the board's
  "waiting Tasks first" tiebreak, since a drag must visibly land where it was
  dropped. A Move onto a Task in another Project has no rank meaning; it
  answers "nothing changed" with the reason.
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
- **No released-lf trigger for `processes.task_id`.** The maintenance backfill
  catches rows an old binary inserts. Passed over: a trigger, which SQLite
  forbids from using the recursive ancestor probe (`WITH` in a trigger).
- **The branch has three homes after this Task**: `tasks.planning_branch_name`
  (the Task's branch, shared), `task_checkouts.branch` (this Machine's
  checkout), `task_prs.branch` (the PR's head). Whether they can
  ever differ, and so whether one "the Task's branch" suffices, is LOO-934's.
- **Checkout before caller for a Process's Task.** Chosen from the live copy:
  a command run inside a checkout is that Task's work whoever ran it
  (`session serve`, `pr land`, `--task X ship` from another Task's agent).
  Passed over: the kickoff's caller/parent-first order, which gave 6,687 such
  Processes to the calling agent's Task.
- **`observed`-only Processes leave their Task's list.** 166 pairs where a
  Process only wrote an observation into a Session; not treated as the Task's
  work. Passed over: a backfill clause for them.
- **`process_task_backfill` cursor table** so a Process with no Task is
  examined once per store rather than every hourly pass. Drops with LOO-877.
- **No maintenance pass for Sessions.** Released `lf` still creates helper
  Sessions without a Task during the window; they miss their Task's list
  until... nothing assigns them. Chosen: accept it (helpers are PR-title and
  commit-message runs; Task Graphs name their Task). Passed over: a second
  cursor in `assign_process_tasks` for `agent_sessions`.
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
  that accepts its absence first; not filed, belongs with LOO-877's drop.
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
