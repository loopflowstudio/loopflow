---
requires: scratch/vsm-s2-scan.md
produces: scratch/vsm-s2-assessment.md
---
Assess coordination and safe ordering.

## Orientation

Before starting, orient yourself in this branch:

- Read `scratch/` — design docs and notes for the current work live here
  (`scratch/<branch>.md` is this PR's design; `scratch/questions.md` holds open
  questions and assumptions).
- Read wave/PM context only when the seed names the exact wave, task, project,
  or a concrete coordination question; never infer it or repair access as a
  prerequisite.
- Read the repo's agent doc (`CLAUDE.md` / `AGENTS.md`) for conventions.

Write design artifacts, notes, and open questions under `scratch/`. Don't
re-derive what these already record.

## Goal

Turn the coordination scan into a conflict map and ordering judgment.

Decide where member waves overlap, which work can safely proceed in parallel,
and what dependencies or trigger relationships should change.

## Workflow

1. Read `scratch/vsm-s2-scan.md`.
2. Judge overlap, conflict risk, and oscillation patterns.
3. Identify safe ordering and concurrency constraints.
4. Produce interference fixes and ordering recommendations.

## Output

Write `scratch/vsm-s2-assessment.md`:

```markdown
# VSM S2 Assessment — <date>

## Summary
<main coordination picture>

## Conflict Map
| Waves | Conflict | Risk | Recommended fix |
|-------|----------|------|-----------------|
| ... | ... | ... | ... |

## Safe Ordering
<which work should run first, later, or concurrently>

## Trigger / Dependency Changes
<changes worth making>

## Pressure Points
1. <highest-leverage coordination concern>
2. <second>
3. <third, if needed>
```

## What to avoid

**Optimistic concurrency.** If two efforts might collide, treat that as real.
