# LOO-562 assumptions and open questions

- **The theme test never hung (2026-10-21).** The brief named `ThemeWorkflowTests`
  because `output.log` lags: Swift Testing's stdout through `tee` is block
  buffered. The event streams of all four failed runs (39683398012, 39685221622,
  39682537781 density; 39684412294 lifecycle) show the theme test passing in
  1.7 s and a `BuiltWorld` test never ending. Fixed the world path instead.
- **Two causes, both in `BuiltWorld` tests.** (1) Slowness: every world write
  ran a full Git planning exchange (fetch, fetch, save, push); density took
  506 s on CI even when green. (2) A real hang: `BuiltWorld.run` called
  `Process.waitUntilExit()` from a detached Task; reproduced locally, stuck
  after the child exited and both pipes hit EOF. `BuiltLf`/`World` (#1619)
  already waits on `terminationHandler`; `BuiltWorld` was its predecessor and
  is deleted.
- **World builds defer the Git exchange** (process-wide flag in
  `planning_sync`, set by `World::create`, published once in `finish`).
  Assumption: a world's origin has no other writer, so acquiring mid-build
  imports nothing and one publication yields the same remote. The Rust recipe
  test asserts the publication is confirmed.
- **Runner-side limit, not `.timeLimit`.** A scratch package proved
  `.timeLimit(.minutes(1))` records the issue at 60 s but the run still waits
  600 s for a test blocked on a child pipe. `scripts/swift_test_limit.py` reads
  the event stream and stops the process group at 300 s per test function,
  naming it. No Swift trait added, to keep one authority.
- **Path filter scope.** Applied to `swift-test` and `loopflow-ui-test` only.
  `macos-process-test` (Rust-only, macOS) is also unaffected by docs/website/
  Python changes; left running — candidate follow-up.
- **Product cost, not fixed here.** A real `lf task create` pays the same
  per-save Git exchange (~1 s locally against a local bare origin; more over
  the network). Candidate Task for Reliability.
