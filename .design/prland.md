# prland

Consolidated `lfpr` into `lfops`. Commands are now:
- `lfops pr` - unified create/update/view
- `lfops land` - squash-merge to main
- `lfops commit` - commit with generated message

## Design notes

**`lfops pr` (unified):** Single command that creates a PR if none exists, updates the title/body if one does, and always opens it in the browser. Idempotent - run it any time to sync PR state with current branch.

**Config option:** `land: gh | local` in `.lf/config.yaml` controls default. Flag `--local/--gh` overrides at runtime.

**PR mode flow:** Requires existing PR (or use `--create-pr`). Uses `gh pr merge --squash --delete-branch` which marks PR as merged on GitHub and handles branch cleanup remotely.

**Local mode flow:** No PR required. Squashes commits, merges to main locally, pushes. Handles both pushed and unpushed branches by detecting whether `origin/<branch>` exists.

**Create-and-merge flow (`--create-pr`):** Creates a PR with generated title/body and immediately merges it. Combines `lfops pr` + `lfops land` into one operation. Useful for quick feature lands where you want PR history without separate create/merge steps.

**Default behavior:** `lfops land` auto-stages, commits, and pushes any uncommitted or unpushed changes. Use `--strict` to error instead if the branch doesn't match remote (for CI or when you want to verify clean state).
