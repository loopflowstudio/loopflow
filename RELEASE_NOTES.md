# v0.6.5

This release refactors the package structure for better organization, splitting loopflow into separate lf/, lfops/, and lfd/ packages.

## Changes

- Reorganize src/loopflow into lf/, lfops/, lfd/ packages for cleaner module boundaries
- Simplify demo tapes with stub commands for cleaner recordings
- Remove pydantic-ai dependency, running agents via CLI instead
- Rebuild Maestro as worktree detail view with collapsed launcher and embedded terminal
- Update documentation with demo gifs and clearer structure
