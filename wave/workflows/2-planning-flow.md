---
asana_id: '1213879706560842'
notion_id: 333f8f99-3d81-819e-81f0-e81723db4621
---
# Planning flow and wave governance

**Finish line:** A garden wave runs a single planning flow that traverses its member tree — scanning up (leaves → root), governing down (root → leaves) — producing one reviewable PR per planning cycle. The s-levels (s5–s2) are steps within the flow, not separate waves.

## Context

The original design had five member waves per coordinator (s5-policy, s4-intelligence, s3-control, s2-coordination, s1-operations), each with independent cron schedules. That model was replaced for three reasons:

- **Planning levels are not independent.** s2 needs s3's capacity decision; s3 needs s4's environmental read. Independent crons mean each level works with stale output from the others.
- **PM noise.** Each wave needs a project, backlog, README, and status loop. Five waves per coordinator is overhead for what is really one planning process.
- **No natural parallelism.** Planning is inherently serial. `workers > 1` is meaningless for governance.

The builtin steps (`s5-scan`, `s5-assess`, ..., `s2-scan`, `s2-assess`) and governance flows (`govern-identity`, `govern-intelligence`, `govern-control`, `govern-coordination`) already shipped. What is missing is the recursive traversal that composes them into a single planning pass.

Depends on:
- ~~worker pools~~ — shipped
- ~~wave cron support~~ — shipped; planning uses `crons:` entries on the garden wave

## The planning flow model

A garden wave has two rhythms:

1. **Planning beat** (periodic): scan up → govern down → one PR
2. **Worker batch** (triggered by planning): N workers fire against the queue, each producing its own PR

The planning pass is itself `workers: 1` — always serial. The parallelism lives in the work phase.

### Up pass (scan)

Leaves → root. Each garden wave scans its member state. Parents see children's scan output as input. Pure information, no side effects.

### Down pass (govern)

Root → leaves. Each garden wave governs based on the full scan picture. Children inherit parent policy and constraints. The pass writes mutations — queue ordering, capacity, policy.

### Cadence

The same up/down flow runs at every cadence. Levels with nothing to say pass through as no-ops:

- Every 4h: s3/s2 mostly — queue reordering, capacity adjustment
- Daily: s4 joins — dependency drift, upstream changes
- Weekly: s5 weighs in — policy shifts, area reorg

No need to configure separate cadences per s-level. One cron, all levels run, cheap levels stay cheap.

## Open design questions

- **Flow engine traversal primitive.** The flow engine already handles recursive expansion, xor branching, loop constructs, and parent-flow plumbing. What's missing is a primitive for “run these steps at each node in tree order.” Options: a new flow-level `traverse` construct, or lfd manages the tree walk externally and invokes standard flows at each node.
- **Capacity writes.** How does s3's capacity allocation get written? Mutation to child wave configs via `mutate`? A separate capacity file?
- **Batch vs pool initiation.** Autonomous work wants batch (plan → execute → plan). Interactive/garden wants pool (workers always running, human feeds queue). Same `workers: N` underneath, different initiation pattern.
- **Scan tool availability.** Some scan prompts depend on tools or external signals (`cargo audit`, `lfq show`, `lfq usage`) that may be unavailable in a given runtime. Planning needs graceful skip behavior when a scan cannot reach an expected data source.

## Done when

- A garden wave can run a planning flow that traverses its member tree
- Scan runs leaves → root, govern runs root → leaves
- The s-levels compose as steps within each scan/govern pass
- One PR per planning cycle captures all governance decisions
- After the planning PR lands, workers fire across all waves with capacity
