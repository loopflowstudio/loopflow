# Open questions

- Docker fork branches rely on host-side git worktrees for prompt assembly. This works today because prompt assembly happens before container launch and worktrees are cleaned up after. Should we leave this as-is, or invest in a prompt build path that doesn't need host worktree materialization? (Not blocking — revisit if it causes problems during Phase 05 dogfooding.)
