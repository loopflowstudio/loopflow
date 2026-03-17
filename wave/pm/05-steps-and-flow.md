# 05: import-pm, export-pm steps and pm-sync flow

**Finish line:** `lf import-pm` and `lf export-pm` work as composable steps. `pm-sync` flow chains import → build → export.

The roadmap file round-trip helper already exists in `RoadmapItemDocument`. Import/export should use that helper for every `pm_id` read/write instead of open-coding frontmatter edits.

## What to build

### `import-pm` step

`.lf/steps/import-pm.md` or builtin step.

Not an LLM step — this is a mechanical operation. Implemented as a Rust ops command that the step invokes:

```bash
lf ops pm import    # reads pm.provider from wave YAML, pulls items
```

1. Read `pm` block from wave YAML
2. Resolve provider, get credentials
3. `provider.list_items(project_id)`
4. For each item: write/update `NN-slug.md` with content and `pm_id`
5. Remove or flag local items whose `pm_id` no longer exists remotely
6. Commit if changes were made

Import is a pull: the external PM state wins.

### `export-pm` step

Same pattern — mechanical, not LLM:

```bash
lf ops pm export    # reads pm.provider from wave YAML, pushes state
```

1. Read `pm` block from wave YAML
2. For each roadmap item:
   - Has `pm_id` → `provider.update_item(id, update)`
   - No `pm_id` → `provider.create_item(project_id, item)`, write `pm_id` back through `RoadmapItemDocument`
3. Sync order to match filename prefix
4. Commit if `pm_id` values were written

Export is a push: loopflow's markdown and filename order become the desired remote state.

### `pm-sync` flow

```yaml
# flows/pm-sync.yaml
steps:
  - ops: pm import
  - implement
  - ops: pm export
```

### Step definitions

The step `.md` files are thin — they just invoke the ops command:

```markdown
---
requires: wave with pm block
produces: updated wave/ items
---
Import items from the configured PM provider into wave/.

Run `lf ops pm import` and commit any changes.
```

## Constraints

- These are ops commands, not LLM steps. No agent involved — deterministic sync.
- Steps exist so they're composable in flows and visible in `lf --help`.
- Import overwrites local; export creates/updates remote. No merge logic.

## Done when

- `lf ops pm import` pulls items from configured provider
- `lf ops pm export` pushes items to configured provider
- `lf pm-sync` runs the full flow (import → build → export)
- Steps appear in `lf --help` output
