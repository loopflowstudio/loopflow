# prland

Consolidated `lfpr` into `lfops`. Commands are now:
- `lfops pr create` (was `lfpr create`)
- `lfops land` (was `lfpr land`)
- `lfops commit` (was `lfpr commit`)

Also fixed `lfops land` to use `gh pr merge` so PRs show as merged (not closed) on GitHub. Added `--local` mode for landing without a PR.

## Design notes

**Config option:** `land: gh | local` in `.lf/config.yaml` controls default. Flag `--local/--gh` overrides at runtime.

**PR mode flow:** Requires existing PR. Uses `gh pr merge --squash --delete-branch` which marks PR as merged on GitHub and handles branch cleanup remotely.

**Local mode flow:** No PR required. Squashes commits, merges to main locally, pushes. Handles both pushed and unpushed branches by detecting whether `origin/<branch>` exists.
