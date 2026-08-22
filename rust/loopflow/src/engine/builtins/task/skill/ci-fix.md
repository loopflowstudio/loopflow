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
   - If the prompt message already includes check metadata (PR number, branch, commit SHA, logs URL), use it directly.
   - Otherwise, resolve the PR for the current branch with the GitHub CLI:
     ```bash
     gh pr view --json number,headRefName,headRefOid,url
     ```
   - If there is no PR, stop and explain what is missing.

2. **Fetch latest check-run failures**
   - Query checks for the PR head SHA (from `headRefOid`):
     ```bash
     repo=$(gh repo view --json nameWithOwner -q .nameWithOwner)
     sha=$(gh pr view <pr> --json headRefOid -q .headRefOid)
     gh api "repos/$repo/commits/$sha/check-runs"
     ```
   - Focus on failed checks from the most recent completed runs.
   - Use the check `html_url`/logs link for repro details.

3. **Fix one failing check at a time**
   - Reproduce the failure locally from the current branch.
   - Apply the smallest correct fix.
   - Run only the relevant local checks first, then broader checks if needed.
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
- Do not push, land, or merge. If you cannot repair the head, say so and stop. A
  material tree change is the only signal that lets the landing supervisor
  publish, re-arm a new head, and continue watching.

## Adaptation

After fixing the immediate failure, ask: why did this get to CI? Could gate have caught it? If the answer points to a missing check in gate, a missing convention in repo docs, or a recurring ci-fix pattern — make that update. The fix addresses the symptom; the step or doc update prevents recurrence.
