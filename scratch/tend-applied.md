# Chord Applied — 2026-03-18

## Applied

### 1. Scan step: add unlanded branch visibility
**Wave**: redesign (chord-wave)
**Change**: Added step 4 to `scan-waves.md` — scan remote branches and local worktrees for work ahead of main. Renumbered subsequent steps.
**Files modified**: `rust/loopflow/src/engine/builtins/steps/tend/scan-waves.md`

### 2. Review-chord step: rewrite for coherence
**Wave**: redesign (chord-wave)
**Change**: Created `.lf/steps/tend/review-chord.md` as a repo override of the builtin. Replaces mutation-by-mutation approve/defer/reject with a conversation about what's working and what isn't.
**Files modified**: `.lf/steps/tend/review-chord.md`

## Deferred

### 3. lfd wave joins the chord
**Reason**: `wave/lfd/` was bootstrapped on the agent-embedding branch (uncommitted). It lands when agent-embedding's branch passes CI and merges. Once on main, `wave/redesign/redesign.yaml` area should be updated to include `wave/lfd/`.
