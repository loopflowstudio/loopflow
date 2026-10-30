# LOO-692 open questions and assumptions

- Retarget destination: dependents move to the repository's default branch.
  Settled by the stack model: `stack_for_landing` refuses to land a child
  before its parent merges, so every Loopflow stack lands onto the default
  branch.
- Abandon also retargets: `lf pr abandon` and `lf task abandon` delete through
  `apply_delete`, so a PR stacked on abandoned work moves to the default branch
  instead of being closed. Its diff then includes the abandoned commits until it
  is synced. Its Task still refuses to sync until the child is re-placed
  deliberately (`task_stack`). Jack may prefer keeping the abandoned branch on
  origin while dependents are open; that would change `lf abandon`, not
  landing.
- A PR closed outside Loopflow, with no finished Task, now keeps its branch
  (`retained … closed PR #N without a finished Task`). Delete it with
  `lf wt delete <branch>`. Jack may prefer a different rule for PRs that
  people close by hand.
- Attribution is unresolved: the command that deleted the manabot branches at
  13:32Z left no ledger record (0.15.0 against a newer database). Both defects
  are fixed at their shared owners whichever path it was.
