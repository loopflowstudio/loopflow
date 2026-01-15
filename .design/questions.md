# Open Questions

## Pipeline Editor (glade-motif)

### Synthesis task behavior
Should there be a built-in "synthesize" task, or should users specify which task to run for synthesis?

Current leaning: Built-in task with option to override via `merge_task: my-custom-synthesizer` in the branch block.

### Branch naming in synthesis
When displaying branch diffs to the synthesis task, what should the branch names be?
- Option A: Use the voice name if configured (e.g., "architect", "minimalist")
- Option B: Use sequential numbers ("branch-1", "branch-2")
- Option C: Let users provide explicit branch names in the YAML

Current leaning: Option A if voice is set, else Option B.

### Worktree cleanup on failure
If a branch fails partway through, should we keep the temp worktrees for debugging or always clean them up?

Current leaning: Clean up by default, add `--keep-branches` flag for debugging.
