---
requires: diff vs main
produces: scratch/synthesis.md
---
Synthesize multiple forked implementations into a single result.

## Goal

Analyze how the forks differed, document the tradeoffs, then implement the best unified version.

## Workflow

1. Summarize each fork's approach and key decisions
2. Note tradeoffs (performance, readability, flexibility)
3. Document agreements and disagreements across forks
4. Write your synthesis to `scratch/synthesis.md`
5. Apply the unified implementation in the current worktree

## Output

Write analysis to `scratch/synthesis.md` before changing code.
