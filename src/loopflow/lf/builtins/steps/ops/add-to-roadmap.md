---
requires: scratch/ files to promote
produces: roadmap/ files
---
Promote files from scratch to the permanent roadmap.

## Path rules

Frontmatter overrides path. If no frontmatter, preserve existing path structure.

**Frontmatter overrides:**
```
scratch/api/auth.md with `area: cli` → roadmap/cli/auth.md
scratch/proposal.md with `area: api` → roadmap/api/proposal.md
```

**Path preserved (no frontmatter):**
```
scratch/api/auth.md → roadmap/api/auth.md
scratch/proposal.md → roadmap/proposal.md
```

## Workflow

1. Find files in `scratch/` that should be promoted (skip temporary analysis files)
2. For each file:
   - If `area:` frontmatter exists, use it as the destination path
   - Otherwise, preserve the relative path from scratch/
3. Create corresponding file in `roadmap/`
4. Remove promoted files from `scratch/`

## What to promote

Promote files that represent decisions or plans:
- `scratch/roadmap-proposal.md`
- `scratch/<area>/item.md`

Skip analysis artifacts that informed the decision:
- `scratch/research.md`
- `scratch/simplification-opportunities.md`
- `scratch/polish-priorities.md`

## Validation

- Destination must be determinable (path or frontmatter)
- If destination already exists, fail with error
