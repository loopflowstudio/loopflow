# refactor: co-design re-architecture prompt

## What to build

A new interactive prompt (`refactor.md`) for human-agent collaborative re-architecture of existing code.

## Data structures

None—this is a prompt file, not code.

## Key functions

N/A—prompt-only change.

## Constraints

- Must be interactive (set in frontmatter)
- Should draw from `reduce` (structural simplification) and `refine` (iterative preference learning)
- Focus on co-design, not autonomous execution
- Incremental commits during refactoring session

## Done when

```bash
cat .claude/commands/refactor.md  # prompt exists
grep "interactive: true" .claude/commands/refactor.md  # is interactive
```

The prompt should guide a collaborative exploration of restructuring options, not just execute changes autonomously.
