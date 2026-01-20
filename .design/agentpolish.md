# Unique Branch Names for Loops

## Summary

Loop branch names now include random words for uniqueness, along with the area of responsibility. Uses the same magical/musical word lists as Maestro's `NameGenerator.swift`.

## Changes

**Format:**
- Without area: `{goal}-{magical}-{musical}-main`
- With area: `{goal}-{area_slug}-{magical}-{musical}-main`

**Examples:**
- `product-engineer-aurora-melody-main` (no area)
- `product-engineer-maestro-aurora-melody-main` (area="Maestro/")

**Implementation:**
- Added magical (34 words) and musical (26 words) lists to `loops.py` (884 combinations)
- Word lists match `Maestro/Maestro/Services/NameGenerator.swift`
- Updated `_allocate_loop_main()` to always include random words
- Iteration branches derive from loop-main: `product-engineer-aurora-melody/001`

## Files Changed

- `src/loopflow/lfd/loops.py` - Word lists and `_allocate_loop_main()` function
- `tests/test_lfd.py` - Tests for random word generation and branch prefix handling
