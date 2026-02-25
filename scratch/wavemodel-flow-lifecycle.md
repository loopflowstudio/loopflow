# Flow lifecycle: current model (2026-02-25)

This document is the single source of truth for the flow lifecycle changes on this branch.

## Final model

### Naming and flow intent

- `build` is the headless implementation flow:
  - `implement → compress → gate → update-wave`
- `ship` is the interactive flow:
  - `design → build → review`

Dependent flows now route through `build`:

- `pair`: `design → build`
- `grind`: `research → iterate → build → gate`
- `incident`: `debug → 5whys → build`
- `ship-wave`: `start → build`
- `ship-roadmap`: `ingest → kickoff → review-design → build → review`
- `scan`: `scan-report → scan-plan → build`

### Canonical post-work reconciliation

`update-wave` is now the only post-work step.

Removed:
- step: `consolidate`
- step: `add-to-wave`
- flow: `publish`

`update-wave` owns all reconciliation work:
1. Update roadmap/status in `wave/<wave>/`
2. Promote unfinished/actionable `scratch/` items into `wave/<wave>/`
3. Merge/dedupe collisions in `wave/<wave>/` (no silent overwrite)
4. Remove promoted scratch artifacts

Plan flows now end in `update-wave` directly:
- `wave-reduce`
- `wave-polish`
- `wave-expand`

### Loop behavior and wave presence

Loop ticker checks for the presence of `wave/<name>/` in the canonical worktree before creating a run. If the directory exists, the wave has work. If it's been removed, the wave is done.

Remove `wave/<name>/` to signal a wave is complete.

## Operational semantics

- Wave dir present = wave has work. Wave dir removed = wave is done.
- No content parsing or backlog heuristics. Presence is the only signal.
- This PR assumes one canonical worktree per wave (no sidecar/per-run worktrees).

## Migration impact

### Breaking name changes

Any external scripts using old names must migrate:
- headless `ship` flow name → `build`
- `design-ship-review` flow → `ship`
- `publish`, `consolidate`, `add-to-wave` removed

### Known risk areas

- Ticker only checks directory presence, not contents.
- Environments without Docker socket may fail Docker-dependent local tests.

## Scope boundary

Out of scope for this change:
- per-run/sidecar worktree architecture
- wave config schema changes
- DB-backed wave item tracking
- auto-pause behavior on empty backlog
- changes to `advance_branch`
- renaming `design-and-ship`
