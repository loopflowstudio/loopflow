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
GitHub and complete the Task only after an authoritative merge.

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
   - For a supplied CI timeout, diagnose pending or missing expected checks.
     Use at most the supplied rerun allowance. A rerun resets the attempt clock,
     not its allowance; report a blocker if it cannot start or still stalls.
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
   - Use the reconciler's supplied arm command verbatim: `lf arm -c`
     preserves Task completion, and `lf arm --next <slug>` preserves rotation.
     Outside a recorded landing, use bare `lf arm` unless the user requested
     a Task disposition.
   - Verify the published `headRefOid` matches local `HEAD` and GitHub shows
     auto-merge enabled (or already merged). Local edits or a local commit
     alone do not complete the repair.

5. **Report the handoff**
   - Give the published SHA, PR URL, auto-merge state, and local checks run.
     Distinguish local passes from CI still pending on the new head.
   - If blocked, name the capability (provider, github-observation, secrets,
     publication) and exact next action. Do not claim an unpublished or unarmed
     repair is complete.
   - When the landing reconciler supplies a final-answer format, use it. Its
     `published` or `blocked` result tells the reconciler whether to continue or
     surface the required human action. Write it only in the final answer.

## Guardrails

- Invoking this skill authorizes sync, commit, push, and auto-merge for the
  recorded PR. Route mutations through `lf`.
- Stay scoped to the CI failures and their prevention; prefer targeted fixes.
- Do not call `lf land`, spawn another watcher, or wait for merge. Return
  after the repaired head is published and armed; a later check observes delivery.
- If the repair cannot be verified or published, report the blocker and stop.
