---
requires: diff vs main
produces: simpler code
---
Eliminate what you can without expanding the diff scope.

## Goal

Make this change smaller. The best reduction isn't deleting a function—it's reshaping a structure so three special cases become one, or rearranging an API so callers don't need conditionals.

One focused change, then stop. Verify tests pass.

## Workflow

1. `git diff main...HEAD --stat` to see the diff scope
2. Read the changed files
3. Find one thing to eliminate
4. Delete it. Run tests (see TESTING.md)
5. If tests pass, done. If not, revert and try something else.

## Priority order

1. **Reshape data structures.** Can a different representation eliminate special cases?
2. **Rearrange APIs.** Can the interface change so callers don't need conditionals?
3. **Delete dead code.** Unused functions, unreachable branches, obsolete options.
4. **Collapse duplication.** Same pattern twice? Inline it or pick one location.

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

**Stay in scope.** If a file wasn't in the diff, don't touch it.

**Preserve behavior.** Tests should pass. If they don't, the reduction was wrong.

**One thing at a time.** Make one reduction, verify tests, then look for another. Don't batch.

**Reshape, don't layer.** Restructuring data or APIs is good. Adding adapters or compatibility shims is not.

## Output

Modified code with a smaller footprint. Note any tradeoffs in `scratch/questions.md`.
