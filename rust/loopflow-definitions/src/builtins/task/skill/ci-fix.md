---
requires: a landing PR with failed or timed-out CI, a required sync, or a CI failure to fix
produces: a CI fix published with auto-merge enabled
diff_files: false
action_style: procedural
---
Fix failing CI checks and leave the repaired PR published with auto-merge enabled.

Nobody is here to answer questions. A local commit is not done: done is the
fixed head published and armed.

1. **Sync first.** `lf commit` if needed, then `lf sync`; resolve conflicts and
   finish the sync before looking at CI. Keep the failed SHA you were given
   even when the local head changes.

2. **Read the failures for the published head.** Use the PR and check details
   you were given, or:
   ```bash
   gh pr view --json number,headRefName,headRefOid,url
   repo=$(gh repo view --json nameWithOwner -q .nameWithOwner)
   sha=$(gh pr view <pr> --json headRefOid -q .headRefOid)
   gh api "repos/$repo/commits/$sha/check-runs"
   ```
   With no PR, stop and say so. Read the failed checks' logs against the synced
   code; an old failure may already be fixed. Continue from an earlier
   attempt's changes if you were handed one.

3. **Fix the cause.** Reproduce each failure locally and make the smallest
   correct fix, with a focused check.
   - A failed test is never repaired by a rerun, an empty commit, a retry, a
     longer timeout or a wider tolerance. A test that fails on a timeout, hang
     or race is flaky even when this branch didn't touch it: make it
     deterministic or delete it here, and say which in the PR body.
   - When only the sync was required and checks are green, finish and verify
     the sync; green on an old base doesn't count.
   - `merge-queue (<reason>)` means GitHub's queue dropped the PR and cleared
     auto-merge. Read the merge-group run. A runner fault needs no code change:
     confirm the head's own checks pass and `lf arm` again. Rerunning the
     merge-group run does not re-queue it.
   - `merge-request (<cause>)` means auto-merge disappeared with nothing
     failing. Confirm the head's checks pass, then `lf arm`.

4. **Publish and arm.** Read the whole diff and keep unrelated work out.
   `lf commit -m "ci-fix: <what failed and why>"`, then `lf arm`, which pushes
   the exact head, enables auto-merge and returns. Confirm the published
   `headRefOid` matches local `HEAD` and GitHub shows auto-merge enabled or the
   PR merged.

Report the published SHA, PR URL, auto-merge state and the local checks you
ran, apart from CI still pending on the new head. If you can't fix it, say what
you tried and what stopped you; never call an unpublished or unarmed fix done.
Asked for a structured conclusion, answer with it in the final message.

Make every change through `lf`. Don't call `lf land`, start another watcher or
wait for merge; the landing that launched you does that. Fixing CI doesn't
complete the Task.
