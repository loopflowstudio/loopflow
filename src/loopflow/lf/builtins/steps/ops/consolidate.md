---
requires: scratch/ with multiple files
produces: scratch/ reorganized for roadmap
---
Reorganize scratch/ content for clean transfer to roadmap/.

## Context

Analysis steps have written multiple files to scratch/—research, opportunities, priorities, synthesis. Before promoting to roadmap/, consolidate into a structure that will make sense as persistent documentation.

## Workflow

1. Read everything in scratch/
2. Identify what should become roadmap items vs what was working notes
3. Reorganize into files that stand alone
4. Use path structure or frontmatter to declare destination

## What to consolidate

**Promote to roadmap:**
- Proposals with clear scope and approach
- Research that has lasting value
- Decisions that should be remembered

**Leave behind:**
- Working notes that informed decisions
- Intermediate analysis superseded by synthesis
- Context that's captured better elsewhere

## Output structure

Consolidate into files ready for `add-to-roadmap`:

```
scratch/
  api/
    auth-redesign.md      # → roadmap/api/auth-redesign.md
    rate-limiting.md      # → roadmap/api/rate-limiting.md
  research.md             # stays in scratch (working notes)
  synthesis.md            # stays in scratch (intermediate)
```

Or use frontmatter:

```markdown
---
area: api
---
# Auth Redesign
...
```

## Consolidation principles

**Standalone files.** Each file should make sense without reading the others. Don't assume readers saw the research.

**Clear scope.** Each roadmap item should have obvious boundaries. Split large analyses into focused items.

**Forward-looking.** Roadmap items describe what to build or what experiments to run, not what was analyzed. Transform observations into proposals.
