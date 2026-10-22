# Open questions / assumptions (LOO-565 kickoff, 2026-10-21)

- Assumed: worktrees created before this lands get no marker backfill. They prompt once; a single "Yes" in any of them records the main-checkout key, which trusts every sibling. Reversible by adding a backfill from Task checkout rows later.
- Assumed: rewriting `~/.claude.json` through serde_json may reorder its keys (no `preserve_order`). Claude rewrites the whole file on every save, so order carries no meaning. Values are unchanged.
