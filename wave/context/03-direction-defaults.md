# 03: Direction Defaults

**Finish line:** `lf implement` in this repo shows directions in the audit without passing `-d`.

Make directions flow from config so you get sensible defaults automatically.

## Current state

Config supports `direction:` key in `.lf/config.yaml`, and `include_config_directions: true` enables merging them in. But this repo doesn't set any default directions — the config has no `direction:` key.

Steps can declare `directions:` in their frontmatter, and some builtin steps do. But most don't.

Result: most `lf` invocations show "direction 0 —" in the audit. Directions are powerful but invisible in the default workflow.

## Approach

### Repo-level defaults

Add `direction:` to this repo's `.lf/config.yaml`:

```yaml
direction:
  - craft
```

`craft` expands to `[care, clarity, scale, simplicity]` — a good baseline for a codebase that values quality. This applies to all steps unless overridden by `-d`.

### Step-level defaults

Review builtin steps that don't declare directions and add sensible defaults where the step has a natural perspective:

- `implement` → `craft` (quality implementation)
- `review` → `craft` (quality review)
- `gate` → `craft` (quality gate)
- `debug` → none (debugging is focused, directions add noise)
- `design` → none (interactive, user brings the perspective)

Step directions merge with config directions (deduped), so a step declaring `craft` when config also has `craft` doesn't double up.

### CLI override

`-d` on the CLI replaces config defaults (current behavior). Add `-D` or `--no-direction` to explicitly suppress config defaults when you want a clean slate.

## Changes

**`.lf/config.yaml`** (this repo):
- Add `direction: [craft]`

**Builtin step frontmatter** (selective):
- Add `directions:` to steps where a default perspective makes sense

**`rust/loopflow/src/engine/launch.rs`**:
- `prepare_launch_prompt()` already merges config directions. Verify `include_config_directions` defaults to `true` (it should, but confirm).

**`rust/loopflow/src/lf/commands/run.rs`**:
- Add `--no-direction` flag to suppress config/step defaults.

## Done when

- `lf implement` in this repo shows "direction 1,200 craft" (or similar) in the audit without `-d`
- `lf implement -d ux` overrides the default
- `lf implement --no-direction` shows "direction 0 —"
- Step frontmatter defaults merge correctly with config defaults
