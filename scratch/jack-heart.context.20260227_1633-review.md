# Area Doc Policy: Review

## What was implemented

`gather_area_docs()` now collects `.md` files from both ancestor directories (existing behavior) and descendant directories (new) when `-a` is used. A new `gather_area_descendants()` helper recursively walks subdirectories under the area path, deduplicating against the ancestor walk's `seen` set.

Changes in one file: `rust/loopflow/src/engine/prompt.rs`.

## Key choices

| Decision | Why |
|----------|-----|
| New helper instead of reusing `gather_md_files()` | `gather_md_files` strips paths relative to `dir.parent()` and has no dedup. Adding parameters would complicate the scratch docs caller for zero benefit. |
| Sort descendants by depth (shallow first) | `pop()` during budget trimming drops from the end — deepest files go first, which is the right priority. Alphabetical tiebreaker within same depth for determinism. |
| Cap at 100 descendants | Tokenizing hundreds of files just to drop most is wasteful. 100 is generous for any reasonable area. |
| Collect-then-truncate (not early exit) | Sorting by depth requires seeing all candidates. The budget trimmer provides a second safety layer. |

## How it fits together

The ancestor walk runs first (unchanged), populating `docs` and `seen`. The descendant walk appends to a separate `descendant_docs` Vec, which is sorted by depth, capped at 100, then extended onto `docs`. During budget trimming, `area_docs.pop()` removes descendants before ancestors (since descendants are at the end), and within descendants, deepest-first.

No changes to `gather_documents()`, `trim_context_with_breakdown()`, or `ContextBreakdown`.

## Risks and bottlenecks

- **Pathological monorepos**: A `-a src/` on a repo with 10,000 `.md` files under `src/` will read them all before capping at 100. Unlikely in practice — the budget trimmer handles any overflow — but worth noting.
- **Symlink loops**: `gather_area_descendants` follows symlinks (via `is_dir()`). A symlink cycle would cause infinite recursion. Low risk in real repos but not guarded against.

## What's not included

- No changes to `gather_md_files()` signature or behavior
- No changes to budget trimming priorities between document sources
- No depth cap on the recursive walk (the 100-file cap is sufficient)
- Wave item `wave/context/02-area-doc-policy.md` deleted — work is complete

## Test coverage

Three new tests:
1. `gather_area_docs_includes_ancestors_and_descendants` — verifies both ancestor and descendant `.md` files are collected, no duplicates
2. `gather_area_docs_excludes_sibling_directories` — verifies `src/web/` is not gathered when area is `src/api`
3. `gather_area_docs_caps_descendants_at_100` — creates 120 files, verifies only 100 descendants are included

Plus one label test: `format_prompt_with_area_docs_uses_ancestor_descendant_label`.
