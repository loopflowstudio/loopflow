---
pm_id: '1213718054761190'
---
# 05: PM import/export commands, steps, and flow

**Finish line:** `lf ops pm import` and `lf ops pm export` sync a linked wave with either Asana or Linear, and `pm-sync` wraps import → build → export.

`rust/loopflow/src/ops/export.rs` already proves the core export path for Asana: it reads numbered roadmap items, uses `RoadmapItemDocument` for `pm_id`, creates a missing project on first export, and updates or creates remote items. Linear's `PmProvider` is fully implemented (CRUD, pagination, completion-state lookup) but the export dispatcher currently returns an error for Linear — the generalization is mechanical. The missing pieces are Linear export dispatch, an import path, step/flow entry points, and honest ordering behavior.

## What to build

### Generalize the current export path

Turn the existing Asana-only exporter into the shared PM export implementation instead of leaving it as a side branch.

```bash
lf ops pm export --wave scale
```

- Resolve provider + project from the wave's `pm` block
- For items with `pm_id`, call `update_item`
- For items without `pm_id`, call `create_item` and write the returned ID back through `RoadmapItemDocument`
- Keep project bootstrap logic shared with item 04 rather than duplicating it here

### Add import

```bash
lf ops pm import --wave scale
```

- Resolve provider + project from the wave's `pm` block
- `provider.list_items(project_id)`
- Rewrite numbered roadmap items from remote state, preserving `pm_id`
- Remove or clearly flag local items whose remote ID no longer exists
- Normalize every frontmatter edit through `RoadmapItemDocument`

### Add step and flow surfaces

- Built-in step wrappers for import/export so the operations are visible in discovery/help
- `pm-sync` flow that runs import → build → export
- `ship-roadmap` can either branch into `pm-sync` when linked or reuse the same underlying commands directly, but there should be one sync implementation

### Ordering

Do not fake reordering success.

- Asana needs provider-aware move operations (`insert_before` / `insert_after` style behavior)
- Linear may need either a concrete ordering API or a clearly documented limitation
- `RoadmapItemDocument` owns file normalization, not remote ordering semantics

## Constraints

- Mechanical ops only — no LLM step for sync itself.
- Import is a pull; export is a push.
- Replace or wrap the current top-level `lf ops export`; do not keep an Asana-only implementation drifting away from the shared PM commands.
- Provider-specific reorder behavior belongs in the provider/export layer, not in generic markdown helpers.

## Done when

- `lf ops pm import --wave test` pulls items from both Asana and Linear
- `lf ops pm export --wave test` pushes items to both Asana and Linear
- New remote IDs are written back through `RoadmapItemDocument`
- `lf pm-sync` (or the equivalent built-in flow entry) is discoverable in `lf --help`
- Reorder behavior is either implemented per provider or reported as an explicit limitation
