# Open choices (ab74ab)

- Bare `lf sync` follows a closed parent to its successor row even when that row is unpublished (no GitHub PR yet), matching `select_stack_parent`. Simpler alternative: follow only a GitHub-open PR.
- Concept review: `task_stack` follows the parent Task to its successor on read and the sync records it, replacing the separate `restack_closed_parent` pass. Landing and publication follow too; they refuse only when no successor is live.
- A followed successor that already merged counts as a merged parent (bare sync moves onto the default branch) instead of the two-command error the brief describes for "none".
