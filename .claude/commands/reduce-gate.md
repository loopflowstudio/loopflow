---
requires: diff vs main
produces: simpler code, same diff scope
---
Eliminate what you can without majorly increasing the scope of the diff.

## Goal

Make this change smaller. Not better architecture, not cleaner patterns—just less code doing the same thing. If something can be deleted or inlined, do it. If it can't, leave it alone.

## Workflow

1. `git diff main...HEAD --stat` to see the diff scope
2. Read the changed files
3. Find one thing to eliminate: dead code, unnecessary abstraction, redundant logic
4. Delete it. Run `uv run pytest tests/`
5. If tests pass, done. If not, revert and try something else.

## What counts as reduction

- Delete unused code the diff introduced
- Inline a helper that's only called once
- Remove a conditional that can't happen
- Collapse two similar branches into one
- Delete comments that restate the code

## What doesn't count

- Refactoring code outside the diff
- Adding abstractions to "simplify" (that's adding, not reducing)
- Renaming for clarity (that's polish, not reduce)
- Fixing unrelated issues you noticed

## Guardrails

**Stay in scope.** If a file wasn't in the diff, don't touch it. The goal is a smaller diff, not a better codebase.

**Preserve behavior.** Tests should pass. If they don't, the reduction was wrong.

**One thing at a time.** Make one reduction, verify tests, then look for another. Don't batch.

## Output

Modified code with a smaller footprint. No design doc—this is a mechanical pass.
