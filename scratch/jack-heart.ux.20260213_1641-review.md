# Worktree Browser — Design Review

## What was implemented

Sidebar section showing git worktrees on disk that aren't tracked by any wave. Users can see orphaned worktrees and upgrade them to waves with one click.

**Rust:** `GET /worktrees?repo=<path>` endpoint. Lists worktrees via `list_worktrees()`, cross-references with waves by matching worktree short names to wave names, returns `WorktreeDto` with optional `wave_id`. Filters out the main repo worktree.

**Swift/LoopflowCore:** `WorktreeInfo` model with `hasWave`, `directoryName`, `shortName` computed properties. `listWorktrees(repo:)` on `LocalWaveService` and `WaveServiceProtocol`.

**Swift/Concerto:** `WorktreeStore` — `@Observable` store with `orphans` computed property. `WorktreeRow` view with hover-to-upgrade button, merged state, context menu (Finder, Warp). "On Disk" section in `WaveSidebar` below wave groups, separated by divider.

**RepoState integration:** `refreshWorktrees()` called on connect, wave create, and wave delete events.

## Key choices

**Wave name matching for upgrade.** Upgrade calls `createWave(name: shortName)` — the daemon discovers the matching worktree on disk by convention (worktree dir = `{repo}.{waveName}`). No new `worktree` field on `CreateWaveRequest` needed.

**Refresh on events, not polling.** Worktrees refresh on connect + wave create/delete. No timer-based polling. Worktrees don't change frequently enough to warrant it.

**Orphans only.** The "On Disk" section only shows worktrees not tracked by a wave. Wave-tracked worktrees are already visible in the wave list. No duplication.

**Empty state condition.** `emptyState` only shows when both `waves.isEmpty` and `orphans.isEmpty`. If orphaned worktrees exist, the wave list renders (with just the "On Disk" section), giving users a path to upgrade rather than an unhelpful empty state.

## How it fits together

```
Sidebar → WorktreeStore.orphans → WorktreeRow (per orphan)
                ↑
RepoState.refreshWorktrees() → LocalWaveService.listWorktrees() → GET /worktrees
                ↑
Event triggers: connect, wave.created, wave.deleted
```

`WorktreeStore` follows the same pattern as `WaveStore` and `RunStore` — `@Observable`, owned by `RepoState`, populated via service calls.

## Risks and bottlenecks

**`list_worktrees` is blocking.** It runs git commands and spawns threads for merge checks. Wrapped in `spawn_blocking` on the Rust side. For repos with many worktrees this could be slow, but single-user local daemon makes this acceptable.

**Wave↔worktree matching is name-based.** If someone renames a wave after creation, the worktree directory doesn't rename (it follows the original branch name). This is an existing limitation of the worktree system, not introduced by this change.

**No worktree event stream.** Worktrees refresh reactively (on wave events) rather than having their own WebSocket events. If a worktree is created outside Concerto (e.g. `git worktree add`), it won't appear until the next wave event or reconnect. This is intentional — a worktree event type would be a separate piece of work.

## What's not included

- Worktree deletion/pruning from the UI
- Worktree events over WebSocket (would need new event type in `ws.rs`)
- Sorting/filtering orphan worktrees
- Expanding `CreateWaveRequest` with a `worktree` field for explicit adoption
