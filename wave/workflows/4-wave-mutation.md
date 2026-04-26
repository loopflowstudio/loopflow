---
asana_id: '1213883255350033'
linear_id: 98e766f3-8785-4bf2-9af1-8c23a93fad8e
notion_id: 32af8f99-3d81-81bd-ab89-f9303b2a1563
---
# Wave mutation

**Finish line:** The `mutate` step can modify wave configuration through a structured mutation API. Direction, area, flow, agent, work items, triggers, and lifecycle are all mutable. Mutations are logged and reversible.

## Context

`mutate` already exists as a shipped builtin step that composes and executes changes in one pass. It edits wave YAML and item files directly, while the waves API / `update_wave` path syncs runtime fields (`flow`, `direction`, `area`, `status`, `agent`, `step_agents`) back into lfd. The governance flows (`govern-*`) and `garden-act` all terminate with `mutate`.

What's missing is one typed mutation layer that spans all levers, logs each change, and preserves enough prior state to revert cleanly. Currently `mutate` operates through direct file edits without structured logging or reversibility.

The levers stay the same:
- **Direction** — shift what a wave optimizes for
- **Area** — tighten or widen scope
- **Flow** — change the process
- **Work items** — rewrite, reprioritize, add, or delete work
- **Agent / step agents** — change model choice where it matters
- **Triggers** — adjust cadence or sources
- **Lifecycle** — pause, resume, split, combine, or prune

## What to build

1. **Mutation API.** A wave-scoped mutation endpoint or loader accepts a list of structured mutations and applies them.
2. **Mutation log.** Record who requested the mutation (garden flow or human), why, when, what changed, and the previous value.
3. **Reversibility.** Store enough prior state to revert an individual mutation cleanly.
4. **Review integration.** Mutation proposals survive the review step unchanged so approval can execute the exact payload that was reviewed.
5. **Pressure-to-mutation mapping.** Repeated incidents, stall patterns, and coordination conflicts turn into structured mutation candidates instead of ad hoc file edits.

## Done when

- The mutation API accepts and executes all planned lever types
- Every mutation is logged with actor, rationale, and previous value
- Mutations are individually revertible
- Review uses the same payload for proposal and approval
- At least one real mutation has been applied by root's garden flow through the structured API
