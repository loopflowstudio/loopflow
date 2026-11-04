# Desktop cutover contribution handoff — November 4, 2026

The LOO-582 conversation contributed the full FFI deletion to LOO-593 at Jack
Heart's request. At 21:50 UTC the Task's primary conversation launched Graph
`5547bd41-270d-4796-af61-46f3aa546384` to finish and publish that existing diff.
Fresh status, graph show and ps confirmed live Graph PID 29444 and implement
Process PID 29986. The contributing conversation leaves that Graph in control:
no concurrent commit, sync, new Graph, or publication.

## Verification already completed on the deletion

- `cargo test -p loopflow --test served_reads`: passed; includes all shared
  operations, Process/effect parity and a 72 KiB Task-save stdin payload.
  Log `/tmp/loo593-rust-cutover.log`.
- Journal library tests: 30 passed. Server library tests: 3 passed. Write
  mapping tests: 2 passed. `cargo clippy --all-targets -- -D warnings` passed.
  Log `/tmp/loo593-final-rust.log`. `cargo fmt` and diff whitespace checks passed.
- Transport server group: 13 passed, 2 fixture/closure failures repaired,
  2 opt-in service tests skipped. The focused pane-closure rerun passed all
  3 tests; `/tmp/loo593-closure-tests.log`. Initial evidence remains in
  `/tmp/loo593-server-transport.log`.
- Python install/release/dev/input checks: 49 passed;
  `/tmp/loo593-python-cutover.log`.
- SwiftPM app and tests compile without the removed Loopflow library. Focused
  ServerClient, protocol, RegistryQuery, opening and Session-control tests
  passed. Work observation fixture initialization was repaired; all 8 tests
  passed in `/tmp/loo593-swift-watch.log`.
- Configured native demo, retained draft and identity evidence:
  `scratch/server-cutover-demo.md`. Measured before/after performance:
  `scratch/server-performance/results.md`, with raw samples. The final isolated
  benchmark passed, `/tmp/loo593-perf-isolated.log`.

## Remaining verification and known failure

The contributor's full Desktop run, `/tmp/loo593-desktop-headless.log`,
failed on fresh-Machine Portfolio initialization and later timed out during
Work observation shutdown. Implement repaired Portfolio setup and reproduced
the shutdown stall: Foundation's `waitUntilExit()` remained blocked after the
child exited (`/tmp/loo593-swift-sample.txt`). The fixture now awaits its
preinstalled termination handler. The focused rerun covers both repairs;
the full suite remains gate's responsibility. The original failed logs remain.

The separate Xcode build-for-testing has not been run for this deletion.
Run `uv run python scripts/test.py --loopflow` with its normal resource/security
pressure guard after sync; do not bypass that guard. Prior Xcode host-pressure
failure remains history, not a current-build result.

Main advanced to dfd454cfc783b5ff3014d5a398ccf1627b427d11 (#1815). PR #1813
was CONFLICTING at inspection. The Graph owns checkpoint/sync/conflict repair,
final affected verification and publication. Preserve upstream updater behavior
through the server architecture. The existing PR body describes the former
partial cutover and needs replacement with the complete deletion scope and
honest latency/verification limits. No merge or Auto-Ship approval.

## Full-run termination

The original full headless run subsequently stopped at its 300-second watchdog.
The event stream named `WorkObservationTransportTests.writeFromAsyncCaller`;
the visible log was at Unified Work and Session navigation. Do not infer a clean
full pass from focused passes. Exact final output is in
`/tmp/loo593-desktop-headless.log`. The fixture initialization failure above and
this timeout both predate the direct-server-write implementation. The new direct
path needs its own focused and full consumer verification.
