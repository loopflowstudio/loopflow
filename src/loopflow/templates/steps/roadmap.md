---
interactive: true
produces: <area>/roadmap/<item>.md
---
Propose a new roadmap item.

## Process

1. Read `roadmap/` to understand the project's global direction and priorities
2. Consider the user's request: {args}
3. Evaluate honestly: where does this fit in the vision?
4. Create a roadmap item at `<area>/roadmap/<slug>.md`

## Output format

Write the roadmap item with this structure:

```markdown
---
status: proposed
area: <area>
---

# Title

One paragraph describing what and why.

## Scope

- What's included
- What's explicitly not included

## Approach

Technical direction. Not a full design doc—just enough to unblock building.
```

## Guidelines

- Focus on substantial work, not small fixes
- Be honest about scope—what's in, what's out
- Pick the right area based on what this primarily affects
- The approach section should have enough detail that someone could start building

If the request is too vague, ask clarifying questions before writing.
