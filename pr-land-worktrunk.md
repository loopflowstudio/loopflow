# Exploration: lf pr land + worktrunk

How should `lf pr land` adapt now that worktrunk handles worktree lifecycle?

## Current state

`lf pr land` does a lot:
1. Check for uncommitted changes, optionally commit/push
2. Get PR info (title, body, base branch) from GitHub
3. Checkout base branch in main repo
4. Squash merge the feature branch
5. Remove `<branch>.md` design doc
6. Commit with PR title + body as message
7. Push to origin
8. Remove worktree and branch

`wt merge` does similar things:
1. Squash all commits into one (with backup)
2. Rebase onto target if behind
3. Run pre-merge hooks
4. Fast-forward merge
5. Run pre-remove hooks
6. Remove worktree and branch
7. Run post-merge hooks

## Key differences

| Aspect | lf pr land | wt merge |
|--------|-----------|----------|
| Commit message | PR title + body | Squashed commit message |
| Requires PR | Yes | No |
| Design doc cleanup | Yes (`<branch>.md`) | No |
| Runs from | Main repo or worktree | Worktree only |
| Target branch | PR's base branch | Default branch (or arg) |
| Hooks | No | Pre-merge, pre-remove, post-merge |

## Options

### Option A: Delegate to wt merge

Replace most of `lf pr land` with `wt merge`:

```python
def land():
    # Get PR info for commit message
    pr_data = get_pr_info()

    # Remove design doc
    remove_design_doc(branch)

    # Delegate to worktrunk
    subprocess.run(["wt", "merge", pr_data["baseRefName"]])
```

**Problem:** `wt merge` creates its own commit message from the squashed commits. We want the PR title/body as the commit message.

**Workaround:** Use `--no-commit` and commit manually:
```python
subprocess.run(["wt", "merge", "--no-commit", "--no-remove", base])
subprocess.run(["git", "commit", "-m", pr_title_and_body])
subprocess.run(["wt", "remove", "-y"])
```

This is awkward—we're fighting worktrunk's workflow.

### Option B: Keep lf pr land separate

`lf pr land` is PR-centric. `wt merge` is local-first. They serve different workflows:

- **lf pr land**: "I have a PR. Merge it using the PR's metadata."
- **wt merge**: "I have local changes. Merge them without needing a PR."

Keep them as separate tools for separate needs.

**Benefit:** Users who don't use GitHub PRs can still use `wt merge`. Users who do use PRs get `lf pr land` with PR-aware commit messages.

### Option C: Enhance wt merge via hooks

Use worktrunk's hook system to inject lf behavior:

```toml
# .config/wt.toml
[hooks.pre-merge]
lf-pr-land = """
# Fetch PR title/body and set GIT_COMMIT_MSG
export GIT_COMMIT_MSG=$(lf pr message)
# Remove design doc
rm -f ${BRANCH}.md
"""
```

**Problem:** Hooks can't easily modify commit message. Would need worktrunk to support `$GIT_COMMIT_MSG` or similar.

### Option D: Add wt merge --pr flag

Request worktrunk add `--pr` flag that:
1. Fetches PR title/body via `gh pr view`
2. Uses PR metadata as commit message
3. Closes PR after merge

This would let loopflow retire `lf pr land` entirely.

## Recommendation

**Option B: Keep them separate.**

Rationale:
- `lf pr land` works well today
- It serves a different workflow than `wt merge`
- No awkward workarounds needed
- Users can choose: `wt merge` for local-first, `lf pr land` for PR-first

The only change needed: ensure `lf pr land` uses worktrunk for worktree removal (already done via `remove_worktree()`).

## Future consideration

If worktrunk adds PR-aware merging (`wt merge --pr`), revisit this decision. That would let us deprecate `lf pr land` entirely.

## What about wt merge for non-PR workflows?

Some users may want to land branches without PRs. Currently loopflow doesn't support this—`lf pr land` requires a PR.

Options:
1. Tell users to use `wt merge` directly (already works)
2. Add `lf land` (no "pr") that wraps `wt merge`
3. Make `lf pr land` work without PR (generate commit message from diff)

Recommendation: Option 1. Users who don't want PRs can use `wt merge` directly. No need to wrap it.

## Sources

- [wt merge documentation](https://worktrunk.dev/merge/)
- [worktrunk GitHub](https://github.com/max-sixty/worktrunk)
