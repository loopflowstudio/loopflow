# LOO-692 open questions and assumptions

- Retarget destination: dependents move to the repository's default branch, not
  to the merged PR's own base. Loopflow stacks land onto the default branch, so
  the two are the same today. A stack whose lower PR merges into another
  feature branch would need the merged PR's `baseRefName`.
- Abandon also retargets: `lf pr abandon` and `lf task abandon` delete through
  `apply_delete`, so a PR stacked on abandoned work moves to the default branch
  instead of being closed. Its diff then includes the abandoned commits until it
  is synced.
- A PR closed outside Loopflow, with no finished Task, now keeps its branch
  (`retained … closed PR #N without a finished Task`). Delete it with
  `lf wt delete <branch>`. Jack may prefer a different rule for PRs that
  people close by hand.
- Attribution is unresolved: the command that deleted the manabot branches at
  13:32Z left no ledger record (0.15.0 against a newer database). Both defects
  are fixed at their shared owners whichever path it was.
