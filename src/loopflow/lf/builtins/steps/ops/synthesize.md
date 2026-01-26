---
requires: fork outputs (multiple diffs from parallel agents)
produces: unified code, scratch/synthesis.md
---
Combine multiple forked implementations into a single coherent result.

## Context

This step runs after a fork completes. Each forked agent worked on the same step with a different direction (e.g., infra-engineer, designer, product-engineer). Their outputs are available as diffs.

## Workflow

1. Read each fork's diff and understand its approach
2. Identify agreements (same solution across forks)
3. Identify disagreements (different solutions, tradeoffs)
4. Write analysis to `scratch/synthesis.md`
5. Implement the unified solution in the current worktree

## Analysis structure

Write `scratch/synthesis.md`:

```markdown
# Synthesis

## Fork approaches
- **Fork 1 (direction)**: <approach summary>
- **Fork 2 (direction)**: <approach summary>
- **Fork 3 (direction)**: <approach summary>

## Agreements
<What all forks did the same way>

## Disagreements
| Issue | Fork 1 | Fork 2 | Fork 3 |
|-------|--------|--------|--------|
| <issue> | <choice> | <choice> | <choice> |

## Resolution
<Which approaches to take and why>
```

## Resolution principles

**Unanimous = adopt.** If all forks made the same choice, use it.

**Majority with clear rationale = adopt.** If 2/3 agree and the reasoning is sound, use it.

**Split decisions = evaluate tradeoffs.** Consider the direction of the synthesize step (if specified) to break ties.

**Conflicts = document and choose.** Note the tradeoff in scratch/, make a call, move on.

## Output

Working code that incorporates the best of each fork. The synthesis doc explains the reasoning for human review.
