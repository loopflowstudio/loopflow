---
pm_id: '1213718325464924'
---
# 07: Run lifecycle → PM sync

**Finish line:** PR creation, merge, and failure events comment on or complete the corresponding PM item without affecting wave execution.

Both providers now expose the verbs this needs (`comment`, `complete_item`), and the daemon already has patterns for best-effort side effects: queue/attention updates log warnings instead of breaking the run. The missing work is wiring run + PR lifecycle data back to the roadmap item's `pm_id` and calling the provider at the right transition points.

## What to build

### Sync points

| Transition | PM action |
|------------|-----------|
| PR created | `provider.comment(pm_id, "PR opened: {url}")` |
| PR merged / run complete | `provider.complete_item(pm_id)` |
| Run failed | `provider.comment(pm_id, "Run failed: {error}")` |

### Resolution path

1. Run → wave → wave YAML → `pm` block
2. Run → roadmap item file → `RoadmapItemDocument` → `pm_id`
3. Stored credentials + config → provider client
4. Call the provider method and log any failure

The store already tracks PR state and merge events; reuse that data flow instead of adding a second source of truth for PR URLs or merge timing.

## Constraints

- Best-effort only: PM failures log warnings and stop there.
- No new event bus or background sync service unless the direct call path proves too slow.
- Skip the sync entirely when the wave has no `pm` block or the roadmap item has no `pm_id`.
- Keep provider-specific completion details inside the provider client (`workflowStates` lookup for Linear, rich-text quirks for Asana).

## Done when

- Creating a PR adds a PM comment with the PR URL
- Merging/completing the run marks the remote item complete
- A failed run adds an error comment
- PM API failures are visible in logs but do not affect run status
- Waves without PM configuration behave exactly as before
