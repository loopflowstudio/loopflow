# Wave Directory Rename + Worktree Browser — Design Review

## What was implemented

Two scoped changes shipped together:

1. **Roadmap rename:** The execution-spec directory is now `wave/` instead of `roadmap/`, including prompt logic, built-in step/flow names, docs, tests, and fixtures.
2. **Worktree browser:** Concerto now shows orphaned git worktrees (“On Disk”) and lets users adopt one into a wave with one click.

**Rust (rename):** `gather_docs()` now reads `wave/` docs, builtins registry/flow names were renamed (`wave-plan`, `add-to-wave`, `wave-reduce`, etc.), and tests/goldens were updated for the new naming.

**Rust (worktrees):** Added `GET /worktrees?repo=<path>` in lfd. It lists worktrees, matches short names to waves, and returns `WorktreeDto` with optional `wave_id` (main repo excluded).

**Swift/LoopflowCore:** Added `WorktreeInfo` and `listWorktrees(repo:)` in `WaveServiceProtocol`/`LocalWaveService`.

**Swift/Concerto:** Added `WorktreeStore`, `WorktreeRow`, and an “On Disk” sidebar section for orphan worktrees. RepoState now refreshes worktrees on connect and wave create/delete.

## Key choices

**Full rename, no compatibility shim.** This is an internal convention change; we moved to `wave/` directly instead of supporting both directories.

**Step/flow rename alignment.** Names containing “roadmap” were renamed to “wave” to keep terminology consistent (`ship-wave`, `wave-expand`, etc.).

**Worktree adoption stays convention-based.** Upgrade action calls `createWave(name: shortName)` and relies on existing `{repo}.{waveName}` worktree naming rather than adding a new request field.

**Event-driven refresh, no polling.** Worktrees refresh on connect and wave create/delete events; no periodic polling loop.

**Orphans-only sidebar section.** Worktree-backed waves stay in the normal wave list; “On Disk” only shows untracked worktrees.

**Empty-state behavior updated.** Sidebar empty state now requires both `waves.isEmpty` and `orphans.isEmpty`, so orphaned worktrees are always visible as an upgrade path.

## How it fits together

```
Prompt/docs pipeline: gather_context() → include wave/ docs + renamed builtins

Sidebar → WorktreeStore.orphans → WorktreeRow (per orphan)
                ↑
RepoState.refreshWorktrees() → LocalWaveService.listWorktrees() → GET /worktrees
                ↑
Event triggers: connect, wave.created, wave.deleted
```

The rename is mechanical across engine/prompts/docs/tests. The worktree feature is additive and follows existing `RepoState` + service/store patterns.

## Risks and bottlenecks

**Rename is a hard cut.** Repos still using `roadmap/` won’t be picked up by lfdocs until moved to `wave/`.

**`list_worktrees` is blocking work.** It shells out to git and does merge checks; route is wrapped in `spawn_blocking` but can still be slow in large repos.

**Wave↔worktree linking is name-based.** Renamed waves can drift from their worktree directory naming.

**No dedicated worktree event stream.** External `git worktree add` changes appear on reconnect or after relevant wave events.

## What's not included

- Migration tooling from `roadmap/` to `wave/`
- Backward-compatible aliases for old step/flow names
- Worktree deletion/pruning from the UI
- Worktree websocket events
- Sorting/filtering controls for the “On Disk” list
