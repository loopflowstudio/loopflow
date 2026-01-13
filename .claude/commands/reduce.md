Simplify code touched by this branch while preserving user behavior.

Focus on trimming complexity within files this branch has already modified. Don't refactor unrelated code.

## Workflow

1. Run `git diff main...HEAD --stat` to see which files this branch modified
2. Read those files and identify what can be deleted or simplified
3. Make one focused simplification
4. Run `uv run pytest tests/` to verify behavior is preserved
5. If tests pass, you're done

## Priority order

1. **Delete dead code.** Unused functions, unreachable branches, obsolete options. If it's not called, delete it.
2. **Collapse duplication.** Repeated patterns within the changed files. Don't create new abstractions—inline the common code or pick one location.
3. **Simplify logic.** Replace nested conditionals with early returns. Replace clever code with obvious code.
4. **Tighten APIs.** Remove optional parameters that are never used. Remove public functions that should be private.

## Guardrails

**Preserve user behavior.** CLI flags, outputs, and workflows must stay the same. If behavior must change to simplify, document the tradeoff in `.design/questions.md`.

**Prefer deletion over rewriting.** The best refactor is code that doesn't exist anymore.

**Stay in scope.** Only simplify code this branch touched. "While I'm here" refactoring belongs in a separate branch.

**No new abstractions.** If you're tempted to extract a helper, base class, or utility—don't. Reduce means less code, not different code.

## Loopflow's simplicity principles

- **One implementation.** No `_v2`, `_old`, `_new`. Delete the old version.
- **No backwards compatibility.** If something's unused, delete it completely. No `_deprecated` stubs.
- **Functions over classes.** If you can delete a class and use a function, do it.
- **Inline over abstract.** Three similar lines of code is better than a premature abstraction.

## Output

Make the simplification directly. Run tests to verify. Note any assumptions in `.design/questions.md`.

## Auto mode

In auto/headless runs, do not pause to ask questions. Make the best assumption you can and append any open questions to `.design/questions.md`.
