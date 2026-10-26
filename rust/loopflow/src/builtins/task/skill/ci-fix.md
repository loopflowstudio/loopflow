---
requires: recorded PR landing with failed or timed-out CI, required integration, or CI failure context
produces: verified CI repair published with auto-merge enabled
diff_files: false
action_style: procedural
---
Fix failing CI checks and leave the repaired PR published with auto-merge enabled.

## Goal

Start from an up-to-date branch, repair the recorded head's failures, verify the
repair, and publish it with auto-merge enabled. Later finite checks observe
GitHub for authoritative merge; the finishing Graph or Task/Wave operator owns
Task completion.

## Workflow

1. **Sync first**
   - Preserve existing work with `lf commit` when needed, then run `lf sync`.
   - Resolve conflicts and continue the sync before investigating CI. Keep
     the recorded failed SHA as evidence even when the local head changes.

2. **Resolve the recorded PR and failures**
   - Use supplied PR/check metadata, or resolve the current branch:
     ```bash
     gh pr view --json number,headRefName,headRefOid,url
     ```
   - If no PR exists, stop and name what is missing.
   - Read checks for the exact published head:
     ```bash
     repo=$(gh repo view --json nameWithOwner -q .nameWithOwner)
     sha=$(gh pr view <pr> --json headRefOid -q .headRefOid)
     gh api "repos/$repo/commits/$sha/check-runs"
     ```
   - Focus on the most recent completed failed checks and their logs. Compare
     the failures with the synced code; an old failure may already be fixed.
   - If a previous repair conclusion is supplied, inspect its existing changes
     and continue from them. The same head can recover through a check rerun;
     do not manufacture a commit just to change its SHA.

3. **Repair and verify**
   - For required integration with green checks, finish the sync and verify its
     result; green checks on an obsolete base do not satisfy required integration.
   - A `merge-queue (<reason>)` check means GitHub's queue dropped the PR and
     cleared auto-merge. Read the supplied merge-group run; an entry that ran
     nothing, e.g. `merge_conflict`, supplies none.
     A runner fault (Docker or service unavailable, cancelled infrastructure,
     a check that never started) needs no code change: verify the PR head's own
     checks pass and run `lf arm` to queue it again. A real failure gets repaired
     like any other check. Rerunning the merge-group run does not re-queue it.
   - A `merge-request (<cause>)` check means auto-merge disappeared with no
     person's removal behind it and nothing failing. Verify the head's checks
     pass, then run `lf arm`.
   - Reproduce each failure locally and apply the smallest correct fix.
   - Run focused checks first; broaden only when needed. Do not ignore tests
     or publish an empty commit to manufacture a rerun.
   - Address the cause in gate guidance or repository conventions when a
     missing local check let the failure reach CI.

4. **Publish and enable auto-merge**
   - Inspect the complete diff and keep unrelated work out of the repair.
   - Commit with `lf commit -m "ci-fix: <what failed and why>"`, then run
     `lf arm`. Arm prepares the exact head, pushes it, enables auto-merge,
     and returns without waiting for CI or merge.
   - Preserve the recorded Task and PR identity. CI repair does not complete
     the Task; its caller owns completion after merge.
   - Verify the published `headRefOid` matches local `HEAD` and GitHub shows
     auto-merge enabled (or already merged). Local edits or a local commit
     alone do not complete the repair.

5. **Report the handoff**
   - Give the published SHA, PR URL, auto-merge state, and local checks run.
     Distinguish local passes from CI still pending on the new head.
   - If blocked, name the capability (provider, github-observation, secrets,
     publication) and exact next action. Do not claim an unpublished or unarmed
     repair is complete.
   - When the landing reconciler launches the run with an `--output-schema`
     conclusion, answer with it. Its `published` or `blocked` status tells the
     reconciler whether to continue or surface the required human action. Write
     it only in the final answer.

## Guardrails

- Sync, commit, push, and enable auto-merge for the recorded PR.
  Route mutations through `lf`.
- Stay scoped to the CI failures and their prevention; prefer targeted fixes.
- Do not call `lf land`, spawn another watcher, or wait for merge. Return
  after the repaired head is published and armed; a later check observes delivery.
- If the repair cannot be verified or published, report the blocker and stop.
