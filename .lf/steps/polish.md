---
include:
  - tests/**
requires: diff vs main
produces: passing tests, clean code
---
Fix issues and run tests before landing.

## Goal

Get this branch ready to merge. Fix obvious problems, make sure tests pass. Not a refactor—just the minimum to ship with confidence.

## Workflow

1. `git diff main...HEAD` to see what changed
2. Run `uv run ruff check --fix` to fix lint errors
3. Run ALL test suites (see TESTING.md)—CI runs three, all must pass:
   - `uv run pytest tests/` — Python
   - `swift test --package-path swift` — Swift
   - `cd swift && xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS'` — Concerto UI
4. Read through the diff for obvious issues
5. Fix what you find, run tests again
6. When ALL tests pass and code is clean, done

## What to fix

- Failing tests (all three suites)
- Lint errors
- Typos in user-facing strings
- Missing imports
- Obvious bugs (off-by-one, wrong variable, etc.)
- Dead code introduced by this branch
- Style violations: inline imports, `_v2` suffixes, tests that assert on mock calls

## What not to fix

- Pre-existing issues in files you didn't change
- Style preferences (if it passes lint, it's fine)
- "While I'm here" improvements
- Test coverage gaps (unless tests are actually failing)

## Test standards

- **Test behavior, not implementation.** Assert on what the function returns.
- **Mocks prevent side effects.** Don't assert on mock calls—assert on results.
- **Delete flaky tests.** Don't add retries or sleeps.
- **One behavior per test.** Short, focused.

## Guardrails

**Stay in scope.** Only touch files this branch modified.

**Don't refactor.** If something works and passes lint, leave it.

**All tests are the gate.** If ALL three test suites pass, stop looking for problems.

## Output

Clean code, passing tests. If you fixed issues, commit with a message like "polish: fix lint errors".
