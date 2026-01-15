# prland

Consolidated `lfpr` into `lfops`. Commands are now:
- `lfops pr create` (was `lfpr create`)
- `lfops land` (was `lfpr land`)
- `lfops commit` (was `lfpr commit`)

Also fixed `lfops land` to use `gh pr merge` so PRs show as merged (not closed) on GitHub. Added `--local` mode for landing without a PR, and `--create-pr` for creating and merging in one step.

## Design notes

**Config option:** `land: gh | local` in `.lf/config.yaml` controls default. Flag `--local/--gh` overrides at runtime.

**PR mode flow:** Requires existing PR (or use `--create-pr`). Uses `gh pr merge --squash --delete-branch` which marks PR as merged on GitHub and handles branch cleanup remotely.

**Local mode flow:** No PR required. Squashes commits, merges to main locally, pushes. Handles both pushed and unpushed branches by detecting whether `origin/<branch>` exists.

**Create-and-merge flow (`--create-pr`):** Creates a PR with generated title/body and immediately merges it. Combines `lfops pr create` + `lfops land` into one operation. Useful for quick feature lands where you want PR history without separate create/merge steps.
