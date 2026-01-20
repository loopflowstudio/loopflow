# Fix: Don't prune new/empty worktrees

**Status:** Implemented. See `WorktreeService.swift:347-359`.

## The Bug

`WorktreeService.detectStaleness()` marks worktrees as `.merged` if `git diff --quiet base...branch` succeeds. For a brand new worktree created from main, this diff is empty (no changes), so the worktree gets marked as merged and auto-pruned.

```swift
// WorktreeService.swift:347-356
if (try? await runProcess(..., ["diff", "--quiet", "\(baseBranch)...\(worktree.branch)"], ...)) != nil {
    return .merged  // BUG: fires for new worktrees with no commits
}
```

## Data Structures

The `Worktree` model already has `aheadMain: Int` which counts commits ahead of main. This is 0 for new worktrees.

```swift
// Worktree.swift:52
let aheadMain: Int
```

## Key Functions

```swift
// WorktreeService.swift
func detectStaleness(for worktree: Worktree, in repoURL: URL) async -> Staleness
```

This is the only function that needs to change.

## Constraints

- Must not change the Worktree data model
- Must preserve existing behavior for actually-merged worktrees (commits that got squash-merged or rebased onto main)
- The `aheadMain` value is already populated by the time `detectStaleness` runs

## The Fix

Before returning `.merged` from the diff check, verify the worktree has commits:

```swift
// WorktreeService.swift:347-356 - BEFORE
if (try? await runProcess(
    URL(fileURLWithPath: "/usr/bin/git"),
    ["diff", "--quiet", "\(baseBranch)...\(worktree.branch)"],
    in: repoURL
)) != nil {
    return .merged
}

// AFTER
if worktree.aheadMain > 0,
   (try? await runProcess(
    URL(fileURLWithPath: "/usr/bin/git"),
    ["diff", "--quiet", "\(baseBranch)...\(worktree.branch)"],
    in: repoURL
)) != nil {
    return .merged
}
```

**Why this works:** A worktree with `aheadMain == 0` has no unique commits. If it also has no diff from base, it's not "merged" - it's just new. Only worktrees that had commits (`aheadMain > 0`) and now show no diff should be considered merged (the commits got squash-merged or rebased).

## Verification

1. Create a new worktree: `wt switch --create test-empty`
2. Open Maestro, wait for staleness detection
3. Verify worktree shows `.active` staleness, not `.merged`
4. Worktree is NOT auto-pruned
