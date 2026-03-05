# configs branch review

## What was implemented

Three changes to config handling, plus a large cleanup of stale project artifacts:

1. **Removed `push` and `include_loopflow_doc` config fields** from the `Config` struct, `Default` impl, and all tests. Both were parsed but never wired up. `push` was handled by `lf ops commit -p`, and `include_loopflow_doc` had no consumers.

2. **Updated init step to separate repo vs user config** guidance. The builtin `init.md` now:
   - Checks for `~/.lf/config.yaml` existence
   - Distinguishes repo config (team conventions: agent, harnesses, exclude) from user config (personal prefs: yolo, ide, chrome)
   - Offers to create `~/.lf/config.yaml` for personal preferences
   - No longer scaffolds `yolo: false`, `push: false`, or `context: "."` into repo config

3. **Improved agent error message** in `run.rs` to use `durable_log_dir()` for the log path hint instead of hardcoding `~/.lf/logs/`.

4. **Deleted ~13,000 lines of stale artifacts**: `.agents/skills/` (superseded by builtin steps), `proto/` (unused protobuf definitions), `reports/` (old research docs), `bin/` scripts, and `AGENTS.md` (consolidated into `CLAUDE.md`/`STYLE.md`).

5. **Added Goals and Documentation sections to STYLE.md** from the existing CLAUDE.md content (Clarity, Simplicity principles; documentation philosophy).

## Key choices

- **Repo-wins merge stays unchanged.** The design considered per-key precedence (user-preference keys winning over repo) but chose the simpler fix: just don't scaffold personal prefs in repo config. Standard repo-wins is the Git/VS Code convention.
- **No migration tooling.** Existing `.lf/config.yaml` files with `push:` or `include_loopflow_doc:` silently ignore the unknown fields (no `deny_unknown_fields`).

## How it fits together

Config loading (`config.rs`) merges global + repo YAML with repo winning for scalars and additive keys combining. The init step (`init.md`) guides what goes where. The design doc (`scratch/configs.md`) captures the full rationale.

## Risks and bottlenecks

- Existing repo configs that set `push: true` will silently lose that setting. Since it was never wired up, no behavior changes.
- The `.lf/steps/init.md` repo override needs to stay in sync with the builtin. The gate pass synced it.

## What's not included

- `-v`/`-vv` verbosity flags and `LF_LOG` env var (planned next per design doc)
- Default log level change from `info` to `warn`
- Config conflict logging
- Wiring up `pr`, `land`, `context`, `exclude` config fields to engine behavior
- `lf config` status command

## Gate fixes applied

- Removed `push: true` from `.lf/config.yaml` (the loopflow repo's own config)
- Removed `push` documentation from `docs/config.md`
- Removed `push: true` from bundled `LOOPFLOW.md` config example
- Updated all 7 golden test files to match
- Synced `.lf/steps/init.md` repo override with updated builtin
