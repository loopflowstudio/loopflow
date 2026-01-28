# Design Review: Unified Wave Context Determination

## What was implemented

This branch adds unified wave context determination for prompts, enabling waves to be identified from multiple sources:

1. **lfd database lookup** - Query the daemon database to find wave by worktree path
2. **Worktree naming pattern** - Recognize `<repo>.<wave>.main` naming convention
3. **CLI flag reordering** - Support `lf -m codex ship` syntax (flags before step/flow name)
4. **Engine display header** - Show the coding agent and model in flow output
5. **Documentation update** - Improved `ingest.md` step with clear wave discovery priority

## Key choices

**Priority order for wave determination:**
1. Explicit `--wave` flag (highest)
2. lfd database by worktree path
3. Worktree naming pattern (`<repo>.<wave>.main`)
4. Roadmap folder inference (lowest)

This order ensures explicit configuration wins, daemon-managed waves are recognized, and naming conventions work as fallback. The database lookup fails silently if lfd isn't running.

**CLI flag reordering approach:**
Rather than forcing users to put flags after the step name, the CLI now scans for a valid step/flow name among the arguments and reorders internally. This makes `lf -m codex ship` work the same as `lf ship -m codex`.

**Engine header display:**
Added a simple dim gray line showing `engine: claude` or `engine: codex:o3` to give visibility into which coding agent is running without being intrusive.

## How it fits together

```
User invokes lf/lfd
       │
       ▼
determine_wave() in lf/wave.py
       │
       ├─► explicit_wave parameter? → use it
       │
       ├─► lfd database lookup via get_wave_by_worktree() → use wave.name
       │
       ├─► worktree name matches <repo>.<wave>.main? → extract wave name
       │
       └─► roadmap/<candidate>/ exists? → use candidate
```

The `get_wave_by_worktree()` function in `lfd/wave.py` mirrors the existing `get_wave_by_name()` pattern, querying SQLite with optional repo filtering.

## Risks and bottlenecks

**Database import overhead:** `_lookup_wave_from_lfd()` imports from `lfd.wave` which pulls in the database module. This is wrapped in try/except to fail silently, but adds import time when lfd is available. Mitigated by lazy import inside the function.

**Worktree pattern matching:** The `<repo>.<wave>.main` pattern could false-positive on worktrees that happen to have 3+ dot-separated parts ending in "main". The check excludes digits and "master" but could still match unintended names.

## What's not included

- No changes to how waves are created or configured
- No new CLI commands for wave management
- No tests for the new wave determination logic (existing tests pass, but the new code paths aren't directly tested)
