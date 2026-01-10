# Implementation Complete: Design Artifact Migration

This branch successfully migrated loopflow's design artifact handling from `<branch>.md` files at repo root to the `.design/` directory.

## What was built

1. **Directory migration**: Changed from `<branch>.md` at repo root to `.design/*.md`
2. **Context loading**: `gather_design_docs()` in `src/loopflow/design.py` loads all `.design/*.md` files into prompt context
3. **Review workflow**: Review task now writes to `.design/review.md` without deleting existing design docs
4. **Polish workflow**: Polish task rewrites primary design doc to match implementation
5. **Landing cleanup**: Both `lf pr land` and `lf land` clear `.design/*` contents (keep folder) before landing
6. **Updated prompts**: All task prompts (design, implement, review, polish) reference `.design/` consistently

## Key files changed

- `src/loopflow/design.py` - New module with `gather_design_docs()`, `clear_design_artifacts()`, `has_design_artifacts()`
- `src/loopflow/context.py` - Loads `.design/*.md` files into docs block
- `src/loopflow/cli/pr.py` - Clears `.design/*` before landing PR
- `src/loopflow/cli/land.py` - New command for local landing, clears `.design/*`
- `.claude/commands/*.md` - Updated all task prompts
- `src/loopflow/prompts/*.md` - Updated bundled prompts
- `README.md`, `STYLE.md` - Updated documentation

## Verification

All "done when" criteria met:
- ✓ .design/ is the only design-doc location; auto-included in context
- ✓ Review writes .design/review.md and leaves design docs intact
- ✓ Polish rewrites primary design doc to match implementation
- ✓ Landing removes .design/* contents without removing folder
- ✓ README/STYLE and prompts reflect the new workflow

Tests pass: 126 passed, 0 failed
