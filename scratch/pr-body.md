## Try it!

```bash
cargo test -p loopflow stop
swift test --package-path swift --filter AttemptFailurePresentationTests
uv run python scripts/test.py --all
```

The first command proves `lf stop` parsing, lifecycle routing, idempotence,
cleanup, and exec-door denial. The second exercises retrying, recovered,
retry-pending, and provenance mismatch states. The full runner now mirrors the
CI matrix, including Rust format/Clippy, Swift boundary checks, E2E smoke, and a
signed macOS test build.

For the product path, run `lf serve product`, open its WaveChat, and use Stop.
The pane returns to the existing not-running state while detached worker loops
continue. Stream a long turn, scroll upward, and new deltas no longer pull the
reader away from older content.

Latest local result: 53 Python tests; Rust format, Clippy, and nextest; 59
website tests with 3 skips; 302 Swift tests plus boundary checks; E2E smoke; and
the signed macOS build all passed.

## Intent

Repair the failures exposed by the 2026-07-10 WaveChat dogfood while carrying
forward the signed-test and release hardening from PR #849. A wave can now stop
cleanly from either CLI or Mac, replay and streaming stay readable, retry
receipts preserve what actually happened, and the Mac release path fails closed
instead of producing an unsigned or unnotarized user artifact.

## Assumptions

- The wave listener is loopback-only and its endpoint file shares the local
  trust boundary used by `lf serve`.
- Detached workers outlive the listener by design; stopping a wave must not
  become a broad cancellation command.
- A retry is proven only by matching invocation, step index, and iteration with
  a different body ID.
- Wispr Flow is the dictation surface, so the unused built-in voice stack and
  microphone permission should not ship.
- Production DMGs require a Developer ID Application identity and all three
  notarization credentials.

## Key decisions

- Keep cleanup in `run_listener`; `/stop`, the CLI, and the Mac only request it.
- Project attempt state at render time instead of mutating journal history or
  adding wire fields.
- Filter empty thoughts in Rust for new data and Swift for historical replay.
- Use signed `build-for-testing` as the stable CI gate; executing hosted UI
  tests remains explicit on a machine with Automation permission.
- Make `scripts/test.py --all` genuinely CI-equivalent, including lint and
  boundary checks and full Python scope.

## Not included

- Detached worker cancellation.
- Retry-related journal or DTO migrations.
- A replacement built-in voice implementation.
- Linear task closure: the installed `lf 0.10.1` hits the known GraphQL ID-type
  mismatch. Reconcile tasks after redeploying the fix already on `main`, using
  this PR's merged URL.
