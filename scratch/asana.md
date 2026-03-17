# Project Management Integration

**Finish line:** Waves sync bidirectionally with external PM tools. Asana and Linear ship together, sharing a common trait. Import, export, bootstrap CLI, and automatic PR-lifecycle sync all work through the abstraction.

## Mapping

| Loopflow | Asana | Linear |
|----------|-------|--------|
| Wave | Project | Project |
| Roadmap item (`01-foo.md`) | Task | Issue |
| Item content | Task description | Issue description (markdown) |
| Priority rank | Sort order | Priority + sort order |
| Item GID | Task GID (string) | Issue ID (UUID) |

## Data structures

Wave YAML gains a `pm` block — provider-agnostic:

```yaml
flow: ship-wave
area:
  - swift/
pm:
  provider: asana
  project: "1234567890"
```

```yaml
pm:
  provider: linear
  project: "PROJ-123"
```

Roadmap item `.md` files gain a `pm_id` frontmatter field:

```markdown
---
pm_id: "9876543210"
---
# 01: Ship offline sync
```

`pm_id` is provider-agnostic — the wave YAML tells you which provider to resolve it against.

## Trait

```rust
#[async_trait]
trait PmProvider: Send + Sync {
    /// Create a new project, return its external ID
    async fn create_project(&self, name: &str, description: &str) -> Result<String>;

    /// List items in a project, ordered by priority
    async fn list_items(&self, project_id: &str) -> Result<Vec<PmItem>>;

    /// Create an item, return its external ID
    async fn create_item(&self, project_id: &str, item: &PmItemCreate) -> Result<String>;

    /// Update an existing item
    async fn update_item(&self, item_id: &str, update: &PmItemUpdate) -> Result<()>;

    /// Mark an item complete
    async fn complete_item(&self, item_id: &str) -> Result<()>;

    /// Add a comment to an item
    async fn comment(&self, item_id: &str, body: &str) -> Result<()>;
}

struct PmItem {
    id: String,
    name: String,
    description: String,
    rank: u32,
    completed: bool,
}

struct PmItemCreate {
    name: String,
    description: String,
    rank: u32,
}

struct PmItemUpdate {
    name: Option<String>,
    description: Option<String>,
    rank: Option<u32>,
}
```

Asana implements this over REST, Linear over GraphQL. Both return the same `PmItem` shape.

## Bootstrap CLI

Provider-specific commands, shared behavior:

```bash
# Asana
lf ops asana init --wave scale          # create Asana project from existing wave
lf ops asana init --project <gid>       # scaffold wave from existing Asana project
lf ops asana link --wave scale --project <gid>
lf ops asana status

# Linear
lf ops linear init --wave scale         # create Linear project from existing wave
lf ops linear init --project PROJ-123   # scaffold wave from existing Linear project
lf ops linear link --wave scale --project PROJ-123
lf ops linear status
```

### `init` from wave (no external project exists)

1. Read `wave/<name>/README.md` for project name and description
2. Call `provider.create_project(name, description)`
3. For each roadmap item: `provider.create_item(project_id, item)`
4. Write `pm` block to wave YAML
5. Write `pm_id` to each roadmap item frontmatter

### `init` from external (no wave exists)

1. `provider.list_items(project_id)` to fetch all items
2. Create `wave/<name>/` with YAML, README scaffold
3. Write roadmap items as numbered `.md` files with `pm_id` frontmatter
4. Wire up `pm` block in wave YAML

### Config

```yaml
# .lf/config.yaml
asana:
  workspace: "1234567890"
  default_team: "9876543210"     # optional

linear:
  team: "TEAM-ID"               # default team for new projects
```

## Steps

### `import-pm`

```bash
lf import-pm                    # uses pm.provider from wave YAML
```

1. Read `pm` block from wave YAML to determine provider and project
2. `provider.list_items(project_id)`
3. For each item: generate `{NN}-{slugified-name}.md`, write content, store `pm_id`
4. Commit changes

Import is a pull — external source wins on conflict.

### `export-pm`

```bash
lf export-pm                    # push wave state to external PM
```

1. Read `pm` block from wave YAML
2. For each roadmap item:
   - Has `pm_id` → `provider.update_item(id, update)`
   - No `pm_id` → `provider.create_item(project_id, item)`, write `pm_id` back
3. Sync order to match filename prefix

### Flows

```yaml
# flow: pm-sync (standalone)
- import-pm
- build
- export-pm

# ship-roadmap gains export-pm at the end when pm is configured
# ingest gains auto-import at the start when pm is configured
```

## Ingest integration

When a wave has a `pm` block, `ingest` refreshes from the tracker before picking the next item:

1. Check wave YAML for `pm` block
2. If present: pull latest items from provider (reordering, description updates, completion status)
3. Pick next unshipped item by priority rank
4. Write to `scratch/<branch>.md` as usual

This lets planning happen entirely in Asana/Linear — reprioritize, flesh out descriptions, mark things done — and `ingest` picks up whatever's next. No manual `import-pm` needed in the `ship-wave` flow.

The `ship-roadmap` flow becomes: ingest (with auto-import) → kickoff → build → gate → export-pm (sync completion back).

## Run lifecycle events → PM sync

**Option B chosen:** internal event dispatch from wave run state transitions.

The wave run knows which roadmap item it's working on. When state changes, dispatch to the PM provider:

| Event | PM action |
|-------|-----------|
| Run starts (PR created) | `comment(item_id, "PR opened: {url}")` |
| Run completes (PR merged) | `complete_item(item_id)` |
| Run fails | `comment(item_id, "Run failed: {error}")` |

Synchronous dispatch initially — call the provider directly from the run lifecycle. Best-effort: log errors, don't block execution.

The subscriber reads `pm` from the wave YAML to resolve the provider. Waves without a `pm` block are unaffected.

## Auth

Both providers added to the existing auth system:

| Provider | Auth method | Setup |
|----------|-------------|-------|
| Asana | Personal Access Token | `lfq auth asana` (paste PAT) |
| Linear | API Key | `lfq auth linear` (paste key) |

Both stored encrypted alongside existing provider tokens. Same `Provider` enum, same credential storage path.

## Pieces

1. **`PmProvider` trait + types** — the abstraction boundary
2. **Asana client** — REST implementation of `PmProvider`
3. **Linear client** — GraphQL implementation of `PmProvider`
4. **Auth** — `Asana` + `Linear` variants in `Provider` enum, credential storage
5. **PM config** — `pm` block in wave YAML, provider config in `.lf/config.yaml`
6. **Bootstrap CLI** — `lf ops asana/linear init/link/status`
7. **`import-pm` step** — pull external → `wave/`
8. **`export-pm` step** — push `wave/` → external
9. **Run lifecycle dispatch** — emit events on run state transitions
10. **PM subscriber** — listen to events, call provider

## Constraints

- Asana rate limit: 1500 req/min. Linear: 400 req/min (stricter). Batch where possible.
- Asana GIDs are strings; Linear IDs are UUIDs. Both stored as strings in `pm_id`.
- Asana sections / Linear project statuses could map to wave status — v2.
- Conflict policy: "don't do that." Loopflow owns `.md` files, PM tools own planning/visibility.
- Linear descriptions are markdown — direct content transfer. Asana descriptions are rich text — may need conversion.

## Done when

- `lf ops asana init --wave test-wave` creates an Asana project with tasks matching roadmap items
- `lf ops linear init --wave test-wave` creates a Linear project with issues matching roadmap items
- `lf import-pm` pulls items from the configured provider into `wave/`
- `lf export-pm` pushes wave state to the configured provider
- `lf ingest` on a PM-synced wave refreshes from the tracker before picking the next item
- Merging a PR for a wave run marks the corresponding PM item complete
- `lfq auth asana` and `lfq auth linear` store and retrieve credentials
