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
