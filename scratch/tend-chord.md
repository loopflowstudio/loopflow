# Chord — 2026-03-17

## Context
The assessment says the redesign chord is still in Phase 1, but attention is split the wrong way: `chord-model` still lacks a live tend proof, `signals` has not crossed from taxonomy into code, `clear-the-deck` is out of phase with its roadmap, and `agent-embedding` is at risk of widening before its queue work lands. The chord should narrow active pressure to the two Phase 1 tracks that actually unblock the system and quiet the rest.

## Mutations

### 1. Move live-runtime proof fully into `chord-model/02`
**Wave**: `chord-model`
**Lever**: `items`
**Before**: `01-algedonic-signals.md` still carries a live-demo section with LF_HOME/dev-token isolation, PR-state sync, and demo-harness gaps, while `02-tend-flow-steps.md` separately owns the first live tend cycle. Two adjacent items are both claiming the same missing runtime proof.
**After**: Trim `01-algedonic-signals.md` back to the repair/escalation plumbing and testable slices it already owns, and move the live-demo/runtime checklist into `02-tend-flow-steps.md` so item 02 becomes the single Phase 1 proving item for lfd-backed execution, `lfq show --json`, and reproducible live tend evidence.
**Rationale**: The blocker is not more architecture; it is one missing operational proof. Putting all live-runtime ownership in item 02 gives the wave one finish line instead of two near-duplicates competing for attention.
**Risk**: `01` will look less “done” end to end until `02` closes, and some algedonic-demo work may feel delayed even though the implementation plumbing is already in place.
**Verdict**: apply
**Notes**: Consolidate the live tend proof under `chord-model/02` so the wave has one operational finish line.

### 2. Narrow `signals/01` to the first additive block slice
**Wave**: `signals`
**Lever**: `items`
**Before**: `01-block-taxonomy.md` names seven initial block types and a broad API/model surface. Even though the item says to stay additive, the current writeup still invites taxonomy expansion before any code ships.
**After**: Rewrite `01-block-taxonomy.md` to ship only the block core that unblocks the rest of the chord now: `BlockType`, `Block`, create/list/resolve APIs, and the first concrete block types that are already real in this redesign (`ci_failure`, `merge_conflict`, and `stall`). Move `shallow_work`, `human_drift`, `capability_gap`, and other calibration-heavy signals into later items.
**Rationale**: The wave is stalled because the first item is still broad enough to stay conceptual. A smaller additive slice turns `signals` into code quickly and gives `agent-embedding` and later tend work real block types instead of placeholders.
**Risk**: The first slice may feel too mechanical and leave the more distinctive qualitative-signal work for later, so the follow-on items need to stay explicit.
**Verdict**: reject
**Notes**: Attention Items may already be the right action surface. Adding a separate `Block` model risks duplicating queue semantics instead of extending the existing attention contract.

### 3. Put `clear-the-deck` into coherence mode instead of shipping mode
**Wave**: `clear-the-deck`
**Lever**: `flow`
**Before**: `clear-the-deck.yaml` still runs `flow: ship-wave` even though the active branch is carrying stale pre-collapse residue, diverged history, and roadmap drift.
**After**: Delete `clear-the-deck` instead of keeping it active. Drop the stale cleanup branch and wave scaffolding rather than spending another cycle trying to reconcile it.
**Rationale**: The wave is overlapping heavily with active backend work, has drifted from its roadmap, and no longer looks like a distinct lane worth tending.
**Risk**: Some cleanup ideas may disappear with the wave and need to be reintroduced later, in a narrower form, if they still matter.
**Verdict**: apply
**Notes**: Don’t put `clear-the-deck` into a holding pattern. Clear it out entirely.

### 4. Re-sequence `agent-embedding` so item 01 lands before item 02 widens
**Wave**: `agent-embedding`
**Lever**: `items`
**Before**: The roadmap and assessment both say the attention queue should complete before terminal embedding broadens the shell, but `01-attention-queue-completion.md` and `02-terminal-embedding.md` are effectively active at the same time.
**After**: Update the item docs so `01-attention-queue-completion.md` explicitly owns any queue-related plumbing needed for the current PR, and `02-terminal-embedding.md` becomes a queued follow-on that starts only after queue coverage lands cleanly.
**Rationale**: This wave already has momentum and the only open PR in the chord, even though that PR is currently blocked by a failing Rust check. The smallest useful intervention is not to reshape it, but to restore the promised sequence so the queue becomes the stable home screen before Ghostty work opens another front.
**Risk**: Terminal embedding loses some immediacy, and a small amount of backend prep may need to be revisited later once item 01 is fully landed.
**Verdict**: defer
**Notes**: The in-progress `agent-embedding` wave has already evolved past the original wording: `02-terminal-embedding.md` is now `02-daemon-owned-pty-transport.md`, and the branch already spans queue, PTY transport, and workspace work. Keep the sequencing concern in mind, but don’t rewrite the roadmap mid-flight.

### 5. Fold `signals` back into `chord-model`
**Wave**: `signals`, `chord-model`
**Lever**: `wave structure`
**Before**: `signals` exists as its own wave with five items, but in practice it is still a design-doc-only branch with no code or PR, heavy area overlap with `chord-model`, and a Phase 1 framing that assumed a distinct block model.
**After**: Retire `signals` as an independent wave and move any surviving work into `chord-model` follow-ons. Keep the useful parts — stall detection, self-healing / algedonic escalation polish, and later memory/pattern work — but drop the standalone block-taxonomy framing.
**Rationale**: `signals` is not earning a separate lane right now. The strongest remaining ideas read more like chord behavior and algedonic polish than a distinct system, especially if Attention Items already own the human action surface.
**Risk**: Folding the wave could blur useful future work if it is not re-homed clearly, and `chord-model` could sprawl if these ideas are merged back without pruning.
**Verdict**: apply
**Notes**: There is no obvious standalone win in `signals` as its own wave. Re-home the worthwhile parts under `chord-model`.

## Coherence
Mutations 1 and 2 are the active Phase 1 pair: `chord-model` proves the chord can tend live, and `signals` ships the first real block surface in parallel. Mutation 3 reduces interference by making `clear-the-deck` do housekeeping instead of shipping overlapping cleanup while those two waves are touching shared infrastructure. Mutation 4 keeps `agent-embedding` shipping its existing PR, but stops it from widening into a second backend/front-end front before the queue contract is complete.

The intended effect is deliberate imbalance: only `chord-model` and `signals` actively push new shared-system work this cycle. `clear-the-deck` becomes quiet internal maintenance, and `agent-embedding` focuses on landing what is already convincingly in motion. None of the four mutations require the others to merge first, but the review order should stay the same as above because each lower-priority mutation mainly protects the focus created by the ones above it.

## Deferred
- No redesign-wave mutation yet. The root chord already has the right `flow: tend`; the problem is member-wave focus, not chord structure.
- No area or direction changes yet for `chord-model` or `signals`. The current pressure is sequencing and scope inside the items, not a demonstrated mismatch in the wave identities.
- No lifecycle split or deletion for `clear-the-deck`. A `reorg` pass should decide whether that stronger intervention is actually needed.
- No additional mutation for `agent-embedding` beyond sequencing. The current PR is active but blocked, so the goal is to avoid widening further while that existing slice gets unstuck and landed.

## Session Notes
- `chord-model/02` should be the single owner of the live tend proof.
- Reject a separate `Block` model for now; Attention Items likely already cover the human action surface.
- `clear-the-deck` should not be preserved as a quiet cleanup wave; delete it instead.
- `signals` did not show an obvious standalone win as a full wave. Its surviving value is mostly algedonic/chord polish, so fold that work back into `chord-model`.
- The in-progress `agent-embedding` wave has already moved from terminal-embedding framing to daemon-owned PTY transport. Keep sequencing pressure in mind, but don’t churn the roadmap mid-flight.
