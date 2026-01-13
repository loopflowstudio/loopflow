---
include:
  - tests/**
---
Turn the design doc into working code.

The design doc is under `.design/` and auto-included in this prompt. It contains data structures, function signatures, constraints, and a "done when" verification step.

## Workflow

1. Read the design doc in `.design/` to understand what to build
2. Read STYLE.md to understand code conventions
3. Implement data structures first—get the core types right
4. Implement functions one at a time, following the signatures in the design
5. Run `uv run pytest tests/` to verify nothing broke
6. Run the "done when" check from the design doc
7. Do not commit—leave that to the caller or pipeline

## Implementation rules

**Match existing patterns.** Before writing new code, find similar code nearby and match its style. If the codebase uses `@dataclass`, use `@dataclass`. If it uses type hints, use type hints.

**Stay in scope.** Implement exactly what the design doc describes. If something should be added, note it in `.design/questions.md` but don't build it.

**Tests prove it works.** Add tests for user-visible behavior. Don't test implementation details. Don't write tests that just verify mock calls—assert on actual results.

**Skip obvious docstrings.** If the function name and types tell the whole story, don't repeat it in prose. No `Args:`/`Returns:` blocks.

**Leave the design doc.** Don't delete `.design/*.md`. The review step and landing process handle cleanup.

## Loopflow code conventions

These are specific to this codebase:

- **Use `uv run`** for all Python commands, or activate `.venv` first
- **Imports at top of file**, never inline
- **Prefix private functions with `_`**
- **Return `None` for "not found"**, raise exceptions for "shouldn't happen"
- **Prefer functions over classes** when you don't need state
- **No backwards-compatibility shims** unless explicitly required

## If something's wrong

If the design doc is unclear, make the simplest choice and move on. Note your assumption in `.design/questions.md`. The code can be rewritten if needed.

If implementation reveals a design flaw, note it but keep going. The design was scaffolding—reality should diverge when it makes sense.

## Auto mode

In auto/headless runs, do not pause to ask questions. Make the best assumption you can and append any open questions to `.design/questions.md`.

