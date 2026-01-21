---
status: proposed
area: infra
---

# Enable GitHub Merge Queue

Enable merge queue to gate main branch with rebased-CI verification. The CI workflow already triggers on `merge_group:` but the repo doesn't have merge queue enabled.

## Scope

- Enable merge queue in GitHub repo settings (branch protection rules)
- Verify CI workflow runs correctly in merge queue context
- Update `lfops land` to submit to merge queue instead of direct merge

Not included:
- Custom merge strategies (use default linear)
- Status checks beyond existing pytest + swift test

## Approach

1. Branch protection rule on `main`:
   - Require status checks: `loopflow-test`, `maestro-test`, `maestro-ui-test`
   - Enable merge queue with default settings
   - Require branches to be up to date before merging

2. Verify `lfops land` works with merge queue:
   - `gh pr merge --merge-queue` instead of direct merge
   - Handle case where merge queue is not enabled (fallback to current behavior)

3. Document in README or `lfops land --help`

This is table stakes for teams (per teams-vision.md) and unblocks Orchestra.
