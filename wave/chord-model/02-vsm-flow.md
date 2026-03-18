# 02: VSM Flow

**Finish line:** `lf vsm` runs a single-pass viable system audit — s5 through s2 — against a chord-wave's members. Each step is a builtin. The flow produces code changes and ships in one PR.

## Context

The VSM flow is the default chord flow. It replaces `tend` as the general-purpose way a chord assesses and acts on its members. `tend` remains as an interactive variant.

Prerequisite: algedonic signals (01) give the control step (s3) concrete operational data — which waves are healthy, which are struggling, what repair attempts have been tried.

**Before starting:** re-evaluate the 01 live demo. The repair dispatch code (`execute_run_inner` → `create_repair_run` → algedonic escalation) is built and unit-tested but hasn't run end-to-end in lfd. Infra gaps (dev lfd token isolation, PR state sync for `check-ci` polling) blocked the demo. Try again — the gaps may have been fixed by other work, or may be quick to close now.

## Steps

### s5 — Identity and Policy

- Is this chord still responsible for the right things?
- Does the member roster match the chord's purpose?
- Are autonomy levels appropriate given recent algedonic history?
- Should any waves be created, archived, merged, or split?
- Has the direction drifted from intent?

### s4 — Intelligence

- What changed in the environment since last cycle?
- Are there upstream changes that affect member waves?
- New dependencies, deprecations, API changes?
- What's coming that members should prepare for?

### s3 — Control

- Are members performing? Run status, velocity, error rates.
- Algedonic history — which waves needed repair? How often?
- Where to allocate attention? What's blocked, stalled, or idle?
- Resource allocation — should any wave get more/fewer slots?

### s2 — Coordination

- Are any members working on overlapping areas?
- Are there conflicts between member PRs?
- Is work oscillating (one wave undoing another's changes)?
- Should triggers or dependencies between members change?

S1 is absent — it's the member waves running their own flows.

## Flow definition

```yaml
# flow: vsm
- s5
- s4
- s3
- s2
```

Each step reads scratch/ from previous steps. Each can write code. The flow produces a single PR with all changes.

## Done when

- Four builtin steps exist (s5, s4, s3, s2)
- `lf vsm` runs them in order against a chord-wave
- Each step has access to member wave state and algedonic history
- The flow produces actionable changes, not just reports
