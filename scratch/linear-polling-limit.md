# Linear acquisition gate repair

Accepted design: `43bb3c1b13:scratch/linear-polling-limit.md`. Jack Heart's
`7aa36539` requests reproduction and repair of hosted run 38216116717 under
the network-denying CI runner before delivery.

The log contains five failures: three bulk-fixture refresh deadlines and both
Linux reconnect tests. The large fixture should finish setup before measuring
requests. Reconnect must use team deltas with current timestamps; an idle agent
waits for a turn boundary, while a watcher uses its 30–240-second cadence.
Late comments target live work because terminal Tasks are intentionally excluded.

The repaired fixtures pass. Review retained production deadlines and the single
acquisition owner; no production change was required. Evidence and delivery
limits are retained in Infrastructure memory. Remaining: hosted CI for the repaired
candidate. Live Linear counts, rejected-request quota
accounting and installed pending-write recovery remain unverified. After install,
restore `linear.team` with `planning.provider: linear` manually in loopflow,
etude, kata and fantasia before measuring. This gate does not change those checkouts.

Checks: disposable Linux `uv run python scripts/canonicalize_migrations.py 0.13.12 --materialize-for-tests`, then `cargo --config 'target."cfg(all())".runner = ["uv", "run", "--no-sync", "python", "/work/scripts/test_network.py"]' test -p loopflow` with `--lib linear_on_demand`, `--lib native_runner_is_quiet_until_its_turn_ends`, `--lib linear_rate_limit`, and `--test planning_reconnect_tests`, all with closed stdin: 17 passed; `cargo fmt --all -- --check`, `cargo clippy --all-targets --jobs 4 -- -D warnings`, `uv run ruff check tests/e2e/planning_reconnect.py`, `uv run ruff format --check tests/e2e/planning_reconnect.py`, `uv run python scripts/check_architecture.py`, and `git diff --check`: passed; hosted candidate matrix and live/installed acceptance remain unverified.
