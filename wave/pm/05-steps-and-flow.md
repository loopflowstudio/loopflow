---
asana_id: '1213718054761190'
linear_id: b83b8ee6-7a17-4090-87fa-aedeac30c524
---
# 05: import-pm, export-pm steps and pm-sync flow

**Finish line:** `lf import-pm` and `lf export-pm` work as composable steps. `pm-sync` flow chains import → build → export.

The roadmap file round-trip helper already exists in `RoadmapItemDocument`, and both Asana and Linear implement the provider seam. Import/export should use `id_for(provider)` and `set_id(provider, id)` for every provider-ID read/write instead of open-coding frontmatter edits.

Ordering is the tricky part. Asana does not expose a numeric rank field, so export cannot treat `PmItemUpdate.rank` as a real remote reorder. Note that `PmTextUpdate` already filters rank-only updates at the trait boundary — both providers skip API calls when only rank changed. When filename order matters, implement provider-aware ordering (`insert_before` / `insert_after` for Asana, whatever Linear actually supports) or emit a clear limitation. Do not fake a successful reorder by writing a local rank nobody uses.

## Sync model

The wave files are the local state. No separate snapshot or ledger. Import produces what the remote looks like; export pushes local state to the remote.

Import and export are one-directional — one side wins entirely. The `pm-sync` flow is the steady-state operation.

## What to build

### `import-pm` step

Not an LLM step — this is a mechanical operation. Implemented as a Rust ops command that the step invokes:

```bash
lf ops pm import    # reads PM provider roles + project ids, pulls from the RW provider
```

1. Read `pm` block from wave YAML
2. Resolve provider, get credentials
3. `provider.list_items(project_id)`
4. For each item: write/update `NN-slug.md` with content and provider ID via `set_id(provider, id)`
5. Remove or flag local items whose provider ID no longer exists remotely
6. Commit if changes were made

Import is a pull: the external PM state wins.

### `export-pm` step

Same pattern — mechanical, not LLM:

```bash
lf ops pm export    # reads PM provider roles + project ids, pushes to the RW + export providers
```

1. Read `pm` block from wave YAML
2. For each roadmap item:
   - Has provider ID (`id_for(provider)` returns Some) → `provider.update_item(id, update)`
   - No provider ID → `provider.create_item(project_id, item)`, write ID back via `set_id(provider, id)`
3. Sync order to match filename prefix
4. Commit if provider ID values were written

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

## Open design questions

These surfaced during sync design work and need decisions during implementation:

- **Deletes:** Should `import` automatically delete local items missing from remote, or require `--destructive`?
- **Normalization:** How fuzzy should "same content" be for detecting changes? Exact match will false-positive on whitespace diffs from provider round-trips.
- **Order sync:** Asana has relative move operations; Linear doesn't expose ordering the same way. Sync order or document the limitation?

## Constraints

- These are ops commands, not LLM steps. No agent involved — deterministic sync.
- Steps exist so they're composable in flows and visible in `lf --help`.
- Import overwrites local; export creates/updates remote. No merge logic.
- Keep provider-specific reorder behavior out of `RoadmapItemDocument`; this item should normalize file I/O once and delegate remote ordering to the provider/export layer.

## Done when

- `lf ops pm import` pulls items from configured provider
- `lf ops pm export` pushes items to configured provider
- `lf pm-sync` runs the full flow (import → build → export)
- Steps appear in `lf --help` output
