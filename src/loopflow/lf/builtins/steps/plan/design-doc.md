---
requires: roadmap items in */roadmap/*.md
produces: scratch/<slug>.md
---
Pick the highest-priority roadmap item for this context. Write a scoped design.

## Scope of responsibility

The included context defines your area of responsibility. Only consider roadmap items that fall within this scope. If given `src/auth/`, you own auth—don't pick items for billing or UI unless they directly affect auth.

## Workflow

1. Read all roadmap items under `*/roadmap/*.md`
2. Consider the current area/direction if specified
3. Evaluate each item: urgency, importance, dependencies, readiness
4. Pick the one that should be built next given the context
5. Create `scratch/<slug>.md` with a scoped design

## Selection criteria

**Urgency.** Is something blocked on this? Is there a deadline?

**Importance.** How much does this move the product forward?

**Readiness.** Are the prerequisites met? Is the scope clear enough to start?

**Fit.** Does it match the current area of focus or direction?

If multiple items score similarly, prefer smaller scope—ship something.

## Output format

Write `scratch/<slug>.md`:

```markdown
# <Title>

## Why this one

<1-2 sentences on why this was selected over alternatives>

## What to build

<One sentence: what exists after this that doesn't exist now>

## Scope

- In scope: ...
- Out of scope: ...

## Approach

<Technical direction, key decisions, enough to start implementing>

## Done when

<Verification command or observable outcome>
```

If no roadmap items exist or none are ready, write `scratch/questions.md` explaining what's missing.
