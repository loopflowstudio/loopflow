# 03: Direction Defaults

**Finish line:** `lf implement` in this repo shows "direction ~900 care, clarity, simplicity" in the audit without passing `-d`.

## Problem

Directions are loopflow's most powerful quality lever but they're invisible by default. Most invocations show "direction 0 —" in the audit because config doesn't set defaults. Users don't know directions exist until someone tells them to pass `-d`.

Fixing this benefits everyone using loopflow: agents get consistent quality signals, users see directions in the audit and start thinking about what perspectives matter for their repo.

## Approach

Two changes: a config default and a suppression flag.

### 1. Config default: `direction: [craft]`

Add to this repo's `.lf/config.yaml`:

```yaml
direction:
  - craft
```

`craft` expands to `[care, clarity, simplicity]`. This applies to every step in the repo. Other repos can set `direction: [ux]`, `direction: [security]`, or whatever fits their domain.

The merge pipeline in `prepare_launch_prompt()` already handles this — config directions are the base, CLI directions append (deduped). No engine changes needed for this layer.

### 2. `--no-direction` flag

Add `--no-direction` to suppress config defaults. Only CLI-passed `-d` values apply.

```bash
lf implement                       # gets craft from config → care, clarity, simplicity
lf implement -d ux                 # gets craft + ux (additive)
lf implement --no-direction        # gets nothing
lf implement --no-direction -d ux  # gets only ux
```

This matches the existing `--no-chrome`, `--no-lfdocs`, `--no-diff` pattern.

### 3. Remove `scale` from `craft`

Delete `rust/loopflow/src/engine/builtins/directions/craft/scale.md`. The `craft` group becomes `[care, clarity, simplicity]`. `scale` is a valid direction but not a universal default — repos that need it can add `-d scale` or `direction: [craft, scale]` in config.

### 4. Direction namespace collision detection

All node names (groups and leaves) share one flat namespace. `-d craft` and `-d care` must both resolve unambiguously. Add a check in `build.rs` that fails the build if any two nodes (group directories or leaf file stems) share a name.

Today this is true by accident. Enforcement prevents silent data loss from HashMap overwrites.

## Design decisions from review

**No step frontmatter defaults.** The original design proposed adding `directions: [craft]` to implement, review, gate, and compress steps. Dropped — config-level is sufficient. If a step needs specific guidance, put the text directly in the step prompt rather than loading directions via frontmatter.

**No `include_step_directions` field.** Without step defaults, `--no-direction` only needs to control the existing `include_config_directions` field. No new field on `LaunchPromptInput`.

**Aliases are out of scope.** Direction aliases (saved shorthand for direction combos) are a user-level concept, not repo-level. They'd live in `~/.lf/config.yaml` or repo `.lf/config.yaml` and `lfd` would merge them. Future feature.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Step defaults + config defaults | Safety net for repos without config | Conceptual overhead of two defaults in two places; config is enough |
| `-d` replaces config instead of merging | Simpler mental model | Breaking change; additive merge is more useful — you usually want the baseline *plus* something |
| `craft` includes `scale` | Broader default | `scale` (horizontal scaling, stateless design, async patterns) isn't universally applicable; small projects don't need it |

## Key decisions

**Additive, not override.** `-d ux` adds ux to the config baseline, not replaces it. This is the current behavior and it's the right one. To get a clean slate, use `--no-direction`.

**Long flag only.** No short form for `--no-direction`. The toggle flags (`--no-chrome`, `--no-lfdocs`, etc.) don't have short forms either.

**All direction node names are globally unique.** Groups and leaves share one namespace. `build.rs` enforces this at compile time for builtins. For user repos, first match wins (alphabetical directory scan order). Users who want collisions to resolve predictably should use absolute paths: `-d craft/care` instead of `-d care`.

## Scope

In scope:
- Remove `scale` from `craft` group
- Add `direction: [craft]` to `.lf/config.yaml`
- Add `--no-direction` CLI flag wired to existing `include_config_directions`
- Add collision detection in `build.rs`
- Fix tests that assert `scale` is in `craft`

Out of scope:
- Step frontmatter direction defaults
- Direction aliases (user-level or repo-level)
- Changing merge semantics (additive stays additive)

### User-repo collision resolution

For user-defined directions in `.lf/directions/`, collisions are allowed but resolved deterministically: **first match wins** in alphabetical directory scan order. Users who want unambiguous resolution should use qualified paths: `-d craft/care` instead of `-d care`.

`validate` can warn about collisions so users know they exist.

## Changes

**`rust/loopflow/src/engine/builtins/directions/craft/scale.md`**:
- Delete

**`.lf/config.yaml`**:
- Add `direction: [craft]` after the `agent:` line

**`rust/loopflow/src/lf/mod.rs`**:
- Add `--no-direction` bool field to `Cli`

**`rust/loopflow/src/lf/commands/run.rs`**:
- Pass `include_config_directions: !cli.no_direction`

**`rust/loopflow/build.rs`**:
- After generating `BUILTIN_DIRECTION_GROUPS`, collect all node names (directory stems + file stems) and panic if any name appears more than once

**Tests**:
- Update `expand_direction_names_expands_builtin_craft_group` to not assert `scale`
- Update recursive group test if it depends on `scale`
- Add test for `--no-direction` suppression

## Done when

- `lf implement` in this repo shows "direction ~900 care, clarity, simplicity" in the audit
- `lf implement -d ux` shows the craft directions plus ux
- `lf implement --no-direction` shows "direction 0 —"
- `lf implement --no-direction -d ux` shows only ux
- `build.rs` fails if a duplicate node name is introduced
- `cargo test --all` passes
- `cargo clippy -- -D warnings` passes
