# 07: Run lifecycle → PM sync

**Finish line:** PR creation and merge automatically update the corresponding PM item. Best-effort, non-blocking.

## What to build

The provider seam already has the required verbs (`comment`, `complete_item`), and Asana implements them today. This item is about wiring those calls into executor lifecycle transitions once both providers are available.

### Sync points

Add sync points to the wave run lifecycle in the executor:

| Run state transition | Action |
|---------------------|--------|
| PR created | `provider.comment(pm_id, "PR opened: {url}")` |
| PR merged (run complete) | `provider.complete_item(pm_id)` |
| Run failed | `provider.comment(pm_id, "Run failed: {error}")` |

### Resolution

The run already knows its wave and roadmap item. Resolve PM context through the existing files and helpers:

1. Run → wave → wave YAML → `pm` block (`provider`, `project`)
2. Run → roadmap item file → `RoadmapItemDocument` → `pm_id`
3. Construct provider client from stored credentials + config
4. Call the appropriate method

Skip the sync entirely when the wave has no `pm` block or the item has no `pm_id`.

### Error handling

Best-effort: if the PM API call fails, log a warning and continue. Never block wave execution on external sync.

### Implementation

Synchronous dispatch initially — call the provider directly from the run lifecycle transition handler. No event bus, no queue, no subscriber layer unless real latency proves that necessary.

## Constraints

- PM sync failures must not affect wave execution.
- No new infrastructure: direct function call, not an event system.
- Only fires when the wave has a `pm` block and the item has a `pm_id`.

## Done when

- Merging a PR for a wave run marks the corresponding PM item complete
- Creating a PR adds a comment on the PM item with the PR URL
- A failed run adds an error comment on the PM item
- PM API failure logs a warning but doesn't affect the run
- Waves without `pm` configuration are completely unaffected
