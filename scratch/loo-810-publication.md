# LOO-810 publication checks · 2026-11-09

Base: `4848816f73ccbacd5f1eb26bff7f54814d7c0c8b` (current main at the check).
`lf sync --manual` / `lf sync --continue` completed as `fef21fd4d`.

- `cargo build -p loopflow --bin lf --jobs 4`: passed after conflict resolution.
- `uv run python scripts/check_migrations.py`: passed; one unmerged additive migration, `creation_receipts.sql`.
- `LF_BIN=$PWD/target/debug/lf uv run python website/dev.py sync-docs`: 33 routes regenerated.
- `uv run python scripts/test.py --reuse-passing --rust-filter 'test(server::) | test(engine::launch::) | test(ops::human_session::tests::) | test(store::sqlite::frontier_tests::) | test(store::sqlite::provider_transitions::) | test(store::sqlite::revisions::) | test(lf::tests::) | test(transport::) | test(task_create) | test(documented_commands::) | test(dto_fixtures::) | test(cli_discovery::) | test(agent_startup_tests::) | binary(lf) | binary(served_reads) | binary(transport_tests)'`: architecture and test inventory passed; Rust ran 416 tests, 414 passed, two failures below. Run cleared inherited `LF_*`, closed stdin, resolved `TMPDIR`, and put the branch CLI first on PATH. Log: `.lf/tmp/loo-810-publish-checks.log`.

All five required terminal cases passed: newer viewer input/geometry, production
61-second grace expiry, exact-owner refusal, a stalled bridge alongside healthy
terminal/control traffic, and stale-socket replacement. Creation tests passed
across lost replies and server restart; budget, schema, migration frontier,
provider-transition, documented-command and new Wave lifecycle parity checks
also passed.

The parity assertion stripped CLI history's final newline but compared it with
untrimmed server text. It now compares rendered history byte-for-byte, retaining
path-only delimiter normalization. The shell fixture inherited
`GHOSTTY_RESOURCES_DIR` from the installed Desktop; its isolated rerun must unset
that variable. `cargo nextest run -p loopflow --no-tests fail --no-fail-fast
--build-jobs 4 --test-threads 2 -E
'test(server_operations_match_their_lf_commands) |
test(desktop_shell_reconnects_without_replay_and_close_settles_its_jobs)'`, with
the network runner and isolated environment above: both passed.

- `cargo fmt` and `cargo clippy --all-targets --jobs 4 -- -D warnings`: passed
  after the final canonical `CreationGet` rename.
- `target/debug/lf server describe --json`: matches the checked-in schema.
- Website suite: 73 passed.
- The first full Swift run built the app and ran 789 tests, finding stale
  `--json` expectations, the receipt operation naming mismatch, and ordinary
  Work loading failing with `server capacity reached; retry shortly`. The
  command/name expectations are corrected. Desktop now backs off only for
  pre-dispatch admission refusals; dispatched errors and unknown outcomes
  are never replayed.
- `scripts/test_desktop.sh --jobs 4 --filter
  'ServerClientTests|WorkCLISubscriptionTests|ServerProtocolTests|DesktopHeadlessTests'`:
  45 tests in four suites passed. This includes concurrent conversation
  preparation, CLI/sidebar updates, and new refusal/replay checks. Log:
  `.lf/tmp/loo-810-swift-rerun.log`.
- `swift_test_limit.py → test_network.py --desktop → swift test --skip-build
  --disable-sandbox --package-path swift --no-parallel --jobs 4`: 791 tests
  in 96 suites passed (171.648 s). Log: `.lf/tmp/loo-810-swift-diagnostic.log`.
  One preceding full run exited during `defaultConfigurationLoadsAfterInitialAppearance`
  without an assertion or crash diagnosis; its isolated 21-test theme suite
  passed. The diagnostic full run used `SWIFT_BACKTRACE=enable=yes` and passed
  without a backtrace. No product cause is established for that exit.
- `uv run python scripts/check_swift_multiplatform_boundaries.py`: passed.

Two mounted-terminal tests from main were outside a display-required suite.
They now belong to `GhosttyTerminalMountTests` with `.requiresDisplay`, matching
AGENTS.md. The headless theme test remains enabled. This classification was
made while the diagnostic full run was using the preceding test binary.
`scripts/test_desktop.sh --jobs 4 --filter 'GhosttyTerminalMountTests|ThemeTests'`:
rebuilt successfully; 21 theme tests passed, both mounted-terminal tests skipped
as display diagnostics. Log: `.lf/tmp/loo-810-swift-display-boundary.log`.
`git diff --check`: passed. No server or watcher from these checks remains.

Physical Mac sleep/wake, mounted cold-app recovery and real-provider continuation
remain for the existing demo review. Publication grants no shipping approval.
