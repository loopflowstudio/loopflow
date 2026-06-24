---
requires: scratch/review-*.md or review feedback
produces: scratch/<branch>.md
---
Read the review. Write a design doc to address it.

## Orientation

Before starting, orient yourself in this branch:

- Read `scratch/` — design docs and notes for the current work live here
  (`scratch/<branch>.md` is this PR's design; `scratch/questions.md` holds open
  questions and assumptions).
- If a `wave/<name>/` directory matches this work, skim its roadmap and items.
- Read the repo's agent doc (`CLAUDE.md` / `AGENTS.md`) for conventions.

Write design artifacts, notes, and open questions under `scratch/`. Don't
re-derive what these already record.

## Scope

The included context defines your area of responsibility. Address issues within that scope. If the review mentions problems outside your area, note them but don't design fixes for them—stay focused on what you own.

## Workflow

1. Read the review in scratch/ or the feedback provided
2. Identify the highest-impact improvement to address
3. Write a focused design doc in scratch/<branch>.md
4. The design feeds into `build` (implement → compress → gate → update-wave)

## Focus

One improvement per iteration. Pick the most important issue from the review, design the fix. Don't try to address everything at once.

The design doc should be concrete enough for `implement` to act on. What files change? What's the approach? What does "done" look like?
