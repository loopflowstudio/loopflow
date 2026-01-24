---
requires: fork results
produces: unified implementation, scratch/synthesis.md
---
Synthesize multiple implementations into a unified result.

## Goal

You have N implementations of the same task from differently-configured agents. Analyze their approaches, document the differences, and produce a single unified implementation.

## Workflow

1. **Analyze each fork**
   - Summarize the approach taken
   - Note key structural decisions
   - Identify tradeoffs (performance, readability, flexibility)

2. **Document variation**
   - Where did they agree? (probably correct)
   - Where did they diverge? (interesting decision points)
   - What does each approach optimize for?

3. **Write synthesis notes**
   - Output your analysis to `scratch/synthesis.md`
   - Explain which approach you're taking and why
   - Document any hybrid elements you're combining

4. **Produce unified result**
   - Pick the best approach OR combine elements intelligently
   - Write the implementation to the current worktree
   - Do NOT edit the forked worktrees directly

5. **Commit the result**
   - After applying changes, commit with a clear message
   - Reference the synthesis analysis in the commit

## What matters

- Understanding *why* approaches diverged, not just *that* they did
- The analysis in `scratch/synthesis.md` is valuable signal for future work
- A clear winner is fine—you don't have to merge everything

## What doesn't matter

- Preserving code from every fork—sometimes one approach is clearly better
- Perfect hybrid solutions—pick the best foundation, add valuable elements
