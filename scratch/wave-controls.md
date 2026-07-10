# Wave controls and truthful WaveChat failures

Four independent WaveChat surface failures from the 2026-07-10 dogfood, repaired
on top of PR #849's signed-test/release hardening. Design and rationale are
folded into `wave/product/MEMORY.md` ("Wave controls & truthful failures").

## Done when

- A live WaveChat visibly offers Stop, and stopping returns it to the existing
  not-running/Start state without `reset-waves`.
- Blank thought cards disappear for both new and replayed turns.
- Reading older content is never interrupted by streamed updates.
- The capacity receipt reads `Attempt failed · recovered on retry` and exposes
  the exact reason, while the successful retry remains its own turn.
- The final diff contains #849's hardening plus this one coherent product
  repair, with no duplicate #855 history.

## How to verify

- **`lf stop`** — Rust CLI parse + exec-door denial tests, plus the stop route:
  ```bash
  cargo test -p loopflow stop
  ```
  Idempotence: `request_stop` returns `false` (already stopped) with no live
  endpoint; a live listener shuts down and its endpoint + resident-token files
  are removed.
- **Empty thoughts** — runtime tests prove whitespace-only thoughts are dropped
  while real thoughts survive; shared Swift tests cover hidden empty thoughts.
- **Attempt states** — `AttemptFailurePresentationTests` cover retrying,
  recovered, retry-pending, different iterations, and different invocations.
- **Launcher** — `LocalWaveAgentLauncherTests` pin the exact `lf stop <wave>` argv.
- **Signed UI gate** — signed macOS `xcodebuild build-for-testing` compiles the
  visible control, transcript-follow, and failure rendering. Executing UI tests
  stays an explicit host-permissioned action (macOS Automation can stop the
  runner before test bootstrap).
