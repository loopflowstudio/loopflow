# Questions from debug session

## lfops land --create-pr deleted worktree without merging new changes

**Analysis complete. Root cause identified and already fixed on main.**

### What happened

1. User had an old `landing` branch that created PR #38
2. PR #38 was successfully merged to main (commit 6ffa690)
3. User created a NEW `landing` branch with new changes
4. User ran `lfops land --create-pr`
5. `gh pr view` found the OLD merged PR #38 (same branch name)
6. The code didn't check if the PR was still OPEN
7. Merge of already-merged PR was a no-op but returned success
8. Worktree was deleted, new changes lost

### Fix already on main

The fix is already in `src/loopflow/lfops.py` lines 949-961:
- Now checks `state == "OPEN"` before using a found PR
- If PR is closed/merged, treats it as "no PR found"

### Open questions

1. **Recovery:** Were there changes in the deleted landing branch that need to be recreated? The reflog might have them if git gc hasn't run yet.

2. **Bootstrap problem:** How do we prevent this when landing the fix itself? (Already resolved since fix is on main now.)
