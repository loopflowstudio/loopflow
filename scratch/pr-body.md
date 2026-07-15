## What

First serial PR for W2-93. A Task can now hold a **stack of open serial PRs**: a child branches off an unmerged parent, keeps shipping, and collapses onto `origin/main` **squash-safely with zero manual git** once the parent lands.

The root cause this fixes: `task_prs.base_commit` was always `origin/main` and rebase never read it — it recomputed a fork point at runtime via `git cherry` (patch-id based), which a squash merge defeats, reintroducing parent commits. Now the child's real fork base is persisted and drives a deterministic `git rebase --onto <main> <base>`.

## Changes

- **Model** (`migration 0.11.004_task_pr_stack`): adds `task_prs.parent_pr_id` and retires the one-open-PR-per-session index. A Task holds a concurrent open serial-PR stack; `active_task_pr` is now the stack tip (highest-sequence open PR). Store plumbing: `stack_task_pr` insert, `collapse_task_pr` repoint, `task_session_by_worktree` lookup.
- **Engine** (`ops/rebase`): `RebaseOptions.fork_base` / `RebasePlan.fork_base` drive `git rebase --onto <onto> <base>`, replaying only `base..HEAD`. An ancestry guard (`OpsError::UnsafeRebaseBase`) refuses and names the divergent commits when the base is not an ancestor of HEAD. `lf rebase --plan` surfaces the fork base.
- **`lf pr stack --next <slug>`**: branches a child off the open parent's tip, persisting `parent_pr_id` + fork base.
- **Collapse**: `lf pr land` / `lf rebase` in a Task worktree resolve the stacked child, rebase it onto main via the persisted base, then clear the parent link and repoint the base. The parent's live GitHub state gates the collapse (non-tip parents aren't reconciled).

## Proof

- `rebase_tests`: a child stacked on a two-commit parent that both edit one file — squash-merged so `git cherry` can't split the patch — collapses onto main carrying only its own file; unsafe (non-ancestor) base is refused.
- `store::tests`: parent link round-trips, two PRs stay open with the tip active, collapse clears the link and repoints the base, stacking on a settled parent is rejected.
- Full package: 1061 lib + all integration suites green.

## Deferred to PR2 (`prune-divergence`)

Pruned/divergent parent paths E2E; restack-onto-open-parent when trunk advances under an unmerged parent (this PR refuses the child land until the parent merges); parent-abandon guard; Task-status / PR-range / gh-base surfacing the persisted base; the 3-PR dogfood. A merged non-tip parent's row stays `open` after collapse (cosmetic; PR2 reconciles all open PRs).
