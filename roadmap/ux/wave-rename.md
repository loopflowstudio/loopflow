---
status: proposed
---

# Rename `roadmap/` → `wave/`

The plan and the execution config are the same thing. `roadmap/` implies a document you read. `wave/` implies something you run.

## What to build

Rename `roadmap/` to `wave/` across the codebase. Each file in `wave/` is both the plan and the wave spec — no separation between what to build and how to wave it.

## Approach

- Rename `roadmap/` directory to `wave/`
- Update all prompts, docs, and code that reference `roadmap/`
- Update `lf roadmap`, `lf add-to-roadmap`, `lf iterate` to use `wave/`
- Update CLAUDE.md references

## Done when

- `roadmap/` no longer exists
- All references point to `wave/`
- Prompts read/write `wave/` directory
