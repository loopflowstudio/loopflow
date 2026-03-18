# Tend Assessment — 2026-03-17

## Summary
The chord is moving, but it is out of phase. `agent-embedding` is the only wave converting work into a live PR, while `chord-model` is blocked on the Phase 1 runtime proof, `signals` is still at design-doc stage, and `clear-the-deck` has drifted away from its current roadmap. The key tension is that bootstrap work is still unfinished, yet scope is already widening across overlapping backend surfaces.

## Wave: chord-model
**Health**: blocked
**Evidence**: Recent merged work in this lane is substantive and aligned with the roadmap: main already absorbed `#560`, `#565`, and `#569`, and the active branch `jack-heart.chord-model.20260317_2324` is clean and still carries additional shared-infra work. But the finish line that matters now is `02: Tend Flow Steps`, and the scan still shows every `lfq show ... --json` failing with `LoopflowError: invalid token`, no verified live runtime state, and no open PR for the active branch. That means the wave is working on the right problem, but the operational proof the redesign depends on still has not crossed the line.
**Pressure**: Restore a real lfd-backed tend cycle so `chord-model/02` can prove live runtime instead of accumulating more adjacent infrastructure.

## Wave: clear-the-deck
**Health**: drifting
**Evidence**: This wave recently shipped meaningful cleanup on main (`#566`, `#571`), but the active branch is now out of coherence with its own roadmap. The README says this wave is in post-collapse cleanup mode with items `03`-`05`, while the live branch `jack-heart.clear-the-deck.20260317_1840` is heavily diverged, has no open PR, still carries pre-collapse roadmap files `01` and `02`, and continues to drag older deploy/sandbox/doc changes through the diff. That is motion, but not motion toward the wave's current finish lines.
**Pressure**: Re-align the branch to the current roadmap so `04: Daemon Surface Cleanup` is the real focus instead of stale phase residue.

## Wave: agent-embedding
**Health**: steady
**Evidence**: This is the only wave with an open PR, and it already landed `#563`, so velocity and substantive output are real. The current PR `#567` is active and non-draft, but it is not cleanly landable yet: `rust-test` is failing, several other checks are still running, the worktree is dirty, and the diff reaches beyond Swift into shared `python/loopflow/` and `rust/loopflow/src/lfd/*` surfaces. The roadmap still says the attention queue should complete before terminal embedding broadens the surface, yet both items `01` and `02` are in flight together. That is healthy momentum with visible sequencing drift.
**Pressure**: Finish and land the queue-completion slice cleanly before widening further into terminal embedding and additional shared backend work.

## Wave: signals
**Health**: stalled
**Evidence**: The roadmap explicitly places `signals/01` in parallel with `chord-model/02` during Phase 1, but the active branch has only one visible commit beyond `main` and the diff is still just `scratch/signals-block-taxonomy.md`. There is no open PR, no code diff, and the branch is behind `main` by five commits. The wave is aligned in concept, but it is silent where the redesign needs additive infrastructure now.
**Pressure**: Turn `01: Block Taxonomy` into the first real block model and API slice so the rest of the chord stops depending on placeholder concepts.

## Chord-Level
**Balance**: Unbalanced. `agent-embedding` is pulling ahead on visible delivery, `chord-model` is still stuck at the Phase 1 proof point, `signals` has not entered implementation, and `clear-the-deck` is spending energy on branch drift instead of its current roadmap.
**Interference**: Shared-surface overlap is already material. `chord-model`, `signals`, and `clear-the-deck` all overlap in `rust/loopflow/src/lfd/`, `rust/loopflow/src/engine/`, and `python/loopflow/`, and PR `#567` from `agent-embedding` also reaches into those backend surfaces. That means sequencing mistakes now create real merge and review contention, not just theoretical overlap.
**Gaps**: Two important needs remain effectively unowned in practice even if they are named on paper: reliable live runtime visibility from lfd, and the first implemented block surface from `signals`. Until those exist, tend and queue work are both operating with placeholders.
**Phase**: The roadmap phase is still right: this is still Phase 1 bootstrap. Nothing in the scan justifies shifting focus to later-phase expansion before the live tend proof and first additive block types exist.

## Pressure Points
1. Recover live lfd runtime visibility so the chord can observe real state and `chord-model/02` can actually finish.
2. Move `signals/01` from taxonomy prose into additive code so block handling becomes shared infrastructure instead of deferred intent.
3. Reduce active-wave scope drift on overlapping backend areas, especially where `agent-embedding` and `clear-the-deck` are widening past their highest-leverage sequencing.
