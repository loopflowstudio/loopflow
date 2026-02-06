---
requires: scratch/<branch>.md
produces: code, tests
---
Turn the design doc into working code.

## Goal

Working code with rough edges beats perfect code that took too long.

Produce a first draft quickly. Polish cleans it up. You can be re-invoked if needed. Don't block on ambiguity—make the simplest choice and keep moving.

## Workflow

The design doc and style guides are in your context.

1. **Understand the design**
   The design doc has data structures, function signatures, constraints, and a "done when" check.

2. **Implement**
   - Data structures first—get the core types right
   - Functions one at a time, following the signatures
   - Match existing patterns in the codebase

3. **Verify**
   - Run tests to confirm nothing broke
   - Run the "done when" check from the design doc

## Rules

**Match existing patterns.** Find similar code nearby and match its style. If the codebase uses `@dataclass`, use `@dataclass`. If it uses type hints, use type hints.

**Stay in scope.** Implement exactly what the design describes. Scope creep goes in `scratch/questions.md`, not the code.

**Tests prove it works.** Add tests for user-visible behavior. Don't test implementation details. Assert on results, not mock calls.

## Wave context

If `<lf:wave>` is present, check `roadmap/<wave>/README.md` in docs:

- Follow the wave's principles during implementation
- Check against compatibility matrix if mentioned
- Note drift from wave constraints in `scratch/questions.md`

## When the design is wrong

If the design doc is unclear, make the simplest choice and move on. Note your assumption in `scratch/questions.md`.

If implementation reveals a design flaw, note it but keep going. The design was scaffolding—diverge when reality demands it.
