# Desktop cutover contribution handoff — November 4, 2026

The LOO-582 conversation contributed the full FFI deletion to LOO-593 at Jack
Heart's request. At 21:50 UTC the Task's primary conversation launched Graph
`5547bd41-270d-4796-af61-46f3aa546384` to finish and publish that existing diff.
Those observations are historical: that Graph later stopped on a usage-limit
failure. Jack Heart authorized continuation, and the same Task's feature Graph
subsequently checkpointed direct writes and merged main. The current plan is
[mobile-backend.md](mobile-backend.md); this handoff retains subprocess-era
verification and failures, not current execution status.

## Verification already completed on the deletion

- `cargo test -p loopflow --test served_reads`: passed; includes all shared
  operations, subprocess-era Process/effect parity and a 72 KiB Task-save stdin payload.
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
Gate owns `uv run python scripts/test.py --loopflow` with its normal
resource/security pressure guard on the completed implementation. Prior Xcode host-pressure
failure remains history, not a current-build result.

Main's #1815 (`dfd454cfc783b5ff3014d5a398ccf1627b427d11`) is integrated by
merge `59287ba3ed` after checkpoint `ced68911ee`; the previously observed base
conflict is resolved locally. The subsequent `ee4eee8dd8` merge integrates main
through `6f0d98e016`, including #1818/#1820, without restoring FFI. Publication
and final affected verification remain, covering complete deletion and accepted
direct writes with their latency/verification limits. No merge or Auto-Ship approval.

## Full-run termination

The original full headless run subsequently stopped at its 300-second watchdog.
The event stream named `WorkObservationTransportTests.writeFromAsyncCaller`;
the visible log was at Unified Work and Session navigation. Do not infer a clean
full pass from focused passes. Exact final output is in
`/tmp/loo593-desktop-headless.log`. The fixture initialization failure above and
this timeout both predate the direct-server-write implementation. The new direct
path needs its own focused and full consumer verification.
