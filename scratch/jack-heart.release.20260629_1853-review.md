# Gate Review

## What was implemented

`lf op release status` now reports the latest tagged release workflow plus the most recent nightly package verification and weekly release workflow runs. The command distinguishes the tagged release publication path from recurring release automation health, so a failed nightly or weekly job is visible from Loopflow without opening GitHub Actions first.

The release ops layer exposes `ReleaseWorkflowStatus` for the two automation checks, and the CLI prints status, conclusion, title, and URL when present. Missing nightly or weekly workflow files are tolerated as `(not found)` so older repos can still inspect release status.

## Key choices

- Reused `gh run list` instead of adding GitHub API plumbing. The release ops module already shells out to `gh`, and keeping the same dependency avoids a second auth/error model.
- Kept workflow names fixed to `nightly-packages.yml` and `weekly-release.yml`. Those names are the repo's documented release automation contract; target-specific release workflow lookup remains separate through release config.
- Tolerated missing automation workflows only for explicit workflow-not-found errors. Other GitHub failures still surface because they may indicate auth, repo, or network problems.

## How it fits together

`release_status()` resolves the repo and target, finds the latest target tag, checks the matching release workflow and GitHub Release, then asks `latest_workflow_status()` for the current package-verification and weekly-release runs. The CLI renders the existing target/tag/release lines first and appends the automation workflow summaries before the GitHub Release line.

## Risks and bottlenecks

- `lf op release status` now makes up to three `gh run list` calls plus one `gh release view` call when a tag exists. This is acceptable for a status command but depends on GitHub CLI latency.
- The two automation workflow filenames are intentionally conventional. Repos with different names will show `(not found)` until config grows a separate way to name those recurring workflows.
- The command reports latest workflow state only; it does not yet create attention items or repair PRs from failures.

## What's not included

- No automatic issue, wave item, or PR creation for failed release workflows.
- No release-status DTO or daemon API surface.
- No deploy/host freshness detection beyond the existing release workflow and GitHub Release checks.

## Validation

- `cargo fmt --check`
- `cargo test -p loopflow release_status`
- `cargo clippy -- -D warnings`
- `uv run pytest python/tests/`
- `cargo test --all`
