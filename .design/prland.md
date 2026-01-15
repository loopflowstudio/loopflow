# prland

Fix `lfpr land` to use `gh pr merge` so GitHub shows PRs as merged (not closed). Add `--local` mode for landing without a PR.

## Implementation

The `lfpr land` command now supports two modes:

**PR mode (default):** Uses `gh pr merge --squash --delete-branch` to merge via GitHub, which properly marks PRs as merged. Requires a PR to exist (run `lfpr create` first).

**Local mode (`--local` flag):** Squash-merges locally and pushes to origin. No PR required. No `wt` dependency.

Config option `land: local` in `.lf/config.yaml` changes the default to local mode.

## Changes

- `lfpr land` uses `gh pr merge` by default (marks PRs as merged on GitHub)
- `lfpr land --local` does local squash-merge + push (no PR needed)
- Removed flags: `--force`, `--no-pr`, `--require-clean-design`, `--base`
- Removed worktrunk dependency from local mode
- Config field `land: gh | local` controls default mode
- Deleted obsolete CLI modules (consolidated into `lfpr.py`)
