# v0.6.9

This release renames Maestro to Concerto with a new app icon, adds the --web flag to launch Claude web sessions, and improves database reliability with automatic schema migration. Worktrees now push to create remote branches automatically.

## Changes

- Add `--web` flag to `lf` command to launch Claude web sessions instead of CLI
- Rename Maestro app to Concerto with new app icon
- Auto-reset database on schema mismatch with migration support
- Automatically push to create remote branch when creating worktrees
- Add LoopflowCore shared Swift framework for code reuse across apps
- Unify design system colors across Swift, web, and documentation
- Handle binary content gracefully in git diffs
