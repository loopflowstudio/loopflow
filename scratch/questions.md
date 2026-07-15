# Context Lab implementation notes

- The branch's store migrations predate the machine's live `loopflow.db`; the
  branch binary refuses it with “written by a newer Loopflow.” Aggregation is
  covered against constructed rows and the shared Rust/Swift fixture, but the
  installed-app 30-day demo needs the branch rebased onto the newer store
  schema before it can run against the long-lived ledger.
- This first pass launches refinement only into an existing Intelligence Task
  that already has a durable Task worktree. Creating a Linear Task from the
  sheet needs a JSON-returning, human-confirmed PM write plus Task Session
  creation; the existing `lf pm task create` prints human text and does not
  return the Task workspace needed for the guarded handoff.
- Project/Task attribution is not present on the branch's launch ledger rows,
  so the session-set query exposes the recorded wave/flow/skill dimensions but
  cannot honestly offer Project or Task filters yet. Add those filters with the
  ledger attribution rather than reconstructing them from worktree names in
  Swift.
