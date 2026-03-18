# Chord Model

## Vision

Make chord-waves first-class without introducing a second runtime model. A chord-wave is an ordinary wave whose `area` points at `wave/<name>/` directories, so coordination, mutation, and memory all grow out of the existing wave/flow system instead of parallel chord CRUD.

This wave builds the machinery the redesign chord-wave will use to tend its own member waves. Build remains the code-producing voice. Tend becomes the coordinating voice. The system only works if those two voices can share one data model.

## Strategy

### Start from the waves-only baseline

Bootstrap is already the ground truth: the redesign chord-wave registers through the normal wave API, membership lives in `wave/redesign/redesign.yaml`, and the redesign waves start in ordinary wave directories. Everything left in this wave builds on that baseline. No separate chord tables, DTOs, or client helpers come back.

### Keep chord cadence inside ordinary wave primitives

The trigger/cadence foundation is now in place:
- the redesign chord-wave runs on `mode: cron` with daily heartbeat plus `wave` and `block` triggers
- member waves run in `mode: managed`, so the chord owns their rhythm
- sourceless `wave` and `block` triggers derive membership from `area: [wave/<name>/]`
- merged-PR webhooks and persistent queue blocks wake the chord through the existing trigger runtime

Future work should treat that as fixed infrastructure, not re-solve scheduling.

### Turn tend into a real flow

The next milestone is a full tend cycle:

```yaml
- scan-waves
- assess
- branch:
    chord: tend-chord   # draft-chord -> review-chord -> apply-chord
    reorg: reorg
```

Those steps need a concrete contract for what a chord-wave can see about its member waves, how it detects drift, and how it turns judgment into durable mutations. The flow should tolerate imperfect state, fix what it finds, and stay legible enough that humans can calibrate it.

### Keep the architecture waves all the way down

Membership is `area`. Runtime state lives in lfd. Filesystem identity lives in `wave/<name>/`. Letta, triggers, mutation APIs, and eventual DAG support all need to extend that model rather than route around it. The current trigger runtime already treats repo-local `wave/<name>/` area entries as membership; the remaining work is to make that contract richer and more observable, not to invent a side channel.

### Build for human calibration, not hidden autonomy

Tend should surface trajectory, conflicts, and shallow work clearly enough that a human can intervene at the right moments. Mechanical fixes can auto-apply. Judgment-heavy changes need explicit calibration hooks, clear rationale, and reversible mutations.

### Use VSM as pressure, not ceremony

The tend steps should naturally ask coordination, optimization, intelligence, identity, and algedonic questions. That pressure belongs in the flow and prompts, not in a second governance framework bolted onto the side. Explicit VSM chord structures should remain representable as nested chord-waves if users want them.

## Goals

- The redesign chord-wave can run `tend` against its own member waves through ordinary wave configs and APIs
- Tend steps produce structured observations, actionable proposals, and durable applied changes
- Letta gives the redesign chord-wave persistent qualitative memory across tend cycles
- Wave mutation stays waves-only: direction, area, flow, triggers, work items, and lifecycle all mutate through one model
- Nested chord-waves remain possible without reintroducing chord-specific runtime concepts

## Risks

- Tend could become a report generator instead of a coordinating loop that changes outcomes
- Letta integration could sprawl beyond a thin memory boundary
- Mutation APIs could become too implicit and make human calibration harder instead of easier
- `area` membership still depends on exact `wave/<name>/` entries; silent typos or missing members could make the chord misread its own shape
- DAG and trigger work could leak chord-specific special cases back into the runtime

## Metrics

- First successful tend cycle against `redesign`: 1
- Tend cycles that surface at least one actionable observation: >50%
- Time from block detection to human awareness during working hours: <1 hour
- Mutation proposals accepted by a human reviewer: tracked per cycle, trending upward
- Human calibration sessions that produce a course correction or explicit affirmation: >0 each week while redesign is active
