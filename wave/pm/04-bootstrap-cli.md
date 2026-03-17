# 04: Bootstrap CLI

**Finish line:** `lf ops asana init` and `lf ops linear init` create and link projects in both directions. `lf ops asana/linear status` shows sync state.

The file formats, auth, and config plumbing are already on main: wave YAML uses `PmConfig` (`pm/mod.rs`), roadmap frontmatter uses `RoadmapItemDocument` + `pm_id`, `lfq auth asana|linear` stores credentials via `Provider::Asana`/`Provider::Linear`, and `AsanaConfig`/`LinearConfig` in `engine::config` carry workspace/team IDs. This item wires real commands onto those existing shapes rather than creating another representation.

## What to build

### Commands

```bash
# Create PM project from existing wave
lf ops asana init --wave scale
lf ops linear init --wave scale

# Create wave from existing PM project
lf ops asana init --project <gid>
lf ops linear init --project PROJ-123

# Link existing wave to existing project
lf ops asana link --wave scale --project <gid>
lf ops linear link --wave scale --project PROJ-123

# Show sync state
lf ops asana status
lf ops linear status
```

Implement these as deterministic Rust ops commands so they compose cleanly with `lf ops commit`, existing repo detection, and future automation.

### `init --wave` (wave exists, no PM project)

1. Read `wave/<name>/README.md` — use wave name as project name, vision section as description
2. `provider.create_project(name, description)`
3. For each `NN-*.md`: `provider.create_item(project_id, item)`
4. Write `pm` block to wave YAML through the existing PM config shape
5. Write `pm_id` to each roadmap item through `RoadmapItemDocument`
6. Commit changes

Missing provider config should fail here with actionable messages (`asana.workspace`, optional `asana.default_team`, `linear.team`) instead of bubbling up opaque API errors.

### `init --project` (PM project exists, no wave)

1. `provider.list_items(project_id)`
2. Create `wave/<name>/` — derive name from project name (slugified)
3. Write wave YAML with `pm` block
4. Write README scaffold from project description
5. Write roadmap items as `NN-slug.md` with `pm_id` frontmatter
6. Commit changes

### `link` (both exist, just wire up)

1. Write `pm` block to wave YAML
2. Match items by name/position — best effort
3. Write `pm_id` where matches found
4. Report unmatched items from both sides

### `status`

For each wave with a `pm` block:
- Wave name, provider, project ID
- Item count (local vs remote)
- Last sync time (if tracked)
- Unlinked items (local without `pm_id`, remote without local match)

## Constraints

- All commands commit their changes (YAML + frontmatter updates)
- `init` should fail clearly if the wave or project already has a `pm` link (use `link` to reconnect)
- Provider resolves from command name (`lf ops asana` → Asana, `lf ops linear` → Linear)
- Reuse existing auth lookup and config parsing — no PM-only credential plumbing
- Reuse the existing Asana/Linear provider clients directly; the commands should orchestrate wave/project/file changes, not duplicate HTTP code.

## Done when

- `lf ops asana init --wave test` creates an Asana project with tasks matching roadmap items
- `lf ops linear init --wave test` creates a Linear project with issues matching roadmap items
- `lf ops asana init --project <gid>` scaffolds a wave from Asana
- `lf ops asana status` shows sync state for all linked waves
- All commands commit their changes
