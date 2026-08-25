---
requires: watched PR landing with failing CI checks (or CI failure message context)
produces: branch with CI failures fixed
diff_files: false
action_style: procedural
---
Fix failing CI checks for the watched PR landing.

## Goal

Use the watched PR context, identify the failing checks on its exact head, fix
them, and leave the branch ready for the landing supervisor to publish.

## Workflow

1. **Resolve the watched PR**
   - If the prompt includes `Hosted failure evidence fetched from the exact
     check URLs`, use that evidence directly. The landing supervisor has
     already bound it to the recorded head.
   - Otherwise, if the prompt includes check metadata (PR number, branch,
     commit SHA, logs URL), use it directly.
   - Otherwise, resolve the PR for the current branch with the GitHub CLI:
     ```bash
     gh pr view --json number,headRefName,headRefOid,url
     ```
   - If there is no PR, stop and explain what is missing.

2. **Fetch latest check-run failures only when evidence was not supplied**
   - Query checks for the PR head SHA (from `headRefOid`):
     ```bash
     repo=$(gh repo view --json nameWithOwner -q .nameWithOwner)
     sha=$(gh pr view <pr> --json headRefOid -q .headRefOid)
     gh api "repos/$repo/commits/$sha/check-runs"
     ```
   - Focus on failed checks from the most recent completed runs.
   - Use the check `html_url`/logs link for repro details.

3. **Fix one failing check at a time**
   - Derive the smallest test selector named by the hosted failure and run it
     first. Do not begin with a repository-wide suite.
   - Reproduce the failure locally from the current branch. This launch has no
     ambient Loopflow Run, Home, or writer authority; keep local repro state
     isolated rather than discovering or restoring the parent Run context.
   - Apply the smallest correct fix.
   - Run only the relevant local checks. The hosted gate reruns the full suite
     after the landing supervisor publishes the repair.
   - Repeat until failing checks for the head SHA are addressed or a real blocker remains.

4. **Verify and report**
   - Summarize what failed, what was changed, and what commands were run.
   - If no failing checks remain, say so clearly.
   - If blocked (missing secrets, flaky upstream, infra outage), give exact
     blocker details and the next manual action. Name the failing capability
     (provider, github-observation, secrets); the landing supervisor records
     that durable blocker.

## Guardrails

- Stay scoped to CI failures on this PR.
- Prefer targeted fixes over broad refactors.
- Do not ignore failing tests to get green.
- Do not commit, push, land, or merge. If you cannot repair the head, say so and
  stop. A material tree change is the only signal that lets the landing
  supervisor publish, re-arm a new head, and continue watching.

## Adaptation

After fixing the immediate failure, ask: why did this get to CI? Could gate have caught it? If the answer points to a missing check in gate, a missing convention in repo docs, or a recurring ci-fix pattern — make that update. The fix addresses the symptom; the step or doc update prevents recurrence.
