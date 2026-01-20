# Unique Iteration Branch Names

## What to build

Iteration branches derive their prefix from `loop_main` instead of `goal_name`, making them unique across loops with the same goal but different areas.

## The Problem

Current branch naming:
- Loop-main: `{goal_name}-main`, `{goal_name}-1-main`, etc. (unique per loop)
- Iteration: `{goal_name}/{iteration:03d}` (not unique!)

Example collision:
```
Loop A: goal=product-engineer, area=src/api/, loop_main=product-engineer-main
Loop B: goal=product-engineer, area=src/ui/, loop_main=product-engineer-1-main

Both create: product-engineer/001  ← collision!
```

## Data structures

No new types. Changes to existing functions only.

## Key functions

```python
# In loop_runner.py

def _iteration_branch_prefix(loop_main: str) -> str:
    """Derive iteration branch prefix from loop-main.

    'product-engineer-main' → 'product-engineer'
    'product-engineer-1-main' → 'product-engineer-1'
    """

# In run_iteration():
# Before:
branch = f"{loop.goal_name}/{iteration:03d}"

# After:
prefix = _iteration_branch_prefix(loop.loop_main)
branch = f"{prefix}/{iteration:03d}"
```

Result:
```
Loop A: product-engineer/001, product-engineer/002
Loop B: product-engineer-1/001, product-engineer-1/002
```

## Constraints

- Must strip `-main` suffix cleanly (the suffix is guaranteed by `_allocate_loop_main`)
- No migration needed: new loops get unique names, existing loops continue working
- PR titles still use `loop.goal_name` so they're human-readable

## Done when

```bash
# Create two loops for the same goal
lfd loop product-engineer -a src/api/
lfd loop product-engineer -a src/ui/

# Both start without branch collision
lfd status
# Shows both running with distinct loop_mains

# Iteration branches are distinct
git branch | grep product-engineer
# product-engineer/001
# product-engineer-1/001
```
