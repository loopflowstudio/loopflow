---
requires: scratch/<branch>.md
produces: code, tests
---
Turn the design doc into working code.

## Goal

Produce a working first draft quickly. The human will review it, polish will clean it up, and you can be re-invoked if needed. Don't block on ambiguity—make the simplest choice and keep moving. Working code with rough edges beats perfect code that took too long.

The design doc is under `scratch/` and auto-included. It contains data structures, function signatures, constraints, and a "done when" verification step.

## Workflow

1. Read the design doc in `scratch/` to understand what to build
2. Check `roadmap/` (root and any area-specific) for architecture guidance and prior decisions
3. Read any style guides or conventions docs in the repo
4. Implement data structures first—get the core types right
5. Implement functions one at a time, following the signatures in the design
6. Run tests to verify nothing broke
7. Run the "done when" check from the design doc
8. Do not commit—leave that to the caller or pipeline

## Implementation rules

**Match existing patterns.** Before writing new code, find similar code nearby and match its style. If the codebase uses `@dataclass`, use `@dataclass`. If it uses type hints, use type hints.

**Stay in scope.** Implement exactly what the design doc describes. If something should be added, note it in `scratch/questions.md` but don't build it.

**Tests prove it works.** Add tests for user-visible behavior. Don't test implementation details. Don't write tests that just verify mock calls—assert on actual results.

**Leave the design doc.** Don't delete `scratch/*.md`. The review step and landing process handle cleanup.

## If something's wrong

If the design doc is unclear, make the simplest choice and move on. Note your assumption in `scratch/questions.md`. The code can be rewritten if needed.

If implementation reveals a design flaw, note it but keep going. The design was scaffolding—reality should diverge when it makes sense.
