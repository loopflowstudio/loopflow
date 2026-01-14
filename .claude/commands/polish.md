---
include:
  - tests/**
requires: code on branch
produces: passing tests, .design/ updated
---
Fix issues and run tests before landing.

## Goal

Get to green quickly. Fix real problems, not hypothetical ones. The bar is "ready to land," not "perfect." The human can do another polish pass or land directly—don't gold-plate.

The deliverable is working, clean code that passes tests.

## Workflow

1. **Review and fix**
   - Run `git diff main...HEAD` to see what changed
   - Review against STYLE.md and general code quality
   - Fix bugs, style violations, and unnecessary complexity directly
   - Don't just note issues—fix them
   - Rewrite the primary design doc in `.design/` to match the implementation (keep any decisions log)

2. **Test**
   - Run the full test suite: `./dev test` (runs both Python and Swift tests)
   - If tests fail, determine: broken test or broken code?
   - Fix failures one by one, running single tests while debugging

   Before adding or fixing tests, ask: **What behaviors matter?**
   - What are the key user-visible behaviors this branch enables?
   - Do existing tests actually verify those behaviors work?
   - A test that passes but doesn't prove the feature works is useless
   - A test that fails because the mock wiring changed is testing the wrong thing

   Then:
   - Add missing tests for behaviors that matter but aren't covered
   - Delete or rewrite tests that verify mock calls instead of results
   - Simplify tests that are complex but don't prove user value

   For component-specific testing:
   - Python only: `./dev py`
   - Swift only: `./dev swift`

## What to fix

Focus only on code changed by this branch.

**Test failures.** Get the suite green first. Run single tests while debugging:
```bash
uv run pytest tests/test_specific.py::test_name -v
```

**Style violations specific to this codebase:**
- Remove `Args:`/`Returns:` docstrings when types are clear
- Move inline imports to top of file
- Add `_` prefix to private functions
- Replace tests that assert on mock calls with tests that assert on results

**Bugs.** Logic errors, edge cases, off-by-ones in the new code.

**Missing tests.** Add tests for user-visible behavior that isn't covered. Keep them short and focused.

## What to ignore

**Unrelated code.** Don't fix things outside this branch's scope. "While I'm here" improvements belong in a separate branch.

**Working code you'd write differently.** Only fix actual problems, not style preferences.

**Design doc deviations.** The implementation is the source of truth. Deviations are intentional.

## Test standards

These are specific to this codebase:

- **Test behavior, not implementation.** Assert on what the function returns, not how it works internally.
- **Mocks prevent side effects.** Use them for network, subprocess, file I/O. But don't assert on mock calls—assert on the result.
- **Delete flaky tests.** Don't add retries or sleeps. If a test is flaky, it's testing the wrong thing.
- **One behavior per test.** Short, focused tests that prove one thing works.

## Output

Fix issues directly. Run tests until they pass. If nothing needs fixing and tests pass, say so.

