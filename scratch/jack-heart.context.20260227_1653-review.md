# Review: Area Doc Descendants + Direction Defaults

## What was implemented

Two features on one branch:

1. **Area doc descendant gathering** — `-a src/api/` now collects `.md` files from ancestor directories (existing behavior) *and* recursively from descendant directories under the area path. Siblings and uncles are excluded. Descendant docs are sorted shallowest-first and capped at 100 for safety.

2. **Direction defaults** — Config-level `direction: [craft]` provides baseline directions without `-d`. `--no-direction` suppresses config defaults. `scale` removed from `craft` group (now standalone). `build.rs` enforces unique direction node names across groups and leaves.

## Key choices

**Separate `gather_area_descendants` function** rather than reusing `gather_md_files`. The existing `gather_md_files` computes relative paths from `dir.parent()`, which wouldn't produce correct repo-relative paths. The new function takes `repo_root` and a `seen` set for dedup against the ancestor walk. Three similar lines is better than a forced abstraction.

**100-doc safety cap** on descendants. The budget system already drops area docs first when over token budget, but this prevents pathological cases in large monorepos from even being collected.

**Additive direction merge, not override.** `-d ux` adds to config defaults; `--no-direction` provides the escape hatch. This matches the existing merge semantics.

**Long flag only for `--no-direction`.** Consistent with `--no-chrome`, `--no-lfdocs`, `--no-diff`, `--no-diff-files`. No short form.

**`scale` moved from `craft/` to standalone.** `craft` becomes `[care, clarity, simplicity]`. `scale` is still available as `-d scale` but isn't a universal default.

## How it fits together

Area docs: `gather_area_docs()` walks ancestors shallowest→deepest (existing), then calls `gather_area_descendants()` on the area directory. The `seen` set prevents double-counting the area dir's own `.md` files. Descendant docs are appended after ancestor docs, sorted by depth.

Direction defaults: `.lf/config.yaml` declares `direction: [craft]`. `prepare_launch_prompt()` merges config directions (when `include_config_directions` is true) with CLI-passed directions. `--no-direction` sets `include_config_directions: false`. `build.rs` checks for namespace collisions at compile time.

## Risks and bottlenecks

- **Large area subtrees**: A deep directory tree could produce many `.md` files. The 100-doc cap mitigates this, and the budget system provides a second safety net.
- **Filesystem walk ordering**: `gather_area_descendants` sorts entries by path for deterministic ordering, but different OSes may return `read_dir` entries in different orders before sorting. The sort makes this deterministic.

## What's not included

- Step frontmatter direction defaults (design doc explicitly dropped this)
- Direction aliases (future feature, out of scope)
- Descendant doc priority in `trim_context_with_breakdown()` — the design doc mentioned dropping deepest descendants first when trimming, but the current trimming drops all area docs as a block. The shallowest-first sort means the truncation at 100 does drop deepest first, which partially addresses this.
- Wave item `02-area-doc-policy` deleted (implemented). Wave item `03-direction-defaults` and `04-lfd-direction-aliases` also deleted (03 implemented, 04 explicitly out of scope).
