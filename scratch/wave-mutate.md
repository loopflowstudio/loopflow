# Chord Played — 2026-03-21

## Source
scratch/garden-assessment.md (five-wave health assessment)

## Summary
The assessment identified model's sequencing as misaligned with the chord's phase transition. Tend-flow-steps — the gate to chord autonomy — was priority 3 behind pure engine work that doesn't unblock downstream waves. Promoted it to priority 1 so model can work on it as soon as lfd lands. Also closed a config gap in pm.

## Mutations

### 1. Promote tend-flow-steps to priority 1
**Wave**: model
**Lever**: items
**Before**: `3-tend-flow-steps.md` — priority 3, behind wave-modes (1), concurrent-ingest (2), planning-flow (2), wave-crons (2)
**After**: `1-tend-flow-steps.md` — priority 1, alongside wave-modes
**Rationale**: The garden assessment identifies the phase transition from "build foundations" to "wire end-to-end" as the chord's current inflection point. Tend-flow-steps is the gate — it proves the garden cycle runs live. With two priority-1 items, model can work wave-modes when lfd is down and switch to tend-flow-steps when lfd is up. Neither blocks the other.
**Risk**: Model wave picks tend-flow-steps first but lfd is still down, wasting a cycle. Low risk — the item description explicitly starts with "boot lfd," so the wave will naturally route to wave-modes if lfd isn't available.
**Files changed**: `wave/model/3-tend-flow-steps.md` → `wave/model/1-tend-flow-steps.md`
**Status**: applied

### 2. Add missing mode field to pm
**Wave**: pm
**Lever**: lifecycle
**Before**: No `mode` field in pm.yaml (implicit default)
**After**: `mode: manual` — explicit, consistent with all other waves
**Rationale**: Every other wave declares mode explicitly. The omission is a config gap that could cause confusion if mode defaults change.
**Risk**: None — this makes explicit what was already the effective behavior.
**Files changed**: `wave/pm/pm.yaml`
**Status**: applied

## Deferred

- **PR #596 CI failure (lfd)**: The single highest-leverage fix for the entire chord. Cannot be addressed from the root wave — requires work in the lfd worktree. The promotion of tend-flow-steps ensures model is ready to move the moment lfd lands.
- **Wave rename completion**: Worktrees still use old names (chord-model, agent-embedding, dogfood). Operational task that requires creating new worktrees and registering waves in lfd. Not a wave-config mutation.
- **Branch cleanup**: 28 stale remote branches, 35 worktrees on disk. Operational hygiene, not wave config. `lf op wt prune` when ready.
- **Validation gap**: 17 PRs landed with no live lfd validation. This resolves naturally once lfd is running — no config change accelerates it beyond fixing PR #596.
