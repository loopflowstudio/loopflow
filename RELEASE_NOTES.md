# v0.7.0

This release introduces DAG-based flow execution with fork/synthesize patterns, a redesigned Concerto UI centered on agents with sidebar navigation and detail panels, and persistent agent worktrees that survive across iterations. The daemon now automatically resets on schema mismatch and consolidates migrations into a baseline schema.

## Changes

- Add DAG-based flow execution with `fork` and `synthesize` steps for parallel agent workflows
- Redesign Concerto UI with agent-centric sidebar, detail panel, and flow picker
- Persist agent worktrees across iterations and move branch on completion
- Add `lfops next` command to land PR and continue work on a stacked branch
- Consolidate database migrations into baseline schema with auto-reset on mismatch
- Rename `voice` to `goal` and `trigger/mode` to `stimulus` throughout codebase
- Remove website deploy from publish workflow; add `--version` flag using PyPI API
