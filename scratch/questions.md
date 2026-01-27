# Open Questions

## Worktree Stacking

1. **wt list output**: The design doc shows `lfops wt list` displaying a BASE column. This requires the worktrunk (`wt`) tool to include `base_branch` in its JSON output. If worktrunk doesn't already include this, it would need to be updated. The loopflow code is ready to parse and display it.

2. **wt sync command**: Not implemented. Design doc mentions `lfops wt sync` to handle rebasing stacked branches when base changes or when base squash-merges. This is deferred to a follow-up.

3. **wt unstack command**: Not implemented. Design doc mentions `lfops wt unstack` to absorb base branch and target main directly. Deferred to follow-up.

4. **base_commit persistence**: The `base_commit` is recorded when creating a stacked worktree, but it's unclear if worktrunk persists this in its metadata. If not, the base_commit would be lost on restart. The code captures it but may need worktrunk changes to persist it.
