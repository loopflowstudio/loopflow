# PM Bootstrap CLI

## Problem

Setting up a PM link today requires manual YAML and frontmatter edits — write `pm:` block in wave config, add `asana_id`/`linear_id` to each roadmap item, ensure IDs match. This is error-prone and invisible to new users. The existing `lf ops pm export` can auto-create a project, but there's no way to go the other direction (PM project → wave), link existing pairs, or see what's connected.

Wave goal: "Bootstrap/link/status commands create or connect Asana and Linear projects without manual YAML or frontmatter edits."

## Approach

Extend `lf ops pm` with three new subcommands: `init`, `link`, `status`. Provider is specified via `--provider` flag, defaulting to `pm.provider` from repo config.

```bash
# Create PM project from existing wave
lf ops pm init --wave scale                    # uses pm.provider default
lf ops pm init --wave scale --provider linear  # explicit provider

# Create wave from existing PM project
lf ops pm init --project 1234567890            # Asana GID
lf ops pm init --project PROJ-123 --provider linear

# Link existing wave ↔ project
lf ops pm link --wave scale --project 1234567890

# Show sync state
lf ops pm status                               # all linked waves
lf ops pm status --wave scale                  # one wave
```

This keeps one command surface instead of adding `OpsCommand::Asana` and `OpsCommand::Linear` variants. Provider flag is only required when `pm.provider` isn't set in config.

### `init --wave` (wave → PM project)

1. Validate: wave exists, no PM link yet for this provider (fail with "use `lf ops pm link` to reconnect")
2. Validate: provider config present (`asana.workspace` for Asana, `linear.team` for Linear) — fail with actionable message
3. Read `wave/<name>/README.md` — extract wave name as project name, vision section as description
4. `provider.create_project(name, description)`
5. For each `NN-*.md` roadmap item: `provider.create_item(project_id, item)`
6. Write `pm` block to wave YAML via `write_pm_project_to_wave_yaml`
7. Write provider ID to each item via `RoadmapItemDocument::set_id(provider, id)`
8. Commit

### `init --project` (PM project → wave)

1. Validate: no wave with the slugified project name exists (or accept `--wave` to override name)
2. `provider.list_items(project_id)` — fail clearly if project not found
3. Create `wave/<name>/` directory
4. Write wave YAML with `pm` block and minimal defaults (`flow: ship-wave`)
5. Write `README.md` scaffold from project name/description
6. Write roadmap items as `NN-slug.md` with provider ID in frontmatter
7. Commit

### `link` (wire up existing wave ↔ project)

1. Validate: both wave and project exist
2. Write `pm` block to wave YAML
3. `provider.list_items(project_id)` to get remote items
4. Match local items to remote by normalized title (case-insensitive, whitespace-collapsed)
5. Write provider IDs where matches found
6. Print matched/unmatched summary — no failure on partial matches

### `status`

For each wave with a `pm` block (or the specified `--wave`):
- Wave name, provider, project ID
- Item counts: local total, linked (have provider ID), unlinked
- Remote item count (one API call per wave)
- Any items that exist only remotely

### Code organization

All new logic lives in `rust/loopflow/src/ops/pm.rs` alongside existing import/export/sync. New types:

```rust
pub struct PmInitOptions {
    pub wave: Option<String>,      // --wave: init from wave
    pub project: Option<String>,   // --project: init from PM project
    pub provider: Option<PmProviderKind>,  // --provider: override default
}

pub struct PmLinkOptions {
    pub wave: String,
    pub project: String,
    pub provider: Option<PmProviderKind>,
}

pub struct PmStatusResult {
    pub wave: String,
    pub provider: PmProviderKind,
    pub project_id: String,
    pub local_total: usize,
    pub linked: usize,
    pub unlinked: usize,
    pub remote_only: usize,
}
```

Provider resolution for init/link: check explicit `--provider` flag first, then fall back to `pm.provider` in config. Error if neither is set: "Specify --provider or set pm.provider in .lf/config.yaml".

### CLI shape

Add to `PmCommand` enum:

```rust
pub enum PmCommand {
    Import { wave: Option<String> },
    Export { wave: Option<String>, dry_run: bool },
    Sync { wave: Option<String> },
    Init {
        #[arg(short = 'w', long = "wave")]
        wave: Option<String>,
        #[arg(long = "project")]
        project: Option<String>,
        #[arg(long = "provider")]
        provider: Option<String>,
    },
    Link {
        #[arg(short = 'w', long = "wave")]
        wave: String,
        #[arg(long = "project")]
        project: String,
        #[arg(long = "provider")]
        provider: Option<String>,
    },
    Status {
        #[arg(short = 'w', long = "wave")]
        wave: Option<String>,
    },
}
```

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Provider-specific top-level commands (`lf ops asana init`) | Very explicit about which provider | Adds 2 OpsCommand variants with duplicate subcommand trees. Doesn't scale. The provider is a parameter, not a command. |
| Fold init into existing `export` (auto-create already exists there) | Less surface area | Export's auto-create is a fallback, not an intentional bootstrap. Conflating "push my items" with "set up the project" makes both harder to reason about. `init --project` (reverse direction) doesn't fit export at all. |
| Config-only provider (no `--provider` flag) | Simpler CLI | Chicken-and-egg: you can't bootstrap if config isn't set yet. Multi-provider repos need explicit selection. |

## Key decisions

**`lf ops pm init` not `lf ops asana init`.** The wave item spec calls for provider-specific commands, but the provider is a parameter of the operation, not a different operation. One `init` with `--provider` is simpler, consistent with `lf ops pm import/export/sync`, and doesn't require duplicating the subcommand tree for each new provider. The flag defaults to `pm.provider` from config so the common case is still short: `lf ops pm init --wave scale`.

**`init` is one command with mutually exclusive modes.** `--wave` and `--project` determine direction. Exactly one must be provided (error otherwise). This is cleaner than separate `init-from-wave` / `init-from-project` commands.

**`link` matches by title, not position.** Position-based matching is fragile when items have been reordered on either side. Normalized title comparison (lowercased, whitespace-collapsed) handles the common case. Unmatched items are reported but not errors — partial linking is useful.

**All commands auto-commit.** Consistent with the wave item spec and existing `lf ops` behavior. Uses `commit_workflow` for meaningful messages.

**`status` makes one API call per linked wave.** Calls `provider.list_items(project_id)` to get remote count and detect remote-only items. Acceptable for the typical case (1-5 waves). No caching — status should always show current state.

## Scope

- In scope: `init`, `link`, `status` subcommands under `lf ops pm`. Wave YAML and roadmap frontmatter mutations. Auto-commit. Actionable error messages for missing config.
- Out of scope: `lf ops asana` / `lf ops linear` as top-level commands (use `--provider` instead). Real-time sync. Webhook integration. Status tracking beyond item counts. The existing `export` auto-create behavior remains unchanged.

## Done when

```bash
# From wave → PM project
lf ops pm init --wave test --provider asana
# Creates Asana project, writes pm block and asana_id frontmatter, commits

# From PM project → wave
lf ops pm init --project 1234567890 --provider asana
# Creates wave/test/ with README, YAML, roadmap items, commits

# Link existing
lf ops pm link --wave test --project 1234567890
# Writes pm block, matches items by title, writes IDs, commits

# Status
lf ops pm status
# Shows: wave name, provider, project ID, local/linked/unlinked/remote counts

# All commands fail clearly on missing config
lf ops pm init --wave test --provider asana
# → "asana.workspace not set. Run: lf ops auth configure asana"
```

Tests:
- `cargo test -p loopflow pm_init` — unit tests for each init direction against mock server
- `cargo test -p loopflow pm_link` — link matching logic
- `cargo test -p loopflow pm_status` — status output formatting
