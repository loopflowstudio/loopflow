---
asana_id: '1213718325464924'
linear_id: 0c9f7553-07ad-4580-961c-1b49b9f6b052
---
# 07: Run lifecycle → PM sync

**Finish line:** PR creation and merge automatically update the corresponding PM item. Best-effort, non-blocking.

## What to build

Both `AsanaClient` and `LinearClient` already implement the required `comment` and `complete_item` methods. This item wires those calls into executor lifecycle transitions.

### Sync points

Add sync points to the wave run lifecycle in the executor:

| Run state transition | Action |
|---------------------|--------|
| PR created | `provider.comment(id_for(provider), "PR opened: {url}")` |
| PR merged (run complete) | `provider.complete_item(id_for(provider))` |
| Run failed | `provider.comment(id_for(provider), "Run failed: {error}")` |

### Resolution

The run already knows its wave and roadmap item. Resolve PM context through the existing files and helpers:

1. Run → wave → wave YAML → `pm` block (`provider`, `project`)
2. Run → roadmap item file → `RoadmapItemDocument` → `id_for(provider)`
3. Construct provider client from stored credentials + config
4. Call the appropriate method

Skip the sync entirely when the wave has no `pm` block or the item has no provider ID (`id_for(provider)` returns None).

### Error handling

Best-effort: if the PM API call fails, log a warning and continue. Never block wave execution on external sync. Both providers already have rate-limit retry logic (`RATE_LIMIT_RETRIES` and `retry_after_delay` in `pm::mod.rs`), so transient 429s are handled automatically.

### Implementation

Synchronous dispatch initially — call the provider directly from the run lifecycle transition handler. No event bus, no queue, no subscriber layer unless real latency proves that necessary.

## Constraints

- PM sync failures must not affect wave execution.
- No new infrastructure: direct function call, not an event system.
- Only fires when the wave has a `pm` block and the item has a provider ID.

## Done when

- Merging a PR for a wave run marks the corresponding PM item complete
- Creating a PR adds a comment on the PM item with the PR URL
- A failed run adds an error comment on the PM item
- PM API failure logs a warning but doesn't affect the run
- Waves without `pm` configuration are completely unaffected
