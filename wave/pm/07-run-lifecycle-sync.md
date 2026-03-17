# 07: Run lifecycle → PM sync

**Finish line:** PR creation and merge automatically update the corresponding PM item. Best-effort, non-blocking.

## What to build

### Event dispatch

Add sync points to the wave run lifecycle in the executor:

| Run state transition | Action |
|---------------------|--------|
| PR created | `provider.comment(pm_id, "PR opened: {url}")` |
| PR merged (run complete) | `provider.complete_item(pm_id)` |
| Run failed | `provider.comment(pm_id, "Run failed: {error}")` |

### Resolution

The run knows its wave and roadmap item. To find the `pm_id`:

1. Run → wave → wave YAML → `pm` block (provider + project)
2. Run → roadmap item → frontmatter → `pm_id`
3. Construct provider client from credentials
4. Call the appropriate method

### Error handling

Best-effort: if the PM API call fails, log a warning and continue. Never block wave execution on external sync. Reasons:

- PM tool might be down
- Credentials might have expired
- Item might have been deleted externally

### Implementation

Synchronous dispatch initially — call the provider directly from the run lifecycle transition handler. No event bus, no queue, no subscribers. Just a function call at the right moment.

If this becomes a bottleneck (slow API calls blocking the executor), extract to an async task queue. But that's unlikely given the low frequency of PR events.

## Constraints

- Non-blocking: PM sync failures must not affect wave execution
- No new infrastructure: direct function call, not an event system
- Only fires when the wave has a `pm` block and the item has a `pm_id`

## Done when

- Merging a PR for a wave run marks the corresponding PM item complete
- Creating a PR adds a comment on the PM item with the PR URL
- A failed run adds an error comment on the PM item
- PM API failure logs a warning but doesn't affect the run
- Waves without `pm` configuration are completely unaffected
