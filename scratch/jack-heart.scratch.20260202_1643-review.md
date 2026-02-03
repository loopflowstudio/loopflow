# Design Review: Rename clear_design_artifacts to clear_scratch

## What was implemented

Renamed `clear_design_artifacts` to `clear_scratch` across the codebase for consistency with the directory it operates on (`scratch/`). Also added scratch-clearing to `lf ops next` before enabling auto-merge.

## Key changes

1. **design.py**: Renamed `clear_design_artifacts` → `clear_scratch`

2. **land.py**:
   - Updated import and function calls
   - Refactored `_clear_scratch_and_push` to use `clear_scratch()` instead of duplicating file deletion logic

3. **next.py**:
   - Added `_clear_scratch_and_push` helper (reusing `clear_scratch`)
   - Call it before enabling auto-merge to prevent scratch/ contents from reaching main

4. **test_design.py**: Updated test name and function references

## Key choices

**Name change**: `clear_design_artifacts` was indirect—it operated on `scratch/` but the name referenced "design artifacts". `clear_scratch` is direct and matches the directory name.

**Consolidated logic**: The file deletion logic was duplicated between `design.py` and `land.py`. Now both `land.py` and `next.py` call `clear_scratch()` from design.py, eliminating duplication.

**Added to next.py**: Previously `lf ops next` didn't clear scratch/ before enabling auto-merge. This could leave scratch/ contents in PRs that get merged. Now consistent with `land.py`.

## How it fits together

```
design.py::clear_scratch()     <- shared implementation
    ↑                ↑
land.py          next.py       <- both use it for pre-merge cleanup
```

Both `land` and `next` operations now clear scratch/ before their PRs merge, using the same underlying function.

## Risks and bottlenecks

- **Minor**: `_clear_scratch_and_push` is now defined in both `land.py` and `next.py`. They're identical. Could be extracted to a shared module, but the duplication is limited to 10 lines and keeps each module self-contained.

## What's not included

- No changes to the behavior of `clear_scratch` itself—just renamed and consolidated usage
- No changes to the scratch/ directory structure or conventions
