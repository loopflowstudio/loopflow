# v0.6.6

This release introduces agent loops for continuous goal-driven automation, external skill sources (superpowers integration), and major Maestro improvements including live git watching and a new worktree dashboard. It also adds image clipboard support, the `lf add` command for scaffolding prompts, and improved PR generation with base-branch diffs.

## Changes

- Add `lfd loop` command for continuous goal-driven automation with configurable PR limits and background execution
- Add external skill sources - run skills from superpowers library via `lf sp:<skill>` syntax
- Add live git watching in Maestro with staleness detection for merged, deleted, or inactive worktrees
- Redesign Maestro worktree panel as workflow dashboard with quick actions, history, and inline diffs
- Add `lf add` command to scaffold new prompt files in `.claude/commands/`
- Add image clipboard support - screenshots and images are now included in prompt context
- Add `lfops sync` and `lfops prune` commands for worktree maintenance
- Add `--refresh` flag to `lfops pr` and use PR base diff for accurate title/body generation
