# Signals

The nervous system. Default state is running. When something blocks progress, the system tries to unblock itself first. What it can't resolve propagates up — wave to chord-wave to human.

## Strategy

Start with the block types that actually occur during this redesign. Don't design a taxonomy speculatively — let real blocks define the categories. CI failure and merge conflict exist already. Stall detection and shallow work detection are the high-value additions.

The self-healing cascade (wave tries -> chord-wave tries -> human decides) is the core architecture. Build it for one block type, then extend.

This wave is the parallel Phase 1 track next to `chord-model/02`, not a later cleanup pass. Start with additive block types and APIs so the work can move now without fighting over the same `lfd/` files that chord-model is still proving live.

## The cascade

```
Block occurs (CI failure, merge conflict, quality gate, stall)
  -> Can the wave unblock itself? (ci-fix, rebase, retry)
    -> yes: keep running, log it
    -> no: block propagates to chord-wave
      -> Can the chord-wave unblock it? (resequence, pause conflicting wave)
        -> yes: keep running, log it
        -> no: block surfaces to human in Concerto
```

The Concerto UX is fundamentally a queue of blocks — "here's what's stuck and what you need to decide." Not a notification feed. A machine waiting for you.

## Qualitative signals

Beyond mechanical blocks (CI, merge conflicts), chord-waves detect qualitative signals:

- **Shallow work** — PRs landing but quality is thin relative to intent
- **Stall** — wave running but not producing meaningful progress
- **Capability gap** — wave shipping code without validating user experience (no integration tests, no screenshots, no end-to-end)
- **Human-system drift** — approvals getting mechanical, no course corrections, the human losing the thread of what's being produced

These surface at calibration, not as interrupts.

## Goals

- Block types defined by what actually blocks work, not speculation
- Self-healing cascade: wave -> chord-wave -> human, with each level trying before escalating
- Stall detection: wave running but not producing
- Shallow work detection: PRs landing but quality is thin
- Human-system drift detection: human approvals getting mechanical
- Tool/capability gap detection: wave shipping code without validating user experience

## Risks

- False positives in stall/shallow detection could erode trust in the system
- Defining "shallow work" is inherently subjective — needs human calibration
- Self-healing could mask problems that should surface earlier

## Metrics

- False positive rate on stall detection (target: <20%)
- Time from block occurrence to resolution (self-healed or human-resolved)
- Percentage of blocks self-healed vs escalated (healthy ratio TBD empirically)
