# docs2: Documentation Consolidation

Consolidates `quick-fix.md` and `workflow.md` into `getting-started.md`, replaces GIF demos with static code blocks, and cleans up outdated assets.

## Review

**Verdict:** Ready to ship

The changes are clean and well-motivated. GIFs added cognitive load without proportional value—static code blocks are scannable, copy-pasteable, and don't require regeneration when CLI output changes.

Minor observation: `index.md` now links directly to `agents.md` as "Next" instead of `getting-started.md`. This makes sense given that the index page is now comprehensive enough to serve as the getting-started content, and agents is the natural next topic for users who want to go deeper.

## Design notes

**Why consolidate:** Two standalone pages (`quick-fix.md`, `workflow.md`) were thin wrappers around the same content now in `getting-started.md`. The consolidation removes duplication and gives users one clear entry point.

**Asset cleanup:** Deleted files include:
- `Makefile` (VHS demo generation)
- All `.tape` files (VHS scripts)
- All demo GIFs (debug, workflow, loops, triggers, context)
- Orphaned `docs/docs/` subdirectory with stale GIFs

**Navigation update:** `_config.yml` removes references to deleted pages and ensures `getting-started.md` appears in the header.
