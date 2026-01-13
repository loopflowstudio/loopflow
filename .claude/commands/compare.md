---
requires: two branches (injected via lf ops compare)
produces: comparison.md
---
Compare two implementations and recommend which to use.

## Goal

Give the human an informed recommendation fast. They have two implementations and need to pick one—your job is to compress the decision. Be opinionated. A clear recommendation they can disagree with is better than a balanced analysis they have to interpret.

This task is typically invoked via `lf ops compare <branch-a> <branch-b>`, which injects the diffs below.

## Implementation A: {{name_a}}

<diff>
{{diff_a}}
</diff>

## Implementation B: {{name_b}}

<diff>
{{diff_b}}
</diff>

## Workflow

1. Read both diffs to understand what each implementation does
2. Identify the key differences in approach
3. Evaluate against loopflow's design principles (below)
4. Write your analysis to `{{output_dir}}/comparison.md`

## What to cover

**Approach.** How does each solve the problem? What are the architectural differences? One-paragraph summary for each.

**Trade-offs.** What does each gain and lose? Consider:
- Code complexity (less code is usually better)
- Match with existing patterns in the codebase
- Error handling and edge cases
- Testability

**Recommendation.** Which implementation should be used? Be specific. If parts of each are better, note what to cherry-pick.

## Evaluation criteria for loopflow

When comparing, prefer implementations that:

- **Use fewer lines of code.** The best code is code that doesn't exist.
- **Match existing patterns.** If the codebase uses `@dataclass`, use `@dataclass`. If it uses functions, use functions.
- **Avoid new abstractions.** Don't prefer the implementation that adds a base class or registry.
- **Work in auto mode.** Implementations that require interactive confirmation are worse.
- **Delegate to CLIs.** Implementations that shell out to claude/codex are better than ones that reimplement.

## Output format

Write to `{{output_dir}}/comparison.md`:

```markdown
# {{name_a}} vs {{name_b}}

## Summary

<Which to use and why, in 2-3 sentences>

## Approach comparison

<One paragraph per implementation>

## Recommendation

<Specific recommendation with rationale>

## Cherry-pick opportunities

<If applicable: what to take from the non-recommended implementation>
```

Keep it under 500 words. This is a decision document, not an exhaustive review.

