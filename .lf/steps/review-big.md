---
requires: none
produces: scratch/codebase-review.md
---
Look at this codebase through a user's eyes—human or digital. Find the highest-leverage improvements.

Not an exhaustive audit. Clarity on where to focus, not a list of everything that could be better.

## Workflow

1. Start with README. Try to follow getting started instructions.
2. Look at CLI help and error messages. Are they helpful to humans? Parseable by agents?
3. Find the main entry points. Is the happy path obvious?
4. Explore broadly: structure, core code, tests
5. Ask: what single change would most improve the experience for users—whether they're people or programs?

## Output

Write `scratch/codebase-review.md`:

```markdown
# Codebase Review

## First impressions
<What a new user encounters. Be specific.>

## The one thing
<Highest-leverage change. What and why.>

## Friction points
<2-3 other places users get stuck>

## Quick wins
<Small changes with outsized impact, if any>
```

Focus on actionable. Skip theoretical concerns.
