---
asana_id: '1213718325464924'
linear_id: d4635c4a-2d1f-46dc-bf1f-76241bef5d73
---
# 06: Item lifecycle comments and completion

**Finish line:** PR open, run failure, and merge can comment on or complete the specific ingested PM item because the run retains stable roadmap-item identity after `ingest`.

Wave-level PM import/export now happens automatically at PR-oriented run start/end. The missing piece is item identity: `ingest` moves a numbered roadmap file into `scratch/`, and the run currently loses the durable link needed to look up `id_for(provider)` later.

**Open design question:** Push scope — should lifecycle comments push the whole item state or just the event payload (PR URL, error message)? Event-only is simpler and avoids accidentally overwriting human edits in the PM tool.

Both `AsanaClient` and `LinearClient` already implement the required `comment` and `complete_item` methods. This item wires those calls into the lifecycle path that actually knows which roadmap item the run is working on.

## What to build

### Persist the ingested item link

- When `ingest` picks an item, record stable metadata on the run or run snapshot: the original numbered item filename/slug/path and any provider IDs known at ingest time.
- Keep that metadata valid after the item moves into `scratch/`, after branch rotation, and after export writes additional provider IDs.
- Completion must fire from the event that proves the work landed (merge/land), not merely from "run reached the end of its local steps."

### Lifecycle actions

| Lifecycle event | Action |
|-----------------|--------|
| PR created | `provider.comment(id_for(provider), "PR opened: {url}")` |
| Run failed | `provider.comment(id_for(provider), "Run failed: {error}")` |
| PR merged / landed | `provider.complete_item(id_for(provider))` |

Apply those actions to every configured provider role that has a linked item ID. Skip providers with no `id_for(provider)` instead of re-matching by title.

### Resolution

1. Run metadata → the ingested roadmap item identity
2. Wave YAML / repo config → provider roles and project IDs
3. `RoadmapItemDocument` or stored provider IDs → `id_for(provider)`
4. Construct provider client from stored credentials + config
5. Call `comment` / `complete_item`

### Error handling

Best-effort: if a PM API call fails, log a warning and continue. Never block wave execution, PR creation, or merge handling on external sync. Both providers already have rate-limit retry logic (`RATE_LIMIT_RETRIES` and `retry_after_delay` in `pm::mod.rs`), so transient 429s are handled automatically.

## Constraints

- PM sync failures must not affect wave execution.
- Stable item identity must survive `ingest` moving files into `scratch/`.
- No fuzzy title matching at lifecycle time — use the item IDs already carried by roadmap frontmatter (`id_for(provider)` on `RoadmapItemDocument`) / run metadata.
- Reuse the existing provider client methods (`comment`, `complete_item` on `AsanaClient`/`LinearClient`); do not create a second PM transport path for lifecycle events.

## Done when

- Creating a PR adds a comment on the linked PM item with the PR URL
- A failed run adds an error comment on the linked PM item
- Merging or landing the PR completes the linked PM item
- PM API failures log warnings but do not affect the run or merge path
- Waves without `pm` configuration, or items without provider IDs, are completely unaffected
