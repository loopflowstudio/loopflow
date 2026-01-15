# prland

Fix `lfpr land` to use `gh pr merge` so PRs show as merged (not closed) on GitHub. Add `--local` mode for landing without a PR.

## Review

**Verdict:** Ready to ship

The implementation is clean and focused. Two minor observations but nothing blocking:

1. **`has_design_artifacts` imported but unused in `lfpr.py:13`** - The `_land_pr` and `_land_local` functions both call `clear_design_artifacts` directly without checking first. The import could be removed, but it's harmless.

2. **Local mode squashes twice** - `_land_local` calls `_squash_commits` on the feature branch, then does `git merge --squash` into main. The second squash is redundant since the branch already has one commit. Works correctly, just wasteful git operations.

## Design notes

**Config option:** `land: gh | local` in `.lf/config.yaml` controls default. Flag `--local/--gh` overrides at runtime.

**PR mode flow:** Requires existing PR. Uses `gh pr merge --squash --delete-branch` which marks PR as merged on GitHub and handles branch cleanup remotely.

**Local mode flow:** No PR required. Squashes commits, merges to main locally, pushes. Handles both pushed and unpushed branches by detecting whether `origin/<branch>` exists.

**Removed complexity:**
- `--force`, `--no-pr`, `--require-clean-design`, `--base` flags
- Worktrunk (`wt`) dependency from local mode
- Separate CLI modules consolidated into `lfpr.py`

## Future

Auto-rebase on merge conflict using LLM is a potential follow-up (captured in questions.md).
