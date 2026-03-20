# import-pm, export-pm steps and pm-sync flow

## Problem

PM sync operations exist in Rust but aren't accessible as composable steps in flows. The WaveExecutor calls `pm_sync` at run start/end, but there's no way to:

- Run `lf import-pm` or `lf export-pm` as standalone steps
- Chain them into a `pm-sync` flow
- Use them in custom flows alongside other steps

Manual users and custom flows need the same PM operations the executor uses, exposed as first-class steps.

## Approach

Three deliverables, each small:

### 1. `lf ops pm export` — new ops command (Rust)

A one-directional push: local markdown wins, remote gets updated. Mirror of the existing `pm_pull` (remote wins).

```rust
pub fn pm_export(repo, options, progress) -> OpsResult<PmExportResult>
```

Logic:
1. Read wave YAML pm block, resolve provider + project
2. Fetch remote items for diffing
3. For each local item:
   - Has `id_for(provider)` and remote item exists → `update_item` if content differs
   - Has `id_for(provider)` but remote item gone → skip (don't recreate deleted items)
   - No provider ID → `create_item`, write ID back via `set_id`
4. Return counts of created/updated/skipped

**No ordering sync.** `PmTextUpdate` already filters rank-only updates. Asana needs relative moves, Linear's ordering API is undocumented. Document the limitation instead of faking it.

**No deletes.** Export doesn't archive/complete remote items missing locally. That's a destructive operation that belongs on explicit `complete_item` calls (item 06's lifecycle work), not on a routine push.

### 2. Builtin step definitions (markdown)

Two thin step files in `builtins/steps/ops/`:

**`import-pm.md`** — wraps `lf ops pm pull` (already exists):
```
lf ops pm pull [wave]
```

**`export-pm.md`** — wraps `lf ops pm export` (new):
```
lf ops pm export [wave]
```

Both are mechanical ops steps (no LLM reasoning needed). The agent reads the wave name from context and runs the command.

### 3. `pm-sync` flow (YAML)

```yaml
# builtins/flows/ops/pm-sync.yaml
- import-pm
- implement
- export-pm
```

This replaces the executor's direct `pm_sync` calls with composable steps. But the executor keeps calling `pm_sync` for its three-way merge — these steps are for manual/flow use.

### CLI wiring

Add `Export` variant to `PmCommand` enum, wire through `pm_cmd`:

```rust
PmCommand::Export { wave, wave_flag, all } => { ... }
```

Same flags as `Pull`: optional wave name, `--wave` flag form, `--all`.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Wrap `pm_sync` (three-way) in steps | Simpler, reuses existing code | Three-way merge requires git base — not appropriate for a one-directional "push my local state" operation. Steps should be predictable: import pulls, export pushes. |
| Add ordering sync to export | More complete remote state | Asana needs `insert_before`/`insert_after` (relative moves, no numeric rank). Linear's ordering is undocumented. Faking order sync creates false confidence. Document the limitation; revisit when providers expose it cleanly. |
| Export deletes remote items missing locally | Closer to "local is truth" | Too destructive for a routine operation. If someone deletes a local item, they probably want to archive/complete it explicitly, not have it vanish from Linear on the next export. Item 06's lifecycle completion handles the intended path. |
| Create `pm_push` instead of `pm_export` | Naming symmetry with `pm_pull` | The wave item and design docs already use "import/export" terminology. CLI already has `lf ops pm pull`; adding `export` as the complement reads naturally. |

## Key decisions

**Export is additive-only.** It creates and updates remote items but never deletes or completes them. This is intentional — destructive remote operations happen through lifecycle events (item 06), not bulk export.

**Import step wraps `pm_pull`, not `pm_import`.** The existing `pm_import` is team-level (imports entire projects as new waves). The existing `pm_pull` is wave-level (refreshes items in a wave). The step needs wave-level behavior.

**Steps are builtins, not `.lf/` overrides.** These are general-purpose operations that belong in every loopflow install. Ship as compiled builtins alongside `land`, `commit`, etc.

**`pm-sync` flow includes `implement`.** The flow isn't just import+export — it's the full cycle: pull latest from PM, do the work, push results back. This matches the design doc's intent and the executor's existing import→work→export pattern.

**Export diff uses content comparison, not change tracking.** Compare local body/title against remote item. If they match, skip the API call. No base-state diffing needed for a one-directional push — if local differs from remote, local wins.

## Scope

- In scope:
  - `pm_export` ops function in Rust
  - `PmCommand::Export` CLI variant
  - `import-pm.md` and `export-pm.md` builtin step definitions
  - `pm-sync.yaml` builtin flow definition
  - Tests for `pm_export` (unit tests with mock server, matching existing `pm_pull` test patterns)

- Out of scope:
  - Ordering sync (documented limitation)
  - Remote deletes/archival on export
  - Changing the executor's `pm_sync` calls (three-way merge stays for automated runs)
  - `ingest` PM refresh (item 05)
  - Item lifecycle comments/completion (item 06)
  - Notion provider

## Done when

```bash
# Export pushes local state to remote
lf ops pm export pm           # push one wave
lf ops pm export --all        # push all PM-enabled waves

# Steps are discoverable
lf import-pm                  # runs pm pull for current wave
lf export-pm                  # runs pm export for current wave

# Flow chains the full cycle
lf pm-sync                    # import → implement → export

# Verification
cargo test -p loopflow pm_export    # unit tests pass
cargo clippy -- -D warnings         # no warnings
```

## Measure

Before: `lf ops pm` subcommands are `init`, `import`, `sync`, `pull`, `status`. No step/flow wrappers.

After: `lf ops pm export` joins the CLI. `lf import-pm`, `lf export-pm` appear as steps. `lf pm-sync` appears as a flow. Round-trip test: create item locally without provider ID → `lf ops pm export` → item appears in Linear with ID written back to frontmatter.
