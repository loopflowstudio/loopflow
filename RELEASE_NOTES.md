# v0.6.0

Adds the lfd daemon for background agent orchestration, voices for reusable personas, and a redesigned CLI with separate commands for worktree and PR operations.

## Changes

- Add `lfd` daemon for session tracking and background agent orchestration
- Add voices: reusable personas that shape agent responses
- Split CLI into `lf`, `lfwt`, `lfops` commands for clearer separation
- Add `diff_files` context source: load full content of files changed on branch (now default)
- Add `lfops land --local` for squash-merging without a PR
- Add `lfops recover` command for worktree recovery
- Add `wtdoctor` to detect and fix squash-merged worktrees
- Improve `lfops land` reliability with better merge verification and cleanup
- Add Maestro UI enhancements: context chips, file drops, live worktree updates
