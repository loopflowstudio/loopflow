---
requires: watched PR landing with failing CI checks (or CI failure message context)
produces: verified CI repair published with auto-merge enabled
diff_files: false
action_style: procedural
---
Fix failing CI checks and leave the repaired PR published with auto-merge enabled.

## Goal

Start from an up-to-date branch, repair the watched head's failures, verify the
repair, and publish it with auto-merge enabled. The landing supervisor watches
GitHub and completes the Task only after an authoritative merge.

## Workflow

1. **Rebase first**
   - Preserve existing work with `lf commit` when needed, then run `lf rebase`.
   - Resolve conflicts and continue the rebase before investigating CI. Keep
     the watched failed SHA as evidence even when the local head changes.

2. **Resolve the watched PR and failures**
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
     the failures with the rebased code; an old failure may already be fixed.
   - If a previous repair conclusion is supplied, inspect its existing changes
     and continue from them. The same head can recover through a check rerun;
     do not manufacture a commit just to change its SHA.

3. **Repair and verify**
   - Reproduce each failure locally and apply the smallest correct fix.
   - Run focused checks first; broaden only when needed. Do not ignore tests
     or publish an empty commit to manufacture a rerun.
   - Address the cause in gate guidance or repository conventions when a
     missing local check let the failure reach CI.

4. **Publish and enable auto-merge**
   - Inspect the complete diff and keep unrelated work out of the repair.
   - Commit with `lf commit -m "ci-fix: <what failed and why>"`, then run
     `lf pr arm`. Arm prepares the exact head, pushes it, enables auto-merge,
     and returns without waiting for CI or merge.
   - Use the supervisor's supplied arm command verbatim: `lf pr arm -c`
     preserves Task completion, and `lf pr arm --next <slug>` preserves rotation.
     Outside a watched landing, use bare `lf pr arm` unless the user requested
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
   - When the landing supervisor supplies a final-answer format, use it. Its
     `published` or `blocked` result tells the watcher whether to continue or
     surface the required human action. Write it only in the final answer.

## Guardrails

- Invoking this skill authorizes rebase, commit, push, and auto-merge for the
  watched PR. Route mutations through `lf`.
- Stay scoped to the CI failures and their prevention; prefer targeted fixes.
- Do not call `lf pr land`, spawn another watcher, or wait for merge. Return
  after the repaired head is published and armed; the existing watcher resumes.
- If the repair cannot be verified or published, report the blocker and stop.
