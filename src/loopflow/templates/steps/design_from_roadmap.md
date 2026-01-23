---
interactive: false
produces: scratch/<slug>.md
---
Create a scoped project design from the most recent roadmap item.

## Steps

1. Find the most recently modified roadmap item under `*/roadmap/*.md` (area-specific roadmaps).
2. If none exist, write `scratch/questions.md` explaining that no roadmap items were found and stop.
3. Read the roadmap item and extract its intent, area, and scope.
4. Create `scratch/<slug>.md` where `<slug>` is the roadmap filename without extension.

## Output format

```markdown
# <Title from roadmap>

## Roadmap Source

- Path: <roadmap/...>
- Status: <status from frontmatter if present>
- Area: <area from frontmatter if present>

## What To Build

One sentence describing what exists after this project that doesn't exist now.

## Scope

- In scope
- Out of scope

## Milestones

- Milestone 1
- Milestone 2

## Open Questions

- Any missing inputs or assumptions
```

Keep it brief and actionable. This is a scoped project plan, not a full design doc.
