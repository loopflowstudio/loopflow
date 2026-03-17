# Chord Model

Make chords think. The CRUD exists — this wave builds the behavior layer. Tend flow, Letta memory, wave mutation, the things that make a chord more than a grouping mechanism.

This wave is recursive: it builds the tools that the redesign chord will use to coordinate all four waves, including this one. Early items ship via existing `build` flow. Later items create `tend`. Then the chord starts using what it built.

## Strategy

Bootstrap first. Get the redesign chord running tend cycles against its own waves as fast as possible. Every item after that is informed by what tend reveals.

The tend flow is the counterpoint to build. Build creates (code, tests, PRs). Tend maintains (scan, assess, propose, apply). Same flow engine, different area — files vs waves.

Human intervention points in each flow:

**Build flow** — two checkpoints, spaced between agent work:
- Design review (forward-looking): is this the right thing to build?
- Code review (backward-looking): is what we built good enough?

**Tend flow** — calibration (meta, cross-cutting):
- Are we making real progress toward what matters?
- Are we lost in details that don't matter, or skipping details that do?
- Do agents have tools to evaluate they're creating polished experiences?
- Is the human still connected to what's being produced, or drifting?

## Goals

- Tend flow runs against the redesign chord's own waves
- Letta provides persistent memory across tend cycles
- Chord can mutate wave configuration (direction, area, flow, agent, work items)
- Human calibration moments surface trajectory, not just status
- Default chord exists as concept, ready to absorb existing waves after proven

## Risks

- Letta integration could be heavier than "thin wrapper" — watch for scope creep
- Tend flow could become ceremony if it doesn't surface genuinely useful observations
- Recursive bootstrapping means early tend cycles run on incomplete machinery

## Metrics

- Number of tend cycles that surface an actionable observation (target: >50%)
- Time from block detection to human awareness (target: <1 hour during working hours)
- Number of wave mutations proposed by chord that human accepts (signal of useful judgment)
- Human-system drift: days since human engaged substantively with a wave's output
