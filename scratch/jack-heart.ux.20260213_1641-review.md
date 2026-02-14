# Worktree Browser — Design Review

## What was implemented

Sidebar section "On Disk" showing git worktrees that exist on the filesystem but aren't tracked by any wave. Users can upgrade orphan worktrees to waves with one click, and see which worktrees have been merged.

**Rust:** `GET /worktrees?repo=<path>` endpoint cross-references `list_worktrees()` with the wave store to populate `wave_id` on each worktree DTO. Filters out the main repo worktree.

**Swift/LoopflowCore:** `WorktreeInfo` model with `shortName` (extracts name from worktree directory convention), `directoryName`, `hasWave`. `listWorktrees()` on `LocalWaveService` + protocol method.

**Swift/Concerto:** `WorktreeStore` (thin `@Observable` store, `orphans` computed property). `WorktreeRow` view with hover-reveal "Upgrade" button, merged strikethrough, context menu (Finder/Warp). `WaveSidebar` gains "On Disk" section below wave groups.

**Tests:** `WorktreeStoreTests` and `WorktreeInfoTests` — 11 tests covering store operations and model computed properties.

## Key choices

| Decision | Why |
|----------|-----|
| `wave_id` instead of `has_wave` boolean | Enables future click-to-navigate from worktree to its wave. No extra cost. |
| Orphans only in UI | Wave-tracked worktrees already appear as waves. Showing them twice adds noise. |
| "On Disk" not "Worktrees" | Avoids git jargon. Communicates what's there — things on your filesystem. |
| Refresh on wave create/delete events | No dedicated worktree WebSocket event. Piggybacks on existing wave events since those are the actions that change worktree↔wave associations. |
| `spawn_blocking` for `list_worktrees()` | The engine function spawns threads for merge/PR checks. Keeps the async handler non-blocking. |
| Naming convention as linkage | "Upgrade" just calls `createWave(name: shortName)` — `ensure_wave_worktree()` already reuses existing directories that match the naming convention. No new API parameters needed. |

## How it fits together

```
WaveSidebar → reads WorktreeStore.orphans
    ↑
RepoState → owns WorktreeStore, calls refreshWorktrees() on:
    - connected event (initial load)
    - wave_created event (may adopt a worktree)
    - wave_deleted event (may orphan a worktree)
    ↑
LocalWaveService.listWorktrees() → GET /worktrees?repo=...
    ↑
routes/worktrees.rs → list_worktrees() + wave name→ID cross-reference
```

## Risks and bottlenecks

- **`list_worktrees()` performance:** Spawns threads for squash-merge and PR checks per worktree. Called once on connect and on wave create/delete events. Fine for typical repos (<20 worktrees). Could become noticeable with many worktrees + slow GitHub API. Design doc notes a lightweight variant as a future optimization.
- **Naming convention fragility:** `shortName` extraction assumes `repo.branch-name` directory pattern. Worktrees created outside this convention won't match. Acceptable since this matches existing loopflow behavior.
- **No WebSocket events for worktree changes:** Worktrees created/deleted outside of wave operations (e.g. `git worktree add` in terminal) won't appear until the next wave event or reconnection. Low risk since the primary workflow goes through Concerto.

## What's not included

- Deleting/pruning worktrees from UI (destructive, needs its own design)
- Worktree detail view
- Dedicated WebSocket events for worktree changes
- Lightweight `list_worktrees()` variant (optimize later if needed)
- Keyboard navigation into the worktree section

## Gate polish applied

- Fixed empty-state logic: orphan worktrees now show even when zero waves exist (was hidden behind the "No waves yet" empty state)
- Terminal app changed from `.terminal` to `.warp` to match the rest of the codebase
- Replaced magic spacing numbers with `Spacing` tokens where tokens exist (`Spacing.sm`, `Spacing.xxs`, `Spacing.md`)
- All tests pass (107 Swift, all Rust)
- `cargo fmt` and `cargo clippy` clean
- No TODOs, debug prints, or dead code in changed files
