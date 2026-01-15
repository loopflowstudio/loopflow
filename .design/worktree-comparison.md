# Worktree Comparison in Maestro

Compare two worktrees side-by-side in Maestro's UI.

## What This Extends

The `worktreediffs` branch added:
- View diff against main (single worktree)
- GitHub compare URL integration
- DiffSheet modal with syntax-highlighted diff

This expansion adds:
- Compare two worktrees (A vs B)
- Context menu integration for comparison
- CompareSheet modal reusing DiffContentView

## User Flow

1. Right-click any worktree in the sidebar
2. Select "Compare with..." → choose another worktree
3. CompareSheet opens showing the diff between the two branches

## Implementation

### WorktreeService.swift

Added:
```swift
func getDiffBetween(branchA: String, branchB: String, in repoURL: URL) async throws -> String
```

Uses `git diff branchA...branchB` (symmetric difference).

### WorktreeSidebar.swift

State additions:
- `showingCompareSheet`, `compareWorktrees`, `compareContent`, `compareLoading`

WorktreeRow additions:
- `otherWorktrees: [Worktree]` - list of worktrees to compare with
- `onCompareWith: (Worktree) -> Void` - callback when comparison selected
- "Compare with..." submenu in context menu

### CompareSheet

New view showing comparison header and diff content:
```swift
struct CompareSheet: View {
    let worktreeA: Worktree
    let worktreeB: Worktree
    let diffContent: String?
    let isLoading: Bool
}
```

Reuses DiffContentView for syntax-highlighted rendering.

## What's Not Included

- Side-by-side split view (just unified diff)
- File-by-file navigation
- LLM analysis integration (use `lfwt compare` CLI for that)
- Cmd-click interaction (requires more complex selection state)
