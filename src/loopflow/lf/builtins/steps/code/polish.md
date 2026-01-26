---
requires: code on branch
produces: passing tests, scratch/ updated
---
Fix issues and run tests before landing.

## Goal

Get to green quickly. Fix real problems, not hypothetical ones. The bar is "ready to land," not "perfect." The human can do another polish pass or land directly—don't gold-plate.

The deliverable is working, clean code that passes tests.

## Workflow

1. **Review and fix**
   - Run `git diff main...HEAD` to see what changed
   - Review against any style guides in the repo
   - Fix bugs, style violations, and unnecessary complexity directly
   - Don't just note issues—fix them
   - Rewrite the primary design doc in `scratch/` to match the implementation
   - Update README and docs if the branch changes user-facing behavior or APIs

2. **Test**
   - Run the project's test suite
   - If tests fail, determine: broken test or broken code?
   - Fix failures one by one
   - Add missing tests for key behavior changes

## What to fix

Focus only on code changed by this branch.

**Test failures.** Get the suite green first.

**Bugs.** Logic errors, edge cases, off-by-ones in the new code.

**Missing tests.** Add tests for user-visible behavior that isn't covered. Keep them short and focused.

## What to ignore

**Unrelated code.** Don't fix things outside this branch's scope. "While I'm here" improvements belong in a separate branch.

**Working code you'd write differently.** Only fix actual problems, not style preferences.

**Design doc deviations.** The implementation is the source of truth. Deviations are intentional.

## Output

Fix issues directly. Run tests until they pass. If nothing needs fixing and tests pass, say so.
