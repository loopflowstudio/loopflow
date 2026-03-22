---
asana_id: '1213718558900002'
linear_id: 98e766f3-8785-4bf2-9af1-8c23a93fad8e
notion_id: 32af8f99-3d81-81bd-ab89-f9303b2a1563
---
# Wave Mutation

**Finish line:** The chord-wave's `apply` step can modify wave configuration through a structured mutation API. Direction, area, flow, agent, work items, triggers, and lifecycle are all mutable. Mutations are logged and reversible.

## Context

`wave/mutate` now exists as a shipped builtin step that composes and executes changes in one pass. It edits wave YAML and item files directly, while the waves API / `update_wave` path syncs runtime fields (`flow`, `direction`, `area`, `status`, `agent`, `step_agents`) back into lfd. The governance flows (`govern-*`) each terminate with `wave/mutate`.

What's missing is one typed mutation layer that spans all levers, logs each change, and preserves enough prior state to revert cleanly. Currently `wave/mutate` operates through direct file edits without structured logging or reversibility.

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

4. **Apply-step integration.** `apply-chord` uses the mutation API for both auto-applied changes and human-gated proposals. Human-review items keep the proposed mutation attached so approval can execute the same payload.

5. **Pressure-to-mutation mapping.** Capture how common chord signals turn into mutation candidates:
   - repeated algedonic incidents → flow or scope change
   - stall pattern → work-item rewrite or lifecycle pause
   - coordination conflicts → resequence, split, or combine

## Done when

- The mutation API accepts and executes all planned lever types
- Every mutation is logged with actor, rationale, and previous value
- Mutations are individually revertible
- `apply-chord` uses the API for both auto and human-gated changes
- At least one real mutation has been applied by the redesign chord-wave's tend flow
- Repeated stall or algedonic pressure can be expressed as structured mutation proposals without a separate signal subsystem
