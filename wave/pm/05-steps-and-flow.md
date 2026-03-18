---
asana_id: '1213718054761190'
linear_id: b83b8ee6-7a17-4090-87fa-aedeac30c524
---
# 05: PM sync steps and flow

**Finish line:** `lf import-pm` and `lf export-pm` exist as thin step surfaces over `lf ops pm import|export`, and `pm-sync` composes them for manual or non-PR flows.

The mechanical sync core already exists: `lf ops pm import`, `lf ops pm export`, and `lf ops pm sync` resolve provider roles, use `id_for(provider)` / `set_id(provider, id)`, and the executor already runs import/export automatically around PR-oriented runs. This item is about making that machinery composable in the normal step/flow surface, not rebuilding sync semantics.

## What to build

### Thin step wrappers

- Add built-in `import-pm` and `export-pm` steps that invoke the corresponding ops commands.
- Keep them mechanical and headless. No agent reasoning, no second sync implementation.
- Make them discoverable in the normal step surface (`lf --help`, discovery, prompt assembly).

### `pm-sync` flow

- Add a `pm-sync` flow that chains `import-pm` → `implement` → `export-pm`.
- Use it for manual runs and non-PR flows where executor lifecycle hooks do not fire automatically.
- Document that PR-oriented runs already do wave-level import/export at run boundaries, so `pm-sync` is explicit composition rather than a replacement for the default executor behavior.

### Shared rules

- Do not duplicate sync logic from `ops/pm.rs`.
- Keep provider-role resolution and frontmatter writes in the existing ops path.
- Pass wave selection through unchanged so the thin steps stay transparent wrappers.

## Open design questions

- **Surface area:** Should `pm-sync` be a dedicated built-in flow, or just a documented pattern using ops items?
- **Redundant sync:** For PR-oriented flows that already auto-sync at run start/end, should `pm-sync` stay available, noop the duplicate phases, or be discouraged in docs?
- **Step location:** Where should the thin wrappers live so they stay aligned with built-in discovery/help without creating a second source of truth for sync behavior?

## Constraints

- These steps are wrappers around deterministic ops commands, not new agent behaviors.
- Steps exist so they're composable in flows and visible in `lf --help`.
- Keep provider-specific sync semantics in `ops/pm.rs` / `ops/export.rs`, not in the step wrappers.
- Avoid creating a second default lifecycle that fights the executor's automatic PM hooks.

## Done when

- `lf import-pm` invokes the existing import logic through a built-in step surface
- `lf export-pm` invokes the existing export logic through a built-in step surface
- `lf pm-sync` runs the composed flow for manual or non-PR work
- The new step/flow surface appears in discovery/help output
- PR-oriented flows do not end up with two conflicting PM sync paths
