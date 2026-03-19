---
asana_id: '1213718558900002'
linear_id: d2421075-cad0-4b3d-8cd7-ad89631b987d
---
# 06: Wave Mutation

**Finish line:** The chord-wave's `wave/mutate` step can modify wave configuration through a structured mutation API. Direction, area, flow, agent, work items, triggers, and lifecycle are all mutable. Mutations are logged and reversible.

## Context

`wave/mutate` composes and executes changes in one pass. After bootstrap, the constraint is sharper: mutations have to operate on the waves-only model that replaced chord CRUD. Changes should update ordinary wave config and runtime state, not smuggle chord-specific state back into the system.

The manual execution path exists now, and `wave/mutate` edits wave YAML and item files directly while the ordinary waves API / `update_wave` path can already sync runtime fields like `flow`, `direction`, `area`, `status`, `agent`, and `step_agents` back into lfd. What's missing is one typed mutation layer that spans those levers, logs each change, and preserves enough prior state to revert it cleanly.

The levers stay the same:
- **Direction** — shift what a wave optimizes for
- **Area** — tighten or widen scope
- **Flow** — change the process
- **Work items** — rewrite, reprioritize, add, or delete work
- **Agent / step agents** — change model choice where it matters
- **Triggers** — adjust cadence or sources
- **Lifecycle** — pause, resume, split, combine, or prune

The folded `signals` work changes the pressure behind these levers. Stall detection, repeated algedonic incidents, and recurring calibration notes do not need their own wave anymore; they should culminate in ordinary mutations here.

## What to build

1. **Mutation API.** A wave-scoped mutation endpoint or loader accepts a list of structured mutations and applies them to ordinary waves.

2. **Mutation log.** Record who requested the mutation (chord-wave or human), why, when, what changed, and the previous value.

3. **Reversibility.** Store enough prior state to revert an individual mutation cleanly.

4. **Mutate-step integration.** `wave/mutate` uses the mutation API for both automatic governance mutations and human-amended follow-up changes. Review artifacts keep enough mutation context attached to amend or revert cleanly.

5. **Pressure-to-mutation mapping.** Capture how common chord signals turn into mutation candidates:
   - repeated algedonic incidents → flow or scope change
   - stall pattern → work-item rewrite or lifecycle pause
   - coordination conflicts → resequence, split, or combine

## Done when

- The mutation API accepts and executes all planned lever types
- Every mutation is logged with actor, rationale, and previous value
- Mutations are individually revertible
- `wave/mutate` uses the API for both automatic and human-amended changes
- At least one real mutation has been applied by the redesign chord-wave's garden flow
- Repeated stall or algedonic pressure can be expressed as structured mutation proposals without a separate signal subsystem
