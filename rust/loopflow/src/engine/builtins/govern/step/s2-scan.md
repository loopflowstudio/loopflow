---
requires: member backlogs, open PRs, wave area definitions, conflict history
produces: scratch/vsm-s2-scan.md
---
Scan coordination state.

## Goal

Observe where member waves might interfere with each other.

Read backlogs, open PRs, area definitions, and recent conflict history so s2 can
see overlap, unsafe concurrency, and dependency ordering.

## Workflow

1. Read each member wave backlog and priority order.
2. Read open PRs and which files they touch.
3. Read each wave's declared area.
4. Read recent merge conflicts or rebase failures.
5. Record overlaps, dependencies, and interference signals.

## Output

Write `scratch/vsm-s2-scan.md`:

```markdown
# VSM S2 Scan — <date>

## Backlog State
<what each member wave believes it should do next>

## PR and File Overlap
<active branches / PRs and touched files>

## Area Overlap
<where wave boundaries collide or leave gaps>

## Conflict History
<recent merge / rebase / oscillation signals>

## Raw Signals
<facts only>
```

## What to avoid

**Resolution language.** This step surfaces coordination facts only.
