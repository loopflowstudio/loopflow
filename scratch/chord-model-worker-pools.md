# Worker Pools & Planning Rhythm

Design notes from conversation. Reshapes 02a (worker pools) and 02c (VSM chord configs).

## The model

Waves are the only primitive. A wave can have children, workers, a backlog — all optional. Chords emerge from composition, not from a distinct type.

### Workers as capacity budget

`workers: N` is a capacity cap on a wave. Intermediate waves in a chord tree split their budget among children and themselves:

```
root: 10 workers total
├── api: 6          (3 auth + 2 billing + 1 self)
│   ├── api-auth: 3
│   └── api-billing: 2
├── frontend: 3
└── docs: 1
```

Workers is a policy decision (s5-level, slow-changing), not recomputed each cycle. s3 can adjust it, but it's a stable cap, not a per-batch calculation.

Intermediate nodes do work too — api keeps 1 worker for cross-cutting work that doesn't belong to either child.

### Areas

A chord's area is a superset of its children's areas. Children *can* overlap — that's what s2 coordination handles. The hard constraint: children stay within the parent's area.

```
api:           area: src/api/
├── api-auth:    area: src/api/auth/, src/api/middleware/
└── api-billing: area: src/api/billing/, src/api/middleware/
# overlap on middleware is fine — s2 deconflicts at runtime
```

### The planning flow

The planning pass is an up/down traversal of the chord tree. It's a regular flow — any wave with children can run it.

```
up (leaves → root):  each chord scans
                     parent sees children's scan output as input
                     pure information, no side effects

down (root → leaves): each chord governs
                      children inherit parent's policy/constraints
                      writes mutations — queue ordering, capacity, policy

commit:              one PR for the entire planning pass
```

Scan is each system's afferent channel (information flowing inward/upward). Govern is the efferent channel (decisions flowing outward/downward). Within each chord's scan or govern, the s-levels (s5, s4, s3, s2) run as steps — they're internal to the chord, not separate waves.

After the planning PR lands, workers fire across all waves with capacity.

### Why not separate waves for s5–s2?

The earlier design (02c) had five member waves per chord with independent cron schedules. Problems:

- **Planning levels aren't independent.** s2 needs s3's capacity decision, s3 needs s4's environmental read. Independent crons mean each level works with stale output from the others.
- **PM noise.** Each wave needs a Linear project, backlog, README. Five waves per chord is overhead for what's really one planning process.
- **No natural parallelism.** Planning is inherently serial — you never want two s4 scans racing. `workers > 1` is meaningless for governance.

Instead: s5–s2 are steps within the planning flow. One wave, one PR, one reviewable decision.

### Why not s5 creating/destroying waves for parallelism?

Ephemeral waves for batch parallelism have a global tick problem — you have to wait for the full planning cycle to create/destroy them. `workers: N` is immediate.

Also messes with PM — you want stable, named waves that humans track over time.

Wave splitting is a structural decision (s5, slow). Workers are operational capacity (s3, fast). Different timescales.

### Two rhythms

A chord has two rhythms:

1. **Planning beat** (periodic): scan up → govern down → one PR
2. **Worker batch** (triggered by planning): N workers fire against the queue, each producing its own PR

The planning pass is itself `workers: 1` — always serial. The parallelism lives in the work phase.

### Cadence is uniform, levels self-gate

The same up/down flow runs at every cadence. Levels with nothing to say pass through as no-ops:

- Every 4h: s3/s2 mostly — queue reordering, capacity adjustment
- Daily: s4 joins — CVEs, dependency updates
- Weekly: s5 weighs in — policy shifts, area reorg

No need to configure "s5 weekly, s4 daily" separately. One cron, all levels run, cheap levels are cheap.

### The root wave is not special

The root is just the wave at the top of the tree. It runs the planning flow because that's its flow. Any wave could run it if it had children. There's no "root wave" type.

## What this changes

### 02a (worker pools) — narrower but same mechanics

The `workers: N` implementation stays the same — replace `serialized: bool` with a capacity cap, dispatch counts active runs. But the framing changes:

- Workers is a **budget that flows down the chord tree**, not just a per-wave config
- Default `workers: 1` — unchanged
- Primary consumer is leaf waves doing actual work, not governance

### 02c (VSM chord configs) — replaced

Five independent governance waves with separate crons → one wave running a planning flow that traverses the chord tree. The s-levels are steps, not waves.

### Governance flows — restructured

The four `govern-*` flows (govern-identity, govern-intelligence, govern-control, govern-coordination) become scan/govern steps within the planning flow, not standalone flows on separate waves.

Current steps (s5-scan, s5-assess, s4-scan, etc.) still useful — they're the building blocks. But they compose into a single planning flow, not four independent flows.

## Open questions

- How does the flow engine express the recursive traversal? Is it built into the planning flow, or does lfd manage the tree walk?
- How does s3's capacity allocation get written? Mutation to child wave configs? A separate capacity file?
- Does the planning flow need a new primitive for "run these steps at each node in tree order"?
- Batch model vs pool model: autonomous work wants batch (plan → execute → plan). Interactive/garden wants pool (workers always running, human feeds queue). Same `workers: N` underneath, different initiation pattern. Is this just mode (cron vs loop)?
