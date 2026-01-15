# Worktreediffs

Adds diff viewing and worktree comparison to Maestro's sidebar.

## Review

**Verdict:** Ready to ship

Clean implementation. The code adds useful features without over-engineering:

- `getDiff` for fetching git diffs (handles both single-branch and comparison cases)
- `getGitHubCompareURL` for opening comparisons in browser
- `DiffSheet` and `CompareSheet` modals with syntax-highlighted rendering
- Context menu integration for "Compare with..." submenu
- Hover actions for quick access to diff viewer and PR actions

The `findCommand` refactor consolidates duplicate `findWt`/`findLfpr` helpers into a single generic function.

## Design notes

The `git diff branchA...branchB` syntax shows symmetric difference (commits reachable from B but not A). This matches GitHub's comparison behavior.

`DiffContentView` is reused between `DiffSheet` and `CompareSheet`, avoiding duplication.

Not included (intentionally): side-by-side split view, file-by-file navigation, LLM analysis integration. The CLI (`lfwt compare`) handles the LLM analysis case.
