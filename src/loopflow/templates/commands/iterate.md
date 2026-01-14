---
requires: diff vs main
produces: improved code
---
Make meaningful improvements to code touched by this branch.

## Goal

One focused improvement, then stop. The human can run you again (`iterate → iterate → iterate`) or move on. Pick the highest-impact fix within scope, make it, verify tests pass. Don't try to fix everything in one pass.

Focus on the areas this branch has already modified. Incremental quality improvements that compound—not generic refactoring across the whole codebase.

## Workflow

1. Run `git diff main...HEAD --stat` to see which files this branch modified
2. Read those files and identify the highest-impact improvement
3. Make one focused change—don't scatter effort across multiple areas
4. Run tests to verify nothing broke
5. If tests pass, you're done. If not, fix what you broke.

## Priority order (within the branch's scope)

**1. User experience problems.** Error messages that don't help. Workflows that require too many steps. Missing feedback. Fix the worst friction first.

**2. Bugs and edge cases.** Logic errors, off-by-ones, unhandled errors in the modified code.

**3. Simplification.** Code that could be deleted. Abstractions that don't earn their keep. Duplication within the changed files.

**4. Test coverage.** Missing tests for the new behavior. Tests that verify mock calls instead of results.

## What to avoid

**Scope creep.** Only improve code this branch touched. "While I'm here" improvements belong in a separate branch.

**Refactoring for style.** Don't rewrite working code because you'd write it differently. Only fix actual problems.

**Adding features.** This is about quality, not scope. New capabilities need their own design.
