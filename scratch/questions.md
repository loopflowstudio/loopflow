# Context Lab implementation notes

- PR #906 supplied `0.11.003_child_body_lease`; current main then added
  `0.11.004_task_pr_ci_state` and `0.11.005_provider_accounts`. Context Lab's
  launch-attribution migration is now `0.11.006_context_launch_work`. On July
  15 the branch binary upgraded the production ledger from 005 to 006 through
  the explicit development-binary opt-in and read the real 30-day population.
  The automatic pre-upgrade snapshot is
  `/Users/jack/.lf/loopflow.db.backup-0.11.005_provider_accounts`. The currently
  installed release `lf` knows only 005 and now refuses that 006 ledger, so the
  next pass must use this branch binary (or deliberately restore the snapshot)
  rather than treating the refusal as data loss.
  The isolated pre-rebase development ledger still ends at the divergent local
  stamp `0.11.004_context_launch_work`; do not mistake that disposable dev store
  for production migration failure on the next pass.
- This first pass launches refinement only into an existing Intelligence Task
  that already has a durable Task worktree. Creating a Linear Task from the
  sheet needs a JSON-returning, human-confirmed PM write plus Task Session
  creation; the existing `lf pm task create` prints human text and does not
  return the Task workspace needed for the guarded handoff.
- The local PM snapshot contains Intelligence Context Task W2-71, but the
  `intelligence` Wave is not registered and no W2-71 Task Session exists. That
  prevents the continuous refinement demo from creating or selecting a real
  Task worktree without a separate Wave-registration decision.
- Migration `0.11.006_context_launch_work` records Project slug and Task
  identifier from durable child-control identity on new launches. Context Lab
  now filters those dimensions without worktree-name inference; historical rows
  remain explicitly unattributed.
- The refinement terminal carries its selected Task Session and Wave identity
  into the fresh `refine` process. This makes the intervention itself appear in
  later Project/Task-filtered research without resuming the historical trace.
- Launch now re-reads the Intelligence roadmap immediately before creating a
  terminal and refuses a Task that started running, lost its worktree, changed
  Task Session, moved Wave, or changed worktree path after the sheet opened.
  The resulting Task workspace opens the terminal that actually owns the fresh
  refinement process rather than the inactive Task agent tab.
- Fresh canonical operating-guide capture is no longer an open question. The
  production ledger contains five naturally captured turns across four sessions
  for `/Users/jack/src/loopflow/rust/loopflow/src/engine/builtins/LOOPFLOW.md` at
  effective hash `130b91c3afb3afa7897e22cb85068a1714ab6431469dee3392eda10eb8bdd4fe`;
  the current-file hash is identical and the revision is editable.
- The current filter contract has no steered-only or current-revision selector;
  steering is a stat/sort and Context Lab always returns all observed revisions.
  The revision comparison gate checks exposure count and complete-capture parity
  but does not balance provider/model mix or observation windows.
- The installed app logs repeated SwiftUI `AttributeGraph` cycles at startup.
  They predate opening Context Lab and have not yet been tied to a visible Lab
  failure, but the final installed-app pass should prove they are unrelated or
  remove their source.
