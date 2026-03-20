# 02c: VSM Chord Configs

**Finish line:** The redesign chord has five member waves (s5-policy, s4-intelligence, s3-control, s2-coordination, s1-operations), each with its own flow and rhythm. The sequential `vsm.yaml` flow is removed.

## Context

Item 02 creates the governance flows (govern-identity, govern-intelligence, govern-control, govern-coordination) and their scan/assess steps. This item creates the actual wave configs that run those flows, and wires s1 as a worker pool.

Depends on:
- ~~02 (governance flows and steps exist)~~ — shipped
- 02a (worker pools — s1 needs `workers: N`)
- 02b (wave modes — s1 needs `mode: loop`, governance waves need `mode: cron`)

Some scan prompts depend on tools or external signals (`cargo audit`, `lfq show`, `lfq usage`) that may be unavailable in a given runtime. Governance waves need graceful skip behavior when their scan can't reach an expected data source.

Open design questions from the governance flow build:
- **`garden/scan` consuming sN outputs:** Does garden/scan pull (run the sN scans inline) or read (assume governance flows ran recently and consume their outputs)? Affects whether garden and governance flows are coupled or independent.
- **Interactive flag on `review-chord`:** Is interactivity baked into the step or inherited from flow context? Matters because governance flows are headless (no human) while garden is interactive.

## The shape

```
wave/redesign/
  redesign.yaml
  wave/s5-policy/
    s5-policy.yaml           # cron: weekly, flow: govern-identity
    README.md
  wave/s4-intelligence/
    s4-intelligence.yaml     # cron: daily, flow: govern-intelligence
    README.md
  wave/s3-control/
    s3-control.yaml          # cron: every 4h, flow: govern-control
    README.md
  wave/s2-coordination/
    s2-coordination.yaml     # cron: every 4h, flow: govern-coordination
    README.md
  wave/s1-operations/
    s1-operations.yaml       # mode: loop, workers: N, flow: build-or-silent
    README.md
    01-first-item.md         # backlog items maintained by s2
```

### Governance waves (s5–s2)

Independent rhythms:

- **s5-policy** — `mode: cron`, weekly or slower. Identity doesn't shift fast.
- **s4-intelligence** — `mode: cron`, daily. Environment changes constantly.
- **s3-control** — `mode: cron`, every 4 hours. Tight control loop.
- **s2-coordination** — `mode: cron`, every 4 hours. Deconfliction before workers grab items.

Each runs on its own clock and works with whatever's current. Not trigger-chained — each reads the latest output from other levels when it runs.

All four only edit wave space — plans, backlogs, configs, `workers` on s1. No code changes.

### s1-operations (worker pool)

`mode: loop`, `workers: N`, `flow: build-or-silent`. Workers pull from the backlog (maintained by s2), each in its own worktree. Ephemeral — worktree pruned after landing.

s3 adjusts `workers` on s1 via `wave/mutate` mutations.

### Chord config update

`redesign.yaml` area updated to include the five member wave directories:
```yaml
area:
  - wave/redesign/wave/s5-policy/
  - wave/redesign/wave/s4-intelligence/
  - wave/redesign/wave/s3-control/
  - wave/redesign/wave/s2-coordination/
  - wave/redesign/wave/s1-operations/
```

## Done when

- Five member wave directories exist with configs and READMEs
- s5–s2 run their governance flows on independent cron schedules
- s1 runs `build-or-silent` with `workers: N`
- Redesign chord area includes all five member waves
- Sequential `vsm.yaml` is removed (already done in 02)
- Garden flow still works for human check-ins on the same chord
