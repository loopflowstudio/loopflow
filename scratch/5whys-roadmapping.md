# 5 Whys: Roadmapping Process

## Symptom

The harness roadmap had us building contracts and persistence before a working agent. Two items in, we discovered the sequence was wrong and the abstractions were premature.

## Why 1: Why did we build persistence before the agent?

The roadmap said 02 (persistence) comes before 03 (agent skeleton). `ingest` picked 02 because it was the lowest-numbered incomplete item.

## Why 2: Why did the roadmap sequence it that way?

The roadmap was structured as a dependency graph: "what needs to exist for the code to compile." Dependencies flow down, not "what do we need to learn first." The `roadmap.md` step that produced the roadmap doesn't have guidance on sequencing for learning — it just says "identify the highest-leverage proposal."

## Why 3: Why doesn't the roadmap step guide sequencing?

Because `roadmap.md` is designed to produce a single proposal (`scratch/roadmap-proposal.md`), not a multi-item sequenced roadmap. When you want a full roadmap, you use `lf design` and tell it to write one — which means roadmap-specific guidance (sequencing, uncertainty, checkpoints) doesn't exist anywhere.

## Why 4: Why isn't there a step for revising roadmaps after shipping an item?

The `ship-roadmap` flow is `start → ship` (ingest → kickoff → implement → compress → gate → consolidate). After shipping, nothing says "revisit the plan." There's no feedback loop. The roadmap is written once and consumed linearly until empty.

## Why 5: Why is the roadmap treated as a static artifact?

Because the tooling assumes roadmaps are correct when written. `ingest` picks items, `kickoff` elaborates them, `implement` builds them. No step asks "given what we just learned, should the remaining items change?"

## Root causes

1. **`roadmap.md` is a proposal step, not a planning step.** It produces one item. Multi-item roadmaps are created ad hoc via `lf design` with no roadmapping guidance.
2. **No update-roadmap step.** The flow has no feedback loop. Ship → pick next → ship → pick next, with no "pause and reassess."
3. **No guidance on sequencing for learning.** Nothing in the tooling says "put the most uncertain thing first" or "build outward from working code."

## Proposed changes

### 1. Enhance `roadmap.md` step

The existing step produces a single proposal. Enhance it to also handle multi-item roadmap creation with guidance on:

- **Build outward.** Start with the smallest working thing. Expand from there.
- **Sequence by learning.** What are you most uncertain about? That goes first. What will you know after each phase that you don't know now?
- **Encode uncertainty.** Each phase has open questions. The roadmap expects to be revised.
- **Checkpoints.** After each phase, explicitly state what you expect to learn and what might change.

### 2. New `update-roadmap.md` step (ops/)

After shipping a roadmap item, read the diff, read the remaining roadmap, and write an updated roadmap.

- What did we learn from building this?
- Do the remaining items still make sense?
- Has the sequence changed?
- Are there new open questions?
- Should anything be added, removed, or reordered?

### 3. Update `ship-roadmap.yaml` flow

Current: `start → ship`

Proposed: `start → ship → update-roadmap`

After each item ships, the roadmap gets revisited. The loop has a feedback mechanism.

### 4. (Optional) Update `grind.yaml` similarly

Current: `review → iterate → ship → gate`

Could add: `review → iterate → ship → gate → update-roadmap`

For iterative loops, the roadmap also gets updated.
