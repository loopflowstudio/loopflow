---
status: proposed
---

# Worktree Browser

Sidebar section showing existing git worktrees. Browse what's on disk, upgrade to waves.

## What to build

Concerto's sidebar gains a worktree section — shows every `git worktree` on disk, whether it's tracked by a wave or not. Orphaned worktrees from previous work become visible and adoptable.

## Approach

### Rust: `GET /worktrees` endpoint

Calls existing `list_worktrees()` from `engine/worktrees.rs`. Cross-references wave store for `has_wave`.

```json
[
  {
    "branch": "feature-auth",
    "path": "/Users/jack/src/loopflow.feature-auth",
    "merged": false,
    "prunable": false,
    "has_wave": false
  }
]
```

### Swift/LoopflowCore: model + service

```swift
public struct WorktreeInfo: Sendable, Identifiable {
    public let branch: String?
    public let path: String
    public let merged: Bool
    public let prunable: Bool
    public let hasWave: Bool
    public var id: String { path }
}
```

New `listWorktrees()` method on `LocalWaveService`.

### Swift/Concerto: sidebar worktree list

Permanent section in sidebar (below waves when they exist, or as empty state).

- Branch name (primary), path (secondary, dimmed)
- Merged worktrees dimmed
- "Upgrade to wave" action per row
- `POST /waves` already accepts a `worktree` field — verify it works for adopting existing worktrees

## Done when

- `GET /worktrees` returns list with wave association
- Sidebar shows worktrees not tracked by waves
- Upgrade creates wave from existing worktree
- Merged/prunable worktrees visually distinct
