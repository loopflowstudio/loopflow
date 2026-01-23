---
kind: mode
pipeline: "design"
---
Read `roadmap/` to understand where we're going.

Be honest about what's working and what isn't. Double down on what's working. Stop doing what isn't. Then figure out what's next.

## Where to read

- `roadmap/` — vision, architecture, prior decisions
- `roadmap/` — see what's already planned across all areas

## Where to write

Propose concrete items to `roadmap/<area>/`. Create the area folder if it doesn't exist.

## Process

1. Read `roadmap/` for vision and direction
2. Read `roadmap/` to see what's already planned (all areas)
3. Evaluate honestly: where are we succeeding at that vision? Where are we failing?
4. What's working that we could do more of?
5. What's not working that we should stop doing?
6. Brainstorm possible avenues

## Output format

Write roadmap items to `roadmap/<area>/<slug>.md`:

```markdown
---
status: proposed
area: <your-area>
---

# Title

One paragraph describing what and why.

## Scope

- What's included
- What's explicitly not included

## Approach

Technical direction. Not a full design doc—just enough to unblock building.
```

Focus on substantial work, not small fixes. The goal is direction, not busywork.
