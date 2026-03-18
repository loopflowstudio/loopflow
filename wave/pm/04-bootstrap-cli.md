---
pm_id: '1213718537307877'
---
# 04: PM bootstrap and status CLI

**Finish line:** `lf ops pm init|link|status --provider <asana|linear>` can create or connect a wave and project in either direction, with actionable auth/config errors.

Both providers implement `PmProvider` (`lfd/pm/asana.rs`, `lfd/pm/linear.rs`) with full CRUD, pagination, rate-limit retry, and completion-state semantics. `lf ops auth configure linear` and `lfq auth linear` persist credentials. `read_wave_config` parses `pm.provider` / `pm.project` / optional `pm.team`. `ops/export.rs` creates an Asana project on first export and writes `pm` + `pm_id` state back through `RoadmapItemDocument` (Linear export currently returns an error — the provider client is ready but the export path only dispatches to Asana). The remaining gap is an explicit bootstrap/link/status UX with Linear export parity.

## What to build

### Commands

```bash
lf ops pm init --provider asana --wave scale
lf ops pm init --provider linear --wave scale
lf ops pm init --provider asana --project <gid>
lf ops pm init --provider linear --project <project-id>
lf ops pm link --provider asana --wave scale --project <gid>
lf ops pm link --provider linear --wave scale --project <project-id>
lf ops pm status
lf ops pm status --wave scale
```

Keep provider selection in the PM subcommand rather than creating a parallel `lf ops asana ...` / `lf ops linear ...` command tree.

### `init --wave` (wave exists, no PM project)

1. Read `wave/<name>/README.md` and pull project metadata from the existing wave identity
2. `provider.create_project(name, description)`
3. Reuse the export path to create remote items and write `pm_id` frontmatter
4. Write the `pm` block through the existing wave config helper
5. Report any items that could not be created or linked

### `init --project` (PM project exists, no wave)

1. `provider.list_items(project_id)`
2. Create `wave/<name>/` from the remote project name/description
3. Write wave YAML with the `pm` block
4. Write numbered roadmap items with `pm_id` frontmatter through `RoadmapItemDocument`
5. Keep the README focused on vision/strategy/goals/risks, not a dump of remote metadata

### `link` (both exist, just wire them together)

1. Write the `pm` block
2. Match existing local and remote items by title, then by order as a fallback
3. Write `pm_id` where a confident match exists
4. Report unmatched local and remote items without silently guessing

### `status`

For each linked wave, show:
- provider and project ID
- whether local credentials and required config are present
- local item count vs remote item count
- local items without `pm_id`
- remote items without a local match (once import exists)

## Constraints

- Reuse the existing provider clients, wave-config parsing, and `RoadmapItemDocument` helpers; do not duplicate transport or markdown-edit code.
- `lf ops auth` remains the credential owner. PM bootstrap/status should only read stored credentials.
- Missing `asana.workspace` / `linear.team` must fail with a direct next step.
- If the current top-level `lf ops export` survives during the transition, make it a thin wrapper over the shared PM export path. Do not keep two divergent implementations.

## Done when

- `lf ops pm init --provider asana --wave test` creates and links an Asana project
- `lf ops pm init --provider linear --wave test` creates and links a Linear project
- `lf ops pm init --provider asana --project <gid>` scaffolds a wave from Asana
- `lf ops pm link ...` backfills `pm_id` on matching roadmap items
- `lf ops pm status` surfaces missing auth/config before a sync fails
