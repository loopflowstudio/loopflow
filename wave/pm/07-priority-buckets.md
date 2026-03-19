# 07: Priority buckets across prompts, ingest, and provider sync

**Finish line:** loopflow roadmap planning uses four semantic priority buckets instead of pretending every provider shares one exact global order, and that model works cleanly with Asana and Linear.

The current roadmap model leaks numeric ordering everywhere: built-in prompts talk about `01-*`, `02-*`, docs describe stages, `ingest` picks the lowest numeric prefix, and PM sync carries `rank` as the main planning signal. That is a bad fit for the PM tools we actually have.

## Priority model

- **P0** — The current codebase is broken and needs to be fixed before forward progress can continue.
- **P1** — This is a clear next step.
- **P2** — This is a big idea and a "when not an if" item, but not immediately the right next thing.
- **P3** — This is speculative.

Prompts should speak in those semantics. Provider adapters should translate them into the native vocabulary the user sees in the tool.

## What to build

### Prompt and docs redo

1. Rewrite the built-in wave-authoring / update-wave / ingest guidance so roadmap writing bins items into four buckets instead of ordering them as stages.
2. Update docs that currently teach `01-*`, `02-*` as the primary planning model.
3. Keep the prompt guidance semantic: the labels may be `P0/P1/P2/P3`, `Urgent/High/Medium/Low`, or equivalent in the remote tool.

### Ingest semantics

1. Update `ops/ingest.rs` so it understands bucketed priorities.
2. `ingest` should choose the highest-priority non-empty bucket first.
3. Do not block this item on solving the within-bucket tie-breaker; other waves can work on that later.

### Provider mapping

1. Asana: map the shared buckets onto a custom field.
2. Linear: map the shared buckets onto native priority values (`Urgent`, `High`, `Medium`, `Low`).
3. Keep provider-specific label choices out of the prompt layer.

## Constraints

- The shared model is binning, not exact total ordering.
- Use the provider's natural vocabulary in the remote UI where possible.
- Do not take on Notion here; prove the model with Asana and Linear first.
- Keep the within-bucket tie-break rule explicitly deferred.

## Done when

- Built-in prompt/docs teach four roadmap buckets instead of numeric stages
- `ingest` understands the bucket model
- Asana and Linear can round-trip the bucket meaning cleanly
- The model no longer depends on exact rank as the main planning signal
