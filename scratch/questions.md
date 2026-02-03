# Open questions

- Wave integration: do we stub with local metadata only, or defer until wave module is ported?
- Optional `wt` CLI dependency: should we emit events if `wt` is installed, or ignore it entirely for Phase 1?
- Shell integration scope: zsh/bash only in Phase 1, or include fish with a simpler directive mechanism?
- Lint integration for `lf ops land/commit`: always run `lf lint`, or respect a configurable command?
- Worktree path layout: implemented as sibling `../<name>` to match the design doc done-when, not the Python `../repo.<name>` layout.
- `lf ops next` is implemented as a minimal preserve+new-worktree flow; PR/stack/auto-merge logic is deferred.
- Shell integration only supports zsh/bash for now; fish is still deferred.
