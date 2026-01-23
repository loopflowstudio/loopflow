# v0.6.10

Adds flexible branch naming templates, global config with auto-pruning for merged worktrees, and new prompt variants for quality gates and codebase-wide analysis. Also renames 'loop' to 'job' across the daemon and aligns Swift/TypeScript models with the Python schema.

## Changes

- Add `branch_names.schema` config for custom worktree branch naming (e.g., `{user}.{name}.{date}`)
- Add global config support and auto-prune for merged worktrees
- Add gate prompt variants (`-gate` suffix) for fast inner-loop quality checks
- Add big prompt variants (`-big` suffix) for strategic codebase-wide assessment
- Rename `loop` to `job` in lfd daemon
- Align Swift and TypeScript models with Python runs/triggers schema
