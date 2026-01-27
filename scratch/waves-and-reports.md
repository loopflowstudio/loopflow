# Waves and Reports: Work Item Management

Restructure how loopflow manages persistent work items and reference material.

## Problem

`roadmap/` conflates two different things:
- **Actionable work items** — things to build next
- **Reference material** — context for understanding

`add-to-roadmap` doesn't distinguish between them. `ingest` doesn't know which wave to pull from. There's no way to clear a backlog systematically.

## Solution

Split into two directories with clear purposes:

| Directory | Purpose | Keyed by | Example |
|-----------|---------|----------|---------|
| `waves/<wave>/` | Actionable work items | wave/worktree name | `waves/lfflow/dynamic-budgets.md` |
| `reports/` | Reference material | topic | `reports/landscape.md` |

### waves/

Each wave has a backlog directory. Items are individual files:

```
waves/
  lfflow/
    dynamic-budgets.md
    summary-fallback.md
  concerto/
    keyboard-nav.md
```

`ingest` pulls from `waves/<wave>/` and moves one item to `scratch/`. The wave name comes from the current worktree or wave configuration.

### reports/

Reference material that might be useful later but isn't immediately actionable:

```
reports/
  landscape.md
  target-customer.md
  terminal-reference.md
```

Keyed by topic, not wave. This is context, not work.

## Updated steps

### add-to-roadmap

Routes content based on type:

- **Actionable follow-up** → `waves/<wave>/<item>.md`
- **Reference/context** → `reports/<topic>.md`

The agent decides based on content. Work items go to the current wave's backlog. Research and analysis go to reports.

**Destination logic:**
```
Is this something to build/do next?
  → waves/<current-wave>/<slug>.md

Is this context/research for later reference?
  → reports/<topic>.md
```

### ingest

Pulls from `waves/<wave>/`:

1. Read items in `waves/<wave>/`
2. Evaluate: urgency, importance, readiness
3. Pick highest priority
4. Move to `scratch/<slug>.md`

If the wave backlog is empty, signal completion (no error, just done).

## New flow primitive: loop_until_empty

Repeat steps until a wave's backlog is empty:

```yaml
- review
- add-to-roadmap
- loop_until_empty:
    wave: lfflow
    steps:
      - ingest
      - ship
```

Flow:
1. `review` analyzes the area
2. `add-to-roadmap` populates `waves/lfflow/` with work items
3. Loop starts:
   - `ingest` pulls one item from `waves/lfflow/`
   - `ship` builds it
   - Repeat until `waves/lfflow/` is empty

Termination is implicit — when `ingest` finds no items, the loop completes.

### Wave inheritance

If `wave:` is omitted, inherit from:
1. Current wave configuration (if running as a wave)
2. Current worktree name
3. Explicit `--wave` flag

## Migration

1. Rename `roadmap/` → `reports/`
2. Move actionable items from `reports/` to `waves/<wave>/`
3. Update `add-to-roadmap` to route based on content type
4. Update `ingest` to read from `waves/<wave>/`
5. Add `loop_until_empty` primitive to flow execution

## What stays the same

- `scratch/` is still the working area for current branch
- `add-to-roadmap` step name stays (conceptually "roadmap" = waves + reports)
- `publish` flow still uses `consolidate → add-to-roadmap`
- Steps produce to `scratch/`, persist via `add-to-roadmap`

## Open questions

- Should `loop_until_empty` have a max iterations safety limit?
- How does `add-to-roadmap` know the current wave name when not running as a wave?
- Should there be `add-to-waves` and `add-to-reports` as explicit alternatives?
