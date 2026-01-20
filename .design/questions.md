# Open Questions

## Maestro Live Reactivity

1. **Working directory watching (Phase 3)** - The design mentions watching worktree working directories for diff refresh. This would require:
   - Extending GitWatcherService to watch `<worktree>/**`
   - Ignoring noise (node_modules, .git/objects, build artifacts)
   - Per-worktree watch lifecycle (add watch when worktree selected)

   Is this needed for the first implementation, or can it be added later?

2. **Auto-pruning (Phase 5)** - The design includes:
   - "Prune Stale" button in sidebar
   - User preference for auto-prune after merge
   - Merge queue completion detection

   Is this needed for the first implementation, or is stale detection (implemented) sufficient for now?

3. **Staleness threshold** - Currently hardcoded to 14 days of inactivity. Should this be configurable?
