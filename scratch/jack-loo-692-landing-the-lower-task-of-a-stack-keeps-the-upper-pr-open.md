# LOO-692: landing the lower Task of a stack keeps the upper PR open

Status: implemented, gated. Realigned 2026-10-30: no upstream changes since the base; code matches this plan.

## Outcome

When the lower PR of a stack lands and its branch is cleaned up, the upper PR
stays open, now based on the default branch, and its branch stays intact.

## Evidence (manabot, 2026-10-30)

- #398 (T2) merged 12:38:40Z by `lf ops pr land --wait-and-fix`. At 12:56:53Z
  `lf task complete 87e012` deferred checkout cleanup because a Process was live.
- 13:32:42Z: the T2 branch was deleted, which made GitHub close #399. 13:33:00Z:
  the T3 branch was deleted. That is 18 s later, so it was a later sweep that
  saw #399 already closed.
- Nothing in this Machine's ledger covers 13:26–13:33Z. Every lf at that point
  was 0.15.0, which logged "incompatible Loopflow database; Processes are not
  being recorded". No transcript, cron entry or launchd job names the command.
  mini-heart has no prune schedule.
- Code evidence: every Task branch deletion on origin goes through
  `wt::apply_delete` (land, completed-Task cleanup, PR landing cleanup,
  abandon, `wt prune --branches`); only release candidates are deleted
  directly (`release.rs`), and nothing stacks on them. Only `branch_prune` treats a closed PR as deletion evidence. So
  defect 2 is `branch_prune` running without a worktree or live-Task protection
  for T3, for example from another clone or with an unreadable registry.

## Change

- `wt::apply_delete` calls `retarget_dependents` before deleting a remote
  branch. On a GitHub origin, it moves every open PR based on that branch to
  the default branch. If listing or editing fails, the deletion fails and the
  branch is retained. `gh` and `git` share one `run` helper in `wt.rs`.
- `branch_prune::decide`: a closed, unmerged PR at the head is evidence only
  when a finished Task names the branch. Otherwise the branch is retained as
  `Retention::ClosedPullRequest`.
- Docs: `docs/troubleshooting.md` and the `wt prune --branches` help.

## Concept review (2026-10-30)

No change. The PR-side retarget agrees with the existing Task stack model in
`ops::task::task_stack`:

- Merged parent: the child's publication base is already the default branch,
  and `stack_for_landing` refuses to land a child before its parent merges, so
  a stacked parent always lands onto the default branch. Retargeting to the
  default branch is the merged-into branch; no `baseRefName` lookup is needed.
- Abandoned parent: `task_stack` refuses the child ("re-place the child
  deliberately"). The retarget only keeps the upper PR open; placement stays
  the person's decision at the child's next sync. Retaining the abandoned
  parent's branch on origin instead is an alternative, not a requirement of
  this outcome (see `questions.md`).
- A closed PR is deletion evidence only beside a finished Task, matching
  "uncertain evidence never auto-closes work".

## Delete — do not maintain

- Closed-PR-alone as branch deletion evidence (removed from `decide`).

## Checks

`cargo test -p loopflow --test integration wt_prune_branches::` passes (9),
including `landing_the_lower_of_a_stack_keeps_the_upper_pr_open_on_its_branch`.
With the retarget step disabled, that test fails (CLOSED ≠ OPEN). `land_tests::`
passes (30). After compress, `wt_prune_branches::` passes (9). Lib `ops::branch_prune::`, `ops::wt::` and `ops::task::lifecycle`
pass.

Gate (2026-10-30): `scripts/test.py --reuse-passing --rust-filter 'test(wt_prune_branches::) | test(land_tests::) | test(task_abandonment::) | test(ops::wt::) | test(ops::branch_prune::) | test(ops::task::lifecycle) | test(ops::pr_landing::) | test(ops::abandon::) | test(ops::land::)'` — PASS (architecture, rustfmt, nextest, clippy, website). Live GitHub stack left to verify.
