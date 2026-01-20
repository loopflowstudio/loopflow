# Branch designer/001 Summary

## What this branch implements

This branch contains several related changes to loopflow:

### 1. Configurable branch naming schema
- New `branch_names.schema` config option in `.lf/config.yaml`
- Placeholder substitution: `{name}`, `{user}`, `{ts}`, `{date}`
- `lfops wt create` command applies the schema
- Maestro sidebar shows short name with full branch in tooltip
- Bug fix: new branches with no commits are no longer marked as prunable

### 2. Documentation restructuring
- Docs reorganized around the three-tier workflow (interactive → background → overnight)
- Updated terminology and examples

### 3. Maestro: agents → loops refactoring
- Replaced `Agent` model with `Loop` model
- Replaced `AgentService` with `LoopService`
- Updated UI to show loops from `lfd.db` instead of static agent list
- Loops are now loaded from the daemon database

### 4. Loop runner improvements
- `loop_runner` derives iteration branch prefix from loop-main
- Support for multiple loops per goal with distinct areas

### 5. Test infrastructure
- Mocked `total_outstanding` in scheduler tests for reliability

## Verification (2026-01-20)

- All 469 Python tests pass
- All 22 Swift tests pass
- Maestro builds with no warnings
- Working tree is clean after polish fixes
