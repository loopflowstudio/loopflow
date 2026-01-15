# Open Questions

## Pipeline Editor (glade-motif)

### Config.yaml migration
Should we migrate existing `config.yaml` pipelines to `.lf/pipelines/*.yaml`?

Current leaning: Support both. Simple task lists in config.yaml continue to work. `.lf/pipelines/` is preferred for new pipelines with per-step config.

---

## Deferred: Branching pipelines

These questions apply to the deferred branching feature:

### Synthesis task behavior
Should there be a built-in "synthesize" task, or should users specify which task to run for synthesis?

### Branch naming in synthesis
When displaying branch diffs to the synthesis task, what should the branch names be?

### Worktree cleanup on failure
If a branch fails partway through, should we keep the temp worktrees for debugging or always clean them up?
