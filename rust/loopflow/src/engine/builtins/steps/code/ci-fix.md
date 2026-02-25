---
requires: wave PR with failing CI checks (or CI failure message context)
produces: branch with CI failures fixed
diff_files: false
action_style: procedural
---
Fix failing CI checks for the current wave PR.

## Goal

Find the wave's current PR, identify the latest failing checks on its head commit, fix them, and leave the branch ready to push.

## Workflow

1. **Resolve the PR for this wave**
   - If the prompt message already includes check metadata (PR number, branch, commit SHA, logs URL), use it directly.
   - Otherwise, identify the wave from the current worktree, then get its PR number.
   - Prefer loopflow metadata first:
     ```bash
     cwd=$(pwd -P)
     lfq list --json | jq -r --arg cwd "$cwd" '
       .[]
       | .worktree = (.active_run.local_worktree // .local_worktree // "")
       | select(.worktree != "" and ($cwd | startswith(.worktree)))
       | [.name, (.active_run.pr.number // ""), .worktree]
       | @tsv
     ' | sort -k3 | tail -1
     ```
   - If wave metadata is unavailable, fall back to GitHub CLI for the current branch:
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
   - If blocked (missing secrets, flaky upstream, infra outage), give exact blocker details and the next manual action.

## Guardrails

- Stay scoped to CI failures on this PR.
- Prefer targeted fixes over broad refactors.
- Do not ignore failing tests to get green.
