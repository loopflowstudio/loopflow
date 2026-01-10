Grease and polish: fix issues and run tests before landing.

The deliverable is working, clean code that passes tests.

## Process

1. **Review and fix**
   - Run `git diff main...HEAD` to see what changed
   - Review against STYLE.md and general code quality
   - Fix bugs, style violations, and unnecessary complexity directly
   - Don't just note issues—fix them
   - Rewrite the primary design doc in `.design/` to match the implementation (keep any decisions log)

2. **Test**
   - Run the full test suite
   - If tests fail, determine: broken test or broken code?
   - Fix failures one by one, running single tests while debugging
   - Add missing tests for key behavior changes

## What to fix

**Style guide violations.** Read STYLE.md. Fix naming, error handling, documentation patterns.

**Bugs.** Logic errors, edge cases, off-by-ones, unhandled errors.

**Unnecessary complexity.** Simplify code that's more elaborate than needed.

**Test failures.** Get the suite green.

**Missing tests.** Add tests for user-visible behaviors that aren't covered.

## What to ignore

Don't expand scope. If something unrelated to this branch could be better, leave it.

Don't refactor working code that isn't broken. Only fix actual problems.

**Design doc deviations.** If any `.design/*.md` docs exist, treat the implementation as the source of truth. Deviations are likely intentional. Evaluate code at face value, not for fidelity to the original plan.

## Test standards

From STYLE.md:
- Test user behavior, not implementation details
- Keep tests short and focused on one behavior
- Delete flaky tests rather than adding retries

**Mocking**: Use mocks to prevent side effects (network, subprocess, file I/O). But don't write tests that just verify mock calls—assert on the *result* of the function under test, not that a mock was called.

## Output

Make the fixes. Run tests until they pass. If there's nothing to fix and tests pass, say so.
