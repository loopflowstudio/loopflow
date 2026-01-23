# Project 4: Atomic UI Updates

Eliminate multi-pass rendering. Single update to worktrees array.

**Status:** Future (after Projects 2-3)

---

## Problem

Concerto updates `worktrees` array multiple times during load:

```swift
// Pass 1: Basic list
worktrees = await worktreeService.list(full: false)  // UI renders

// Pass 2: Full list with sessions
worktrees = await worktreeService.list(full: true)   // UI re-renders

// Pass 3: Staleness
for i in worktrees.indices {
    worktrees[i].staleness = ...                      // UI re-renders per worktree
}

// Pass 4: CI status
for branch in branches {
    ciStatus[branch] = ...                            // UI re-renders per branch
}
```

Each assignment to `@Published var worktrees` triggers SwiftUI re-render. With 5 worktrees and 4 passes, that's 20+ renders on initial load.

---

## Solution

Gather all data, then update once.

### Atomic Update Pattern

```swift
func loadWorktrees() async {
    // Gather everything first
    let response = try await lfdService.worktreesList(repo: repoURL)

    // Single atomic update
    await MainActor.run {
        self.worktrees = response.worktrees
    }
}
```

With Projects 2-3 complete, lfd provides complete status in one response. No multi-pass needed.

### For Event-Based Updates

Events update individual worktrees. Use copy-on-write to minimize re-renders:

```swift
func handleWorktreeUpdate(_ updated: Worktree) {
    // Find and replace in single operation
    if let index = worktrees.firstIndex(where: { $0.branch == updated.branch }) {
        // Copy array, modify, assign back
        var newWorktrees = worktrees
        newWorktrees[index] = updated
        worktrees = newWorktrees  // Single publish
    }
}
```

Or use SwiftUI's `@Observable` (iOS 17+/macOS 14+) which has finer-grained tracking.

---

## Anti-Patterns to Remove

### 1. Staged Loading

```swift
// Bad: Two fetches, two renders
let basic = await list(full: false)
worktrees = basic                    // render
let full = await list(full: true)
worktrees = full                     // render again

// Good: One fetch, one render
let full = await lfd.worktreesList()
worktrees = full
```

### 2. In-Place Mutation

```swift
// Bad: Mutating published array triggers render per mutation
for i in worktrees.indices {
    worktrees[i].staleness = calculateStaleness(worktrees[i])
}

// Good: Calculate first, assign once
let enriched = await enrichAll(worktrees)
worktrees = enriched
```

### 3. Separate State for Related Data

```swift
// Bad: Two published properties that update at different times
@Published var worktrees: [Worktree] = []
@Published var ciStatus: [String: CIStatus] = [:]  // Updates separately

// Good: CI status is part of Worktree
@Published var worktrees: [Worktree] = []  // Worktree.ci is populated
```

---

## Implementation

### Phase 1: Remove staged loading

Once lfd provides full status (Project 2):

1. Remove `full: false` initial load
2. Single load from lfd with all status
3. Remove `syncAndEnrich()` background task

### Phase 2: Consolidate related state

1. Move `ciStatus` dict into `Worktree.ci`
2. Move `stalenessMap` into `Worktree.staleness`
3. Remove separate published properties

### Phase 3: Batch event updates

For rapid events (multiple commits in quick succession):

1. Collect events for 100ms
2. Apply all updates at once
3. Single UI render for batch

---

## Files to Modify

**Swift (Concerto):**
- `swift/Concerto/AppState.swift` — Remove multi-pass loading
- `swift/LoopflowCore/Models/Worktree.swift` — Ensure all status fields present
- `swift/Concerto/Views/WorktreeSidebar.swift` — Simplify data binding

---

## Verification

### Render Count Test

Add logging to verify render count:

```swift
var body: some View {
    let _ = print("WorktreeSidebar render")  // Should print once per actual change
    ...
}
```

Before: 20+ renders on load
After: 1-2 renders on load

### Performance Profile

Use Instruments to verify:
- No layout thrashing
- Minimal view invalidation
- Smooth 60fps scrolling

---

## Done When

- [ ] Initial load: single render after data arrives
- [ ] Event updates: single render per event batch
- [ ] No visible flicker during load or updates
- [ ] Render count matches actual data changes
- [ ] Smooth scrolling in worktree list
