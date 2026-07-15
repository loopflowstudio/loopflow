# Context Lab implementation notes

- PR #906 supplied the missing `0.11.003_child_body_lease` migration. The branch
  now reads the live ledger, and the installed dev app renders the real 30-day
  Loopflow population.
- This first pass launches refinement only into an existing Intelligence Task
  that already has a durable Task worktree. Creating a Linear Task from the
  sheet needs a JSON-returning, human-confirmed PM write plus Task Session
  creation; the existing `lf pm task create` prints human text and does not
  return the Task workspace needed for the guarded handoff.
- The local PM snapshot contains Intelligence Context Task W2-71, but the
  `intelligence` Wave is not registered and no W2-71 Task Session exists. That
  prevents the continuous refinement demo from creating or selecting a real
  Task worktree without a separate Wave-registration decision.
- Project/Task attribution is not present on the branch's launch ledger rows,
  so the session-set query exposes the recorded wave/flow/skill dimensions but
  cannot honestly offer Project or Task filters yet. Add those filters with the
  ledger attribution rather than reconstructing them from worktree names in
  Swift.
- The installed app logs repeated SwiftUI `AttributeGraph` cycles at startup.
  They predate opening Context Lab and have not yet been tied to a visible Lab
  failure, but the final installed-app pass should prove they are unrelated or
  remove their source.
