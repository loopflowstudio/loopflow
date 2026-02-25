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

### Loop behavior and backlog signal

Loop ticker now checks backlog from the canonical wave worktree before creating a run.

Backlog is considered empty when `wave/<name>/` has no actionable top-level markdown items:
- include: `*.md`
- exclude: `README.md`
- ignore: `*.yaml`

If backlog is empty, ticker skips creating a new loop run.

## Operational semantics

- Empty backlog means “no queued wave items,” not “last run succeeded.”
- No auto-pause on empty backlog. Wave remains idle with loop stimulus enabled.
- This PR assumes one canonical worktree per wave (no sidecar/per-run worktrees).

## Migration impact

### Breaking name changes

Any external scripts using old names must migrate:
- headless `ship` flow name → `build`
- `design-ship-review` flow → `ship`
- `publish`, `consolidate`, `add-to-wave` removed

### Known risk areas

- Backlog detection only sees top-level actionable markdown files in `wave/<name>/`.
- Environments without Docker socket may fail Docker-dependent local tests.

## Scope boundary

Out of scope for this change:
- per-run/sidecar worktree architecture
- wave config schema changes
- DB-backed wave item tracking
- auto-pause behavior on empty backlog
- changes to `advance_branch`
- renaming `design-and-ship`
