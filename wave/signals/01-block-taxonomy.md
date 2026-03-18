---
asana_id: '1213717741058350'
linear_id: a170df59-33ca-48f1-9f7d-6485553629d8
---
# 01: Block Taxonomy

**Finish line:** Block types defined by what actually blocks work, not speculation. Each type has: detection criteria, self-healing options, escalation path. The taxonomy is a living document — new types added as they're discovered.

## Context

The system currently handles three signals: repo (paths changed), wave (wave completed), ci_failure. The redesign reframes these as blocks — the default state is running, blocks are what interrupts.

Don't design the full taxonomy upfront. Start with the types that will actually occur during this redesign, build detection for those, and extend as new block types emerge.

This item can run in parallel with chord-model/02's live tend-cycle validation. It does not depend on Letta or the trigger work, and the block queue UI is waiting on at least the first concrete block types to exist.

Keep the first slice additive: a `BlockType` enum, the core `Block` model, and the minimal API surface the queue needs. That keeps overlap with chord-model manageable while still unblocking agent-embedding with real types instead of placeholders.

## What to build

### Initial block types

**ci_failure** — CI failed on a wave's PR. Already exists as a signal. Promote to a block with self-healing (ci-fix flow) and escalation (to chord-wave, then human).

**merge_conflict** — Wave's branch can't merge cleanly. Self-healing: rebase flow. Escalation: chord-wave resequences waves, or human resolves manually.

**stall** — Wave hasn't produced meaningful output in N hours while nominally running. No self-healing — this is a judgment call. Escalates to chord-wave assess, then human.

**shallow_work** — Wave producing PRs but quality is thin. Small diffs, no tests, mechanical changes when the work item called for depth. Detected by tend flow's assess step. Escalates to human calibration.

**file_conflict** — Two waves modifying the same files. Not a merge conflict (yet) — a coordination concern. Detected by chord-wave scan. Chord-wave proposes resequencing.

**human_drift** — Human approvals getting faster, reviews getting shorter, no course corrections in N cycles. The human is disengaging. Detected by tend flow. Surfaces as a calibration block.

**capability_gap** — Wave shipping code without validation that it works for users. No integration tests, no screenshots, no end-to-end runs. Detected by tend assess. Escalates to human with specific recommendation.

### Block data model

```rust
struct Block {
    id: BlockId,
    block_type: BlockType,
    wave_id: WaveId,
    owner_wave_id: Option<WaveId>,  // present when a chord-wave owns escalation
    detected_at: DateTime,
    detected_by: BlockSource,  // wave-self, chord-wave-tend, human
    status: BlockStatus,       // detected, self-healing, escalated, resolved
    self_heal_attempts: Vec<HealAttempt>,
    resolution: Option<Resolution>,
}
```

### API

- `POST /v0/blocks` — create a block (internal, from wave or chord-wave)
- `GET /v0/blocks?status=escalated` — list blocks for human (block queue consumes this)
- `PUT /v0/blocks/{id}/resolve` — resolve with outcome
- `GET /v0/waves/{id}/blocks` — blocks for a specific wave
- `GET /v0/waves/{id}/blocks?scope=area` — blocks across a chord-wave's member waves

## Done when

- Block types exist as an enum with detection criteria documented
- Block API supports create, list, resolve
- Block queue view (agent-embedding/01) can consume the API
- At least ci_failure and stall are detectable automatically
