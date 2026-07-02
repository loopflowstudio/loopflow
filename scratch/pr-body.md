## Try it!

```bash
lf op release status
lf op release status --json
cargo test -p loopflow release_status
```

`lf op release status` still shows the target, latest tag, tagged release workflow, and GitHub Release state. It now also prints the most recent package verification and weekly release workflow status, including titles, URLs, and normalized issue labels when GitHub returns failed runs. `--json` exposes the same data with workflow run IDs for repair agents.

Validation run:

```bash
cargo fmt --check
cargo test -p loopflow release_status
cargo clippy -- -D warnings
cargo test -p loopflow
uv run pytest python/tests/
cargo test --all
uv run pytest tests/regression/test_orphaned_runs_reset_wave_status.py tests/regression/test_run_with_roadmap_item_on_pm_wave.py tests/regression/test_terminal_session_dto_exposes_tmux_name.py -v
cargo run --quiet -p loopflow --bin lf -- op release status --json
```

The live `--json` smoke reported package verification success and a weekly release failure with `failure_kind: "publish"`, which confirms the new repair signal is visible from the CLI.

## Intent

Close the first local feedback-loop gap for release automation by making failed nightly package verification or weekly release runs discoverable from Loopflow itself, not only from GitHub Actions history.

## Assumptions

The recurring release workflows are named `nightly-packages.yml` and `weekly-release.yml`, matching the repo's documented release automation. The GitHub CLI remains the release ops integration point and must be installed/authenticated for release status checks.

## Key decisions

The release status command keeps tagged release publication and recurring automation health as separate lines. Missing nightly/weekly workflow files are reported as `(not found)` for compatibility with repos that have not adopted the release automation skeleton, while unrelated GitHub errors still fail loudly. Failed package verification and publish workflows now carry explicit failure kinds instead of relying on humans or agents to infer category from workflow names.

## Not included

This does not create wave items, issues, or repair PRs from failed workflow runs. It also does not add a daemon/API status surface or deploy-host freshness checks.
