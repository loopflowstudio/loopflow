# 03: Run Visibility (sketch)

Improve what you see while the agent is working autonomously. Design sketch — not yet scoped for implementation.

**Status: backlog sketch**

## Ideas

### Turn-by-turn file change summary

During autonomous runs, each completed turn surfaces which files were touched. Not checkpoints or revert — just visibility.

Requires lfd tracking file changes per turn, possibly via `.diffUpdated` session events (currently ignored by SessionState).

### Live commit feed

Commits animate into `commitLogSection` as they land during a run. Gives a sense of momentum.

### Progress-aware diff stat

Real-time file count during runs: "4 files changed so far" that updates as commits land. More informative than the static `--stat` string.

## Prior art

Inline glanceability (02) established patterns this can build on:

- **Per-file diff endpoint**: `GET /waves/:id/diff?path=...` returns truncated unified diff from lfd. "Progress-aware diff stat" can extend this to stream updates during runs.
- **`DiffLinesView`**: Shared component for colored inline diffs. Reusable for turn-by-turn file change summaries.
- **Expand-on-tap pattern**: `expandedSections: Set<String>` on WaveDetailPanel for lazy content reveal. Same pattern applies to expanding run details.

## Open questions

- How much run-time visibility is useful vs. distracting?
- Should turn-by-turn changes be per-session (interactive) or per-run (autonomous)?
- What's the right granularity — per-turn, per-commit, or per-step?
