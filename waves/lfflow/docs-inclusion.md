# Always include docs/

`docs/` should be in context by default so agents know what documentation exists and can update it when behavior changes.

**Problem**: When working in an area, agents don't see `docs/` and forget to update user-facing documentation. The responsibility exists (CLAUDE.md says "update docs when you change code") but the context doesn't support it.

**Solution**: Add `docs/` to the "always include" list alongside `scratch/`, `roadmap/`, and root `.md` files. Token-budget it like other reference docs.

**Effort**: Low (add to default context list in config)
