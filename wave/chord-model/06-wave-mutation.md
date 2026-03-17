# 06: Wave Mutation

**Finish line:** The chord's apply step can modify wave configuration through a structured mutation API. Direction, area, flow, agent, work items, triggers — all mutable. Mutations are logged and reversible.

## Context

The tend flow's propose step suggests changes. The apply step executes them. Currently wave config is YAML on disk and wave state is in lfd's database. Mutations need to work across both.

The levers (from design discussion):

- **Direction**: shift what a wave optimizes for. Add `care` if shipping sloppy. Add `simplicity` if over-engineering. Remove directions that pull focus.
- **Area**: tighten scope if producing shallow work across too many files. Widen if missing the point.
- **Flow**: change the process. Inject a research step if building without understanding. Remove gates if they're ceremony.
- **Work items**: re-prioritize, rewrite stale items, delete non-issues, add new items discovered by tend.
- **Agent**: shift model. Opus for research/design. Sonnet for implementation. Haiku for cleanup.
- **Step agents**: different models for different steps in the flow.
- **Triggers**: change frequency, add/remove trigger sources.
- **Lifecycle**: pause, resume, split a wave into two, combine waves, prune a wave entirely.

## What to build

1. **Mutation API.** `POST /v0/waves/{id}/mutate` accepts a list of mutations:
   ```json
   {
     "mutations": [
       {"type": "set_direction", "value": ["clarity", "care"]},
       {"type": "set_area", "value": ["rust/loopflow/src/lfd/"]},
       {"type": "add_work_item", "value": {"title": "...", "body": "..."}},
       {"type": "set_agent", "value": "claude:opus"},
       {"type": "pause"},
       {"type": "set_flow", "value": "grind"}
     ]
   }
   ```

2. **Mutation log.** Every mutation recorded with: who requested it (chord or human), why (link to assessment), when, what changed. Queryable via API.

3. **Reversibility.** Mutations store the previous value. `POST /v0/waves/{id}/revert/{mutation_id}` undoes a specific mutation. The chord (or human) can roll back if a change made things worse.

4. **Apply step integration.** The tend flow's apply step uses the mutation API. Auto-apply mutations go through directly. Needs-human mutations create block queue entries with the proposed mutation attached.

## Done when

- Mutation API accepts and executes all lever types
- Every mutation is logged with actor, rationale, and previous value
- Mutations are individually revertible
- Apply step uses the API for both auto and human-gated mutations
- At least one real mutation has been applied by the redesign chord's tend flow
