# Worktree Browser

## Problem

Worktrees accumulate on disk. Waves create them, but not every worktree has a wave — previous manual work, abandoned experiments, worktrees from `lf ops` outside Concerto. These orphans are invisible. You discover them by `ls`-ing the parent directory. Merged branches sit there forever because nothing tells you they're done.

Concerto should show what's on disk. Every worktree, whether tracked by a wave or not. Merged ones are visually finished. Orphaned ones get a one-click path to becoming a wave.

## Approach

### Rust: `GET /v0/worktrees?repo=<path>`

New endpoint in `routes/worktrees.rs`. Calls `list_worktrees()` from `engine/worktrees.rs`, cross-references wave names from the store.

**DTO:**

```rust
#[derive(Debug, Serialize)]
pub struct WorktreeDto {
    pub branch: Option<String>,
    pub path: String,
    pub merged: bool,
    pub prunable: bool,
    pub wave_id: Option<String>,  // not just has_wave — need the ID for navigation
}
```

`wave_id` instead of `has_wave` boolean. The UI needs to know *which* wave owns a worktree so clicking it navigates to the wave. Cross-reference logic: `worktree_short_name(path)` → compare against wave names in repo → return wave ID if matched.

Filter out the main repo worktree (no branch, it's not interesting).

**Performance note:** `list_worktrees()` is expensive — it spawns threads for squash-merge and PR checks. For the sidebar, we might want a lightweight version that skips merge detection and just lists what's on disk. But start with the full version; optimize only if it's noticeably slow. The endpoint is called once on connect, not on every keystroke.

### Swift/LoopflowCore: model

```swift
public struct WorktreeInfo: Sendable, Identifiable, Hashable {
    public let branch: String?
    public let path: String
    public let merged: Bool
    public let prunable: Bool
    public let waveId: String?

    public var id: String { path }
    public var hasWave: Bool { waveId != nil }
}
```

New `listWorktrees()` method on `LocalWaveService`. Simple GET, parse JSON array.

### Swift/Concerto: WorktreeStore + RepoState integration

Thin `@Observable` store following the WaveStore pattern:

```swift
@Observable
final class WorktreeStore {
    private(set) var worktrees: [WorktreeInfo] = []

    /// Only orphans — worktrees not tracked by any wave.
    var orphans: [WorktreeInfo] {
        worktrees.filter { !$0.hasWave }
    }

    func setAll(_ items: [WorktreeInfo]) {
        worktrees = items
    }
}
```

RepoState owns the store, loads on connect (alongside `refreshWaves()`), refreshes on `wave_created` / `wave_deleted` events (a new wave might adopt a worktree; deleting one might orphan it).

### Swift/Concerto: sidebar section

New section at the bottom of `waveList`, below Idle. Only shows when there are orphan worktrees.

```
┌─────────────────────────────┐
│ ▽ Needs Attention (2)       │
│   broken-auth               │
│   stale-migration           │
│ ▽ Active (3)                │
│   ...                       │
│ ▽ Idle (1)                  │
│   ...                       │
│ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ │
│ ▽ On Disk (3)               │  ← new section
│   feature-auth     Upgrade  │
│   old-experiment   Upgrade  │
│   cleanup-v2       ✓ merged │
└─────────────────────────────┘
```

**Section design:**
- Header: "On Disk" with `folder` icon and count of orphans
- Only visible when orphan worktrees exist
- Separated from wave sections by a divider or extra spacing to signal "these are different"

**Row design (`WorktreeRow`):**
- **Primary:** Branch name (or "detached" if none). Use `body` typography.
- **Secondary:** Relative path, dimmed (`textSecondary`, `caption` typography). Show just the directory name, not full absolute path — e.g. `loopflow.feature-auth` not `/Users/jack/src/loopflow.feature-auth`.
- **Merged state:** Entire row dimmed. Branch name gets strikethrough. "merged" label in `statusNeutral`.
- **Action:** "Upgrade" button on hover/focus. Calls `createWave(name: shortName)` — the existing create flow already adopts the worktree if the path matches.

**Interaction:**
- Click row → no navigation (these aren't waves). The "Upgrade" action is the primary interaction.
- After upgrading, the worktree disappears from "On Disk" (it now has a wave) and appears in the wave sections. The new wave auto-selects.
- Context menu: "Open in Finder", "Open in Terminal" using the worktree path.

### Wave creation: adopting existing worktrees

The existing `ensure_wave_worktree()` in executor.rs already handles this: if `worktree_path(repo, wave_name)` exists, it reuses it rather than creating a new one. The "Upgrade" action just calls `createWave(name: shortName)` — no special worktree parameter needed.

The key insight: **naming convention is the linkage**. A wave named `feature-auth` expects a worktree at `../loopflow.feature-auth`. If that directory already exists, `ensure_wave_worktree()` reuses it. This means upgrade is just wave creation with the right name.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Separate worktree management view (not sidebar) | Cleaner separation, but hidden | Worktrees are contextual to the repo — they belong in the sidebar where you see them alongside waves |
| Show all worktrees including wave-tracked ones | Complete picture | Redundant. Wave-tracked worktrees already appear in the wave list. Showing them twice adds noise without information. |
| `has_wave: bool` instead of `wave_id` | Simpler DTO | Loses the ability to navigate from a worktree to its wave. Costs nothing to include the ID. |
| Add `worktree` field to `POST /waves` | Explicit adoption | Unnecessary — naming convention already handles it. Adding a field creates two paths to the same outcome. |
| Load worktrees lazily / on expand | Less work on connect | The list is small (typically <20 items) and the endpoint is called once. Lazy loading adds complexity for no perceptible benefit. |

## Key decisions

1. **"On Disk" section, not "Worktrees."** The word "worktree" is git jargon. "On Disk" communicates what this section shows — things that exist on your filesystem but aren't waves yet.

2. **Orphans only.** Wave-tracked worktrees already appear as waves. Showing them again adds noise. The store tracks all worktrees (for cross-referencing), but the UI only shows orphans.

3. **`wave_id` over `has_wave`.** Small cost, enables future navigation (click an "On Disk" item with a wave to jump to that wave).

4. **No delete/prune action yet.** The design doc mentions merged/prunable worktrees. Showing their state is useful (you can see what's done). But deleting worktrees from the UI is a destructive action that warrants its own design. Keep it out of scope.

5. **Refresh on wave events.** Worktree list changes when waves are created/deleted. Rather than a dedicated WebSocket event, piggyback on existing wave events to refresh.

Following UX wave principles: in-memory only (WorktreeStore holds no persistent state), single source of truth (store is canonical, views read from it), simple (no optimistic patterns needed — worktree listing is read-only).

## Scope

**In scope:**
- `GET /v0/worktrees` endpoint with wave cross-reference
- `WorktreeInfo` model and `listWorktrees()` service method
- `WorktreeStore` with `orphans` computed property
- "On Disk" sidebar section showing orphan worktrees
- "Upgrade" action (creates wave from existing worktree)
- Merged/prunable visual distinction
- Context menu: Open in Finder / Terminal

**Out of scope:**
- Deleting/pruning worktrees from UI
- Worktree detail view
- WebSocket events for worktree changes
- Lightweight `list_worktrees()` variant (optimize later)
- Keyboard navigation into worktree section (follow-up, after basic functionality works)

## Done when

- `GET /v0/worktrees?repo=<path>` returns worktrees with `wave_id` cross-reference
- Sidebar shows "On Disk" section with orphan worktrees
- Merged worktrees are visually dimmed with strikethrough
- "Upgrade" creates a wave that adopts the existing worktree
- After upgrade, worktree moves from "On Disk" to wave sections
- Context menu offers Open in Finder / Terminal
