# Planning Flow and Chord Governance

**Finish line:** A chord-wave runs a single planning flow that traverses its member tree — scanning up (leaves → root), governing down (root → leaves) — producing one reviewable PR per planning cycle. The s-levels (s5–s2) are steps within the flow, not separate waves.

## Context

The original design had five member waves per chord (s5-policy, s4-intelligence, s3-control, s2-coordination, s1-operations), each with independent cron schedules. That model was replaced during the worker-pools design conversation for three reasons:

- **Planning levels aren't independent.** s2 needs s3's capacity decision, s3 needs s4's environmental read. Independent crons mean each level works with stale output from the others.
- **PM noise.** Each wave needs a Linear project, backlog, README. Five waves per chord is overhead for what's really one planning process.
- **No natural parallelism.** Planning is inherently serial — you never want two s4 scans racing. `workers > 1` is meaningless for governance.

The builtin steps (`vsm/s5-scan`, `vsm/s5-assess`, ..., `vsm/s2-scan`, `vsm/s2-assess`) and governance flows (`govern-identity`, `govern-intelligence`, `govern-control`, `govern-coordination`) already shipped. What's missing is the recursive traversal that composes them into a single chord-tree planning pass.

Depends on:
- ~~02a (worker pools)~~ — shipped
- 02b (wave modes — planning flow needs `mode: cron`)

## The planning flow model

A chord has two rhythms:

1. **Planning beat** (periodic): scan up → govern down → one PR
2. **Worker batch** (triggered by planning): N workers fire against the queue, each producing its own PR

The planning pass is itself `workers: 1` — always serial. The parallelism lives in the work phase.

### Up pass (scan)

Leaves → root. Each chord scans its member state. Parent sees children's scan output as input. Pure information, no side effects. This is each system's afferent channel.

### Down pass (govern)

Root → leaves. Each chord governs based on the full scan picture. Children inherit parent's policy/constraints. Writes mutations — queue ordering, capacity, policy. This is the efferent channel.

### Cadence

The same up/down flow runs at every cadence. Levels with nothing to say pass through as no-ops:

- Every 4h: s3/s2 mostly — queue reordering, capacity adjustment
- Daily: s4 joins — CVEs, dependency updates
- Weekly: s5 weighs in — policy shifts, area reorg

No need to configure "s5 weekly, s4 daily" separately. One cron, all levels run, cheap levels are cheap.

## Open design questions

- **Flow engine traversal primitive.** The flow engine now handles recursive expansion (`expand_with_chain`), AND/XOR branching, loop constructs, and parent-flow plumbing. What's missing is a primitive for "run these steps at each node in tree order" — the chord-tree up/down traversal. Options: a new flow-level `traverse` construct, or lfd manages the tree walk externally and invokes standard flows at each node.
- **Capacity writes.** How does s3's capacity allocation get written? Mutation to child wave configs via `wave/mutate`? A separate capacity file?
- **Batch vs pool initiation.** Autonomous work wants batch (plan → execute → plan). Interactive/garden wants pool (workers always running, human feeds queue). Same `workers: N` underneath, different initiation pattern. Is this just `mode` (cron vs loop)?
- **Scan tool availability.** Some scan prompts depend on tools or external signals (`cargo audit`, `lfq show`, `lfq usage`) that may be unavailable in a given runtime. Planning flow needs graceful skip behavior when a scan can't reach an expected data source.

## Done when

- A chord-wave can run a planning flow that traverses its member tree
- Scan runs leaves → root, govern runs root → leaves
- The s-levels compose as steps within each chord's scan/govern pass
- One PR per planning cycle captures all governance decisions
- After the planning PR lands, workers fire across all waves with capacity
