---
requires: scratch/ files to promote
produces: roadmap/ files
---
Move files from scratch/ to roadmap/.

## Determining destination

Use the **area from this request** as the destination folder. The area is passed via `--area` flag or inherited from a wave's configuration.

If the area is a pathset (e.g., `src/cli/, src/commands/`), use the first path as the roadmap folder name (e.g., `cli`).

**Destination:** `roadmap/<area>/<filename>.md`

**Examples:**
```
--area src/api/ + scratch/auth-redesign.md → roadmap/src/api/auth-redesign.md
--area src/cli/,src/commands/ + scratch/ux.md → roadmap/src/cli/ux.md
(no area) + scratch/research.md → roadmap/research.md
```

## Workflow

1. Read everything in scratch/
2. For each file worth promoting:
   - Determine which area it belongs to
   - Create destination path: `roadmap/<area>/<filename>.md`
   - Move content (don't just copy—remove from scratch/)
3. Skip temporary analysis files that shouldn't persist

## What to promote

**Promote:**
- Proposals with clear scope
- Research with lasting value
- Decisions that should be remembered

**Skip:**
- Working notes that informed decisions
- Intermediate analysis superseded by synthesis
- Branch-specific design docs (these get cleared on merge anyway)

## Validation

- Every promoted file must have a clear destination
- If destination already exists, merge or fail (don't silently overwrite)
