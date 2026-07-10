# Wave controls review

## What was implemented

- Added `lf stop <wave>` and a WaveChat Stop control. The CLI discovers the
  live listener, requests graceful shutdown, and waits for listener-owned
  resident, registry, endpoint, and token cleanup. Detached worker loops remain
  independent.
- Made WaveChat receipts truthful across retries. Failed body attempts retain
  their recorded reason and render as failed, retry pending, retrying, or
  recovered only when provenance proves the relationship.
- Dropped whitespace-only thought items before journaling and hid old empty
  thoughts during replay. Transcript streaming follows the reader only while
  they remain near the bottom.
- Hardened the Mac build and release path: signed test runners compile in CI,
  user DMGs require Developer ID signing and notarization, and Keychain access
  fails non-interactively instead of opening an authentication prompt.
- Removed the unused built-in voice stack, WhisperKit dependency, microphone
  declarations, and audio-input entitlement. Dictation remains an external
  Wispr Flow concern.
- Repaired `scripts/test.py` so its Rust suite includes format and Clippy, its
  Swift suite includes the multiplatform boundary check, and `--all` expands
  suite-internal scope instead of selecting every suite with narrowed tests.

## Key choices

- The listener remains the only shutdown owner. `/stop` is a lifecycle request,
  not a second cleanup implementation in the CLI or Mac app.
- Retry status is a Swift presentation over existing journal provenance keyed
  by invocation, step index, and iteration. No wire or journal format changed,
  and one attempt never rewrites another turn.
- Empty thoughts are handled at both boundaries: Rust keeps new journals clean;
  Swift keeps existing journals readable.
- The Mac uses the same `lf stop` verb as the terminal. The agent exec door
  rejects that verb so a worker cannot stop its steward wave.
- Signed `build-for-testing` is the deterministic CI gate. Launching hosted UI
  tests remains a host-permissioned action because macOS Automation can stop
  the runner before test bootstrap.

## How it fits together

WaveChat calls `LocalWaveAgentLauncher`, which resolves a lifecycle-capable
`lf` and invokes `lf stop`. Rust posts to the listener's local shutdown door;
`run_listener` then performs the same ordered cleanup used by graceful process
shutdown. Separately, streamed and replayed `ChatTurn` values feed a pure Swift
failure projection and visibility filter before `MessageRow` renders them.

## Risks and bottlenecks

- Stop depends on the loopback endpoint pointer and waits up to five seconds
  for this boot's pointer to disappear. A listener that accepts the request but
  wedges during cleanup returns a clear timeout error.
- The Mac requires an `lf` binary that exposes both `serve` and `stop`; stale
  binaries are rejected explicitly rather than interpreting an unknown verb as
  a skill.
- Scroll-follow behavior is compiled by the signed macOS gate and uses macOS 15
  scroll geometry APIs. Hosted UI-test execution was not attempted because it
  requires machine Automation permission.
- Production DMG creation now fails closed when Developer ID or notarization
  credentials are absent. This is intentional but removes the former local
  ad-hoc-DMG escape hatch.

## What's not included

- `lf stop` does not cancel detached worker loops.
- No journal migration or wire-model change was added for retry presentation.
- Built-in dictation is not replaced; Wispr Flow is the chosen input surface.
- Linear tasks were not reconciled. The installed `lf 0.10.1` sends obsolete
  GraphQL ID types and receives HTTP 400; the fix already exists on `main` and
  reconciliation should happen after redeploy with the merged PR URL.

## Validation

`uv run python scripts/test.py --all` passed the complete local CI matrix:

- Python: 53 passed.
- Rust: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
  and `cargo nextest run --all` passed.
- Website: 59 passed, 3 skipped.
- Swift: 302 passed; multiplatform boundary checks passed.
- E2E smoke: passed.
- Loopflow UI: signed `xcodebuild build-for-testing` passed, including signed
  unit and UI-test runners.

The signed build after voice cleanup showed only the development
`get-task-allow` entitlement; audio input is gone.
