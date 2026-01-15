---
requires: diff vs main
produces: simplified code
---
Simplify code touched by this branch while preserving user behavior.

## Goal

Find places where things don't fit together cleanly. Special cases, conditionals, and adapters are signs the underlying data structures or APIs could be reshaped so the pieces just slot in.

The best reduction isn't deleting a function—it's reshaping a structure so three special cases become one, or rearranging an API so callers don't need conditionals.

One focused change, then stop. Verify tests pass. Focus on files this branch has already modified.

## Workflow

1. Run `git diff main...HEAD --stat` to see which files this branch modified
2. Read those files and identify what can be deleted or simplified
3. Make one focused simplification
4. Run `uv run pytest tests/` to verify behavior is preserved
5. If tests pass, you're done

## Priority order

1. **Reshape data structures.** Can a different representation eliminate special cases? Can fields be combined, split, or retyped so code that consumes them gets simpler?
2. **Rearrange APIs.** Can the interface change so callers don't need conditionals? Can two similar functions become one with a better signature?
3. **Delete dead code.** Unused functions, unreachable branches, obsolete options.
4. **Collapse duplication.** If the same pattern appears twice, inline it or pick one location.

## Guardrails

**Preserve user behavior.** CLI flags, outputs, and workflows must stay the same. If behavior must change to simplify, document the tradeoff in `.design/questions.md`.

**Reshape, don't layer.** Restructuring data or APIs is good. Adding adapters, wrappers, or compatibility shims is not. The goal is fewer moving parts, not different ones.

**Stay in scope.** Only simplify code this branch touched. "While I'm here" refactoring belongs in a separate branch.

**No new abstractions.** Don't extract helpers, base classes, or utilities. Reshaping a data structure to eliminate special cases is different from adding a layer to hide them.

## Loopflow's simplicity principles

- **One implementation.** No `_v2`, `_old`, `_new`. Delete the old version.
- **No backwards compatibility.** If something's unused, delete it completely. No `_deprecated` stubs.
- **Functions over classes.** If you can delete a class and use a function, do it.
- **Inline over abstract.** Three similar lines of code is better than a premature abstraction.

## Output

Make the simplification directly. Run tests to verify. Note any assumptions in `.design/questions.md`.
