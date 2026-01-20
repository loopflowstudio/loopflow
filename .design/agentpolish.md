# Unique Branch Names for Loops

## Summary

Loop branch names now include random words for uniqueness, along with the area of responsibility.

## Changes

**Format:**
- Without area: `{goal}-{adjective}-{noun}-main`
- With area: `{goal}-{area_slug}-{adjective}-{noun}-main`

**Examples:**
- `product-engineer-swift-river-main`
- `test-coverage-api-calm-brook-main`

**Implementation:**
- Added 50 adjectives and 50 nouns to `loops.py` (2500 possible combinations)
- Updated `_allocate_loop_main()` to always include random words
- Iteration branches derive from loop-main: `product-engineer-swift-river/001`

## Files Changed

- `src/loopflow/lfd/loops.py` - Word lists and `_allocate_loop_main()` function
- `tests/test_lfd.py` - Tests for random word generation and branch prefix handling
