# Open questions

- PM run-lifecycle comments/completion on a single roadmap item are still unimplemented. Current runs know the wave and worktree, but they do not retain a stable roadmap-item identity after `ingest` moves an item into `scratch/`. This implementation ships wave-level import/export hooks now and leaves item-level PR/merge comments for a follow-up once run state carries item linkage.
