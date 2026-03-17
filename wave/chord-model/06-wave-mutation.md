# 06: Wave Mutation

**Finish line:** The chord-wave's `apply` step can modify wave configuration through a structured mutation API. Direction, area, flow, agent, work items, triggers, and lifecycle are all mutable. Mutations are logged and reversible.

## Context

`draft-chord` suggests changes. `apply-chord` executes them. After bootstrap, the constraint is sharper: mutations have to operate on the waves-only model that replaced chord CRUD. Changes should update ordinary wave config and runtime state, not smuggle chord-specific state back into the system.

The manual execution path exists now: `apply-chord` edits wave YAML and files, then syncs lfd with `lf ops update-wave`. What's missing is a typed mutation layer with logging, reversibility, and a durable record of who changed what and why.

The levers stay the same:
- **Direction** — shift what a wave optimizes for
- **Area** — tighten or widen scope
- **Flow** — change the process
- **Work items** — rewrite, reprioritize, add, or delete work
- **Agent / step agents** — change model choice where it matters
- **Triggers** — adjust cadence or sources
- **Lifecycle** — pause, resume, split, combine, or prune

## What to build

1. **Mutation API.** A wave-scoped mutation endpoint or loader accepts a list of structured mutations and applies them to ordinary waves.

2. **Mutation log.** Record who requested the mutation (chord-wave or human), why, when, what changed, and the previous value.

3. **Reversibility.** Store enough prior state to revert an individual mutation cleanly.

4. **Apply-step integration.** `apply-chord` uses the mutation API for both auto-applied changes and human-gated proposals. Human-review items keep the proposed mutation attached so approval can execute the same payload.

## Done when

- The mutation API accepts and executes all planned lever types
- Every mutation is logged with actor, rationale, and previous value
- Mutations are individually revertible
- `apply-chord` uses the API for both auto and human-gated changes
- At least one real mutation has been applied by the redesign chord-wave's tend flow
