# Wave Directory Rename + Worktree Browser

## Current state

This branch shipped two related changes:

1. **Execution specs now live in `wave/`** (not `roadmap/`) across Rust engine logic, built-in step/flow names, docs, fixtures, and tests.
2. **Concerto now surfaces orphaned git worktrees** in an **On Disk** section and supports one-click adoption into a wave.

## Why this direction

`wave/` matches how loopflow actually works: these files are executable wave specs, not passive roadmap notes. The rename removes terminology drift and aligns naming across prompts, flows, and docs (`wave-plan`, `add-to-wave`, `ship-wave`, `wave-reduce`, etc.).

Worktree browsing closes the loop for local repos with existing worktrees: users can see orphaned worktrees and adopt them without shell work.

## Implemented architecture

### Rename path

- `gather_docs()` reads `wave/<wave>/` docs.
- Builtins registry keys and file paths were renamed to the wave terms.
- CLI/help/docs/test goldens now reference `wave/`.
- No compatibility shim for `roadmap/` was added.

### Worktree path

- `lfd` exposes `GET /worktrees?repo=<path>`.
- Route returns `WorktreeDto` with optional `wave_id` when a worktree maps to an existing wave.
- Swift core adds `WorktreeInfo` + `listWorktrees(repo:)`.
- Concerto adds `WorktreeStore`, `WorktreeRow`, and an **On Disk** orphan list.
- Repo state refreshes worktrees on connect and on wave create/delete events.

## Product behavior

- Existing wave-backed worktrees stay in the normal wave list.
- Only orphaned worktrees appear under **On Disk**.
- Empty state only shows when both waves and orphans are empty.
- Adopting an orphan uses existing naming conventions (`{repo}.{waveName}`).

## Constraints and follow-ups

- **Hard cut rename:** repos still using `roadmap/` must migrate to `wave/`.
- **Name-based linking:** wave/worktree drift is possible after manual renames.
- **Refresh model:** external `git worktree` changes appear on reconnect or related events.
- Not included: migration tooling, legacy aliases, worktree prune/delete UI, worktree event stream, sidebar sorting/filtering.
