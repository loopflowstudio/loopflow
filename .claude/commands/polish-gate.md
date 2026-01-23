---
requires: diff vs main
produces: polished code, passing tests
---
Fix issues and run tests before landing.

## Goal

Get this branch ready to merge. Fix obvious problems, make sure tests pass, clean up rough edges. Not a refactor—just the minimum to ship with confidence.

## Workflow

1. `git diff main...HEAD` to see what changed
2. Run `uv run pytest tests/` — fix any failures
3. Run `uv run ruff check --fix` — fix any lint errors
4. Read through the diff for obvious issues
5. Fix what you find, run tests again
6. When tests pass and code is clean, done

## What to fix

- Failing tests
- Lint errors
- Typos in user-facing strings
- Missing imports
- Obvious bugs (off-by-one, wrong variable, etc.)
- Dead code introduced by this branch

## What not to fix

- Pre-existing issues in files you didn't change
- Style preferences (if it passes lint, it's fine)
- "While I'm here" improvements
- Test coverage gaps (unless tests are actually failing)

## Guardrails

**Stay in scope.** Only touch files this branch modified.

**Don't refactor.** If something works and passes lint, leave it.

**Tests are the gate.** If tests pass, stop looking for problems.

## Output

Clean code, passing tests. No design doc—this is a mechanical pass. If you found and fixed issues, commit them with a message like "polish: fix lint errors and typo in help text".
