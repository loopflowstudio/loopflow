---
requires: a landing PR with failed or timed-out CI, a required sync, or a CI failure to fix
produces: a CI fix published with auto-merge enabled
diff_files: false
action_style: procedural
---
Fix failing CI checks and leave the repaired PR published with auto-merge enabled.

## Workflow

1. **Sync first.** Save work with `lf commit` if needed, then `lf sync`. Resolve
   conflicts and finish the sync before looking at CI. Keep the failed SHA you
   were given even when the local head changes.

2. **Find the PR and its failures.** Use the PR and check details you were given,
   or resolve the current branch:
   ```bash
   gh pr view --json number,headRefName,headRefOid,url
   ```
   If there is no PR, stop and say so. Read checks for the exact published head:
   ```bash
   repo=$(gh repo view --json nameWithOwner -q .nameWithOwner)
   sha=$(gh pr view <pr> --json headRefOid -q .headRefOid)
   gh api "repos/$repo/commits/$sha/check-runs"
   ```
   - Read the latest completed failed checks and their logs, and compare them
     with the synced code; an old failure may already be fixed.
   - If you were handed an earlier attempt's conclusion, continue from its
     changes. A rerun can fix the same head; don't make a commit just to change
     the SHA.
   - Before changing code, check whether the same check fails on the base
     branch's latest commit, or failed on a timeout, hang or race unrelated to
     the diff. That is a flake: rerun it (`gh run rerun <run-id> --failed`), or
     re-queue a failed merge-queue run with `lf arm`, and report `published`
     with no code change.

3. **Fix and verify.**
   - When the sync was the requirement and checks are green, finish the sync
     and verify it; green checks on an old base don't count.
   - A `merge-queue (<reason>)` check means GitHub's queue dropped the PR and
     cleared auto-merge. Read the merge-group run you were given; an entry that
     ran nothing, e.g. `merge_conflict`, has no logs. A runner fault (Docker or
     a service unavailable, cancelled infrastructure, a check that never started)
     needs no code change: confirm the PR head's own checks pass and run
     `lf arm` to queue it again. Fix a real failure like any other check.
     Rerunning the merge-group run does not re-queue it.
   - A `merge-request (<cause>)` check means auto-merge disappeared with nobody
     removing it and nothing failing. Confirm the head's checks pass, then run
     `lf arm`.
   - Reproduce each failure locally and make the smallest correct fix.
   - Run focused checks first; broaden only when needed. Don't skip tests or
     push an empty commit to force a rerun.
   - When a missing local check let the failure reach CI, fix that in the gate
     guidance or repository conventions too.

4. **Publish and arm.**
   - Read the whole diff and keep unrelated work out.
   - `lf commit -m "ci-fix: <what failed and why>"`, then `lf arm`. Arm pushes
     the exact head, enables auto-merge and returns without waiting.
   - Keep the same Task and PR. Fixing CI doesn't complete the Task; the caller
     does that after merge.
   - Confirm the published `headRefOid` matches local `HEAD` and GitHub shows
     auto-merge enabled (or already merged). A local commit alone is not done.

5. **Report.**
   - The published SHA, PR URL, auto-merge state, and the local checks you ran,
     kept apart from CI still pending on the new head.
   - If you can't fix it, say what you tried and what stopped you (provider,
     GitHub, secrets, publishing). Never call an unpublished or unarmed fix
     done. Nobody is available to answer questions.
   - When the landing launches you with an `--output-schema` conclusion, answer
     with it. A `blocked` result ends this attempt; the landing starts the next
     after a backoff and hands it your summary. Write it only in the final answer.

## Rules

- Make every change through `lf`: sync, commit, push, arm.
- Stay on the CI failures and their prevention.
- Don't call `lf land`, start another watcher, or wait for merge. Return once
  the fixed head is published and armed.
- If the fix can't be verified or published, report what stopped it and stop.
