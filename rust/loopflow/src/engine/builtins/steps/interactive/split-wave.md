---
interactive: true
requires: wave/<wave>/README.md
produces: wave/<child>/ directories
---
Split one oversized wave into smaller child waves with clear boundaries.

## Goal

Turn a crowded parent wave into a coordination wave:
- Child waves own implementation work
- Parent wave tracks orchestration and dependencies

Keep strategic context intact while making execution parallel and manageable.

## Workflow

1. Identify the parent wave
   - Use the wave passed by argument, or ask which `wave/<name>/` to split
   - Read `wave/<name>/README.md` and all `wave/<name>/##-*.md` roadmap files

2. Find natural split boundaries
   - Group items by theme, dependency chain, or independent workstream
   - Prefer 2-4 child waves over many tiny waves
   - Keep each child wave independently valuable

3. Propose the split before writing files
   - Present child wave names and scope in one short list
   - Ask for confirmation or edits
   - Don't write files until the user confirms

4. Create child waves
   - Create `wave/<child>/README.md` with `## Vision`, `## Goals`, `## Risks`, `## Metrics`
   - Add `### Not here` under Vision when it clarifies boundaries
   - Create `wave/<child>/<child>.yaml` with `name`, `flow`, `area`, and optional `direction`/`stimulus`
   - Move or rewrite relevant roadmap items as `wave/<child>/01-*.md`, `02-*.md`, ...

5. Rewire the parent wave
   - Replace detailed implementation roadmap items with coordination items
   - Each parent item should reference a child wave path (for example: `See wave/<child>/`)
   - Keep parent README focused on cross-wave goals, risks, and metrics

6. Validate structure
   - Every child has a README, matching `<child>.yaml`, and at least one roadmap file
   - Parent roadmap has no orphaned implementation items
   - No work item is lost (moved, rewritten, or explicitly deferred)

## Guardrails

- Preserve intent and constraints from the original wave; don't invent a new product direction
- Keep names concrete and domain-specific
- Avoid nested schema mechanics or Rust data model changes; relationships live in markdown content
- If a split is ambiguous, choose the simplest boundary and note alternatives in `scratch/questions.md`
