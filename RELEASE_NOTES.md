# v0.6.7

This release adds automated screenshot pipelines with Maestro, a roadmap system for agent work selection, and interactive project setup. It also introduces linting and formatting infrastructure with ruff.

## Changes

- Add `lf init` command with interactive setup for lint-before-ship and project configuration
- Add Maestro integration for automated screenshot pipelines
- Add roadmap system for guiding agent work selection toward goals
- Add ruff-based linting and formatting infrastructure
- Support configurable branch naming schema for worktrees
- Add global task discovery from `~/.claude/commands/` for user-wide tasks
- Support multiple loops per goal with distinct areas in `lfd`
- Add `voices` parameter to context building for prompt customization
