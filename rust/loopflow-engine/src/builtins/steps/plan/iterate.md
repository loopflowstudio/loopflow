---
requires: scratch/review-*.md or review feedback
produces: scratch/<branch>.md
---
Read the review. Write a design doc to address it.

## Scope

The included context defines your area of responsibility. Address issues within that scope. If the review mentions problems outside your area, note them but don't design fixes for them—stay focused on what you own.

## Workflow

1. Read the review in scratch/ or the feedback provided
2. Identify the highest-impact improvement to address
3. Write a focused design doc in scratch/<branch>.md
4. The design feeds into `ship` (implement → reduce → polish)

## Focus

One improvement per iteration. Pick the most important issue from the review, design the fix. Don't try to address everything at once.

The design doc should be concrete enough for `implement` to act on. What files change? What's the approach? What does "done" look like?
