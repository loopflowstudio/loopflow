# Land Button as Primary Action

Land is the primary shipping action in Maestro's worktree detail panel. Draft PRs are auto-created by lfd so a PR typically exists when there's work to land.

## Implementation

**Quick actions bar:**
```
[Land]  [Cursor]  [Warp]      [View PR*]  [Abandon]
                              * only when prNumber != nil
```

**Land button:**
- First position (primary action)
- Calls `lfops land --create-pr` (creates PR if needed, rebases, marks ready, enables auto-merge)
- Disabled when `hasDiff == false`

**View PR button:**
- Only visible when `prNumber != nil`
- Disabled when `hasDiff == false`
- Opens PR URL in browser

## Data Structures

`hasDiff` in Worktree model (computed from `diff_vs_main` JSON):

```swift
// Worktree.swift
let hasDiff: Bool  // True if diff_vs_main has changes

// In init(from json:)
let diffStats = json.workingTree?.diffVsMain
self.hasDiff = (diffStats?.added ?? 0) + (diffStats?.deleted ?? 0) > 0
```

## lfd Auto-Draft PRs

`src/loopflow/lfd/draft_prs.py` runs on the scheduler tick (every 30s). Creates draft PRs for branches that:
- Are not main/master
- Don't already have a PR
- Have commits ahead of main
- Have line changes
- Are pushed to origin
