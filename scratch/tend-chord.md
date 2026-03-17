# Chord — 2026-03-17

## Context

Phase 1 calls for chord-model/02 and signals/01 in parallel. chord-model has done its structural work and has a green PR waiting to land. signals hasn't started. The chord can't tend for real until its member waves exist in lfd, and that's chord-model/02's finish line.

## Mutations

### 1. Wake signals/01 — block taxonomy

**Wave**: signals
**Lever**: wake
**Before**: All items queued. Zero activity. No branches, PRs, or commits.
**After**: signals/01 (block taxonomy) is the active item. A worktree is created and work begins on the block type enum, data model, and API endpoints.
**Rationale**: Phase 1 explicitly requires signals/01 parallel with chord-model/02. It hasn't started. agent-embedding/01 is already building a block queue view that will consume these types. Every day signals stays dormant, agent-embedding builds further around placeholder types that will need rework. The dependency chain is: signals/01 → agent-embedding/01 → functional block queue. Starting now keeps that chain honest.
**Risk**: Area overlap with chord-model in `rust/loopflow/src/lfd/` and `python/loopflow/`. Merge conflicts are possible if both waves touch the same files. Mitigation: signals/01's primary deliverable (block enum, block API) is additive — new files and new routes, not modifications to existing engine code. The overlap is manageable.

### 2. Silence clear-the-deck

**Wave**: clear-the-deck
**Lever**: silence
**Before**: Four items queued, zero activity. Wave watches `lfd/auth.rs`, `lfd/provider_auth.rs`, `lfd/executor/sandbox.rs`, `lfd/http/`, `python/loopflow/`.
**After**: All items remain defined but explicitly deferred. Wave stays in manual mode with no active items until chord-model/02 and signals/01 land.
**Rationale**: clear-the-deck's area overlaps heavily with chord-model and signals. Starting auth consolidation or deployment collapse while those waves are actively changing `lfd/` and `python/loopflow/` creates unnecessary merge friction. The four items are independent cuts — they can run fast once the shared area settles. Silencing now shrinks the blocking queue: two active waves instead of three competing for review in overlapping files.
**Risk**: Delay. But clear-the-deck isn't Phase 1 work and none of its items are dependencies for other waves. The cost of waiting is low.

## Coherence

These two mutations reinforce each other. Waking signals while silencing clear-the-deck keeps exactly two waves building in Phase 1 (chord-model + signals), with agent-embedding continuing its isolated Swift work. Three active waves, one silent — the right ratio for a chord in bootstrap.

The dependency ordering is clean:
- chord-model/02 (live proof) → enables real tend cycles
- signals/01 (block taxonomy) → unblocks agent-embedding/01's typed content
- agent-embedding/01 (block queue) → continues independently in Swift
- clear-the-deck → waits for shared area to settle

No mutation conflicts. No new ordering constraints introduced.

## Execution notes

**PR #565** (chord-model) has all CI green and should land. Once on main, chord-model/02's remaining work is operational: boot lfd, run `scripts/bootstrap-redesign.py`, exercise `lf tend` against live state. This isn't a mutation — the wave's config and direction are correct. It's execution.

**PR #563** (agent-embedding) title mentions "pm wave, auth improvements" alongside the core attention queue work. Worth verifying on review that it stayed within `swift/` and didn't touch shared areas. Not a mutation — an observation for the review step.

## Deferred

- **chord-model direction change**: Considered adding `simplicity` to chord-model's direction for the live proof phase (operational execution, not design). Deferred — the current `[clarity, care]` direction is appropriate for proving that the tend cycle works correctly. Simplicity can be added if the live proof reveals over-engineering.

- **agent-embedding items reorder**: signals/01 dependency means agent-embedding/01 will eventually need typed blocks. But the block queue UI scaffolding (layout, navigation, status tracking) is valuable independent work that doesn't need the taxonomy yet. No item reorder needed — the dependency resolves naturally when signals/01 delivers.

- **Trigger wiring**: None of the member waves have triggers configured beyond defaults. Adding `signal: wave` triggers (e.g., signals completion triggers agent-embedding) would formalize the dependency chain. Deferred until chord-model/04 (chord-wave triggers) — that's where trigger design belongs.
