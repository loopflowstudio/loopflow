# maestroux

Adds native git watching, auto-sync + staleness tracking in Maestro, plus publish automation and rebase assistant tweaks.

## Review

**Verdict:** Needs work

- Auto-prune does not guard against active sessions; a worktree with a running session could be removed if its branch is merged or remote-deleted. Consider checking `AppState.activeWorktreePaths` or a session flag before pruning. `Maestro/Maestro/AppState.swift`.
- Staleness detection hardcodes `main` for merge checks and commit age, so repos with a different default branch may be misclassified or pruned incorrectly. Use the repo default/base branch instead. `Maestro/Maestro/Services/WorktreeService.swift`.
- No tests cover staleness classification or prune candidate selection. Consider adding unit tests around `detectStaleness` and the auto-prune filter to lock behavior. `Maestro/Maestro/Services/WorktreeService.swift`, `Maestro/Maestro/AppState.swift`.

## Design notes

- Two-layer refresh: FSEvents git watcher for baseline updates plus lfd socket for live sessions.
- Auto-sync timer runs `lfops sync` every 120s and refreshes worktrees.
- Staleness uses four states: active, merged, remote deleted, inactive (days), with a 14-day inactive threshold.
