# Open choices (made for review)

- **Working board order.** Chosen: `task_orders` goes; Desktop's Working board
  sorts by Project rank, then Task rank, and a tile Move writes rank through
  `reorder_in` (so it reaches Linear as an `order` action). Passed over:
  keeping a board-only manual order as client screen state (like
  `work_pane_layouts` moving to Desktop files). Also dropped: the board's
  "waiting Tasks first" tiebreak, since a drag must visibly land where it was
  dropped. A Move onto a Task in another Project has no rank meaning; it
  answers "nothing changed" with the reason.
- **Peer-only Machines lose the Task URL** a released peer sent via
  `planning_peer_changes` once `planning_url` leaves `tasks`. A Machine bound
  to Linear observes it itself. Passed over: keeping URL on `tasks` as a
  shared fact.
- **`task_prs.branch/base_commit/parent_pr_id`** stay: trigger-copied from
  `task_checkouts` while a PR is open, and the only record once it merges and
  the checkout is gone. Could become "the PR's branch" only, with open PRs
  reading the checkout; left for LOO-934.
- **Renames** (`issue_title` → title, `planning_*`, `agent_sessions` →
  sessions) are table rebuilds that keep every column; left to LOO-939.
- **Released-lf trigger** filling `processes.task_id` from the path is the one
  compat shim; it goes with LOO-877. Add that to LOO-877's brief at gate.
