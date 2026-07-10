# Wave controls and truthful WaveChat failures

## Receipt

The 2026-07-10 product dogfood exposed four independent surface failures:

1. The running wave had no visible Stop control. `ShortcutAction.stopWave`
   existed, but no WaveChat view registered or rendered it, and the old
   `RepoState.stopWave` path terminates in an unsupported `WaveService` method.
2. Empty `ConversationItem::Thought` values were journaled and rendered as
   blank bordered cards.
3. Every streamed text update scrolled the transcript to the bottom, even
   after the reader deliberately scrolled back.
4. A recoverable body failure rendered as `Turn failed` with no reason. The
   exact journal receipt was a failed body for `wave_clarify` with
   `codex_error: Selected model is at capacity. Please try a different model.`,
   immediately followed by a different body for the same logical step which
   completed. The wave and step did not fail.

PR #849's nine-file signed-test/release hardening is the base commit in this
branch. PR #855's functional GUI-PATH work already exists on `main`; its closed
head contains only planning drift, so none of that duplicate history is
replayed here.

During implementation the product owner selected Wispr Flow as the dictation
layer on both Mac and iOS. Loopflow's unused built-in voice service, WhisperKit
package, tests, and microphone permission declarations therefore leave the
product instead of being carried into the new signed app build.

## Decision

### Stop is a wave lifecycle verb

Add `lf stop <wave>`. It discovers the live loopback listener through the same
`.wave-endpoint` used by `lf serve`, posts to a new `POST /stop` route, and
waits briefly for graceful shutdown. Missing or stale endpoints are an
idempotent success. The listener owns cleanup: stop the supervisor first,
terminate the resident, deregister the session, and remove only this boot's
endpoint and resident-token files. Detached worker loops remain independent;
the listener never owned their tmux sessions.

The Mac Stop button invokes that CLI verb through `LocalWaveAgentLauncher`, so
CLI and GUI share one implementation. Show the control beside Skip whenever
the WaveChat connection is live, require confirmation, disable it while the
request is running, and keep failures visible. The agent exec door denies
`stop`: a worker must not be able to tear down its steward wave.

### Empty thought records never become cards

Drop whitespace-only thoughts at the listener's shared turn-item boundary so
new journals stay clean. Also filter them in the shared Swift model so existing
journals replay without blank cards. Preserve non-empty thoughts and every
other item type.

### Follow only while the reader is at the bottom

Track whether the transcript is near its bottom from scroll geometry. New
turns and streaming text auto-follow only while that flag is true. Scrolling
back disables following; returning to the bottom enables it again. Initial
replay still starts in follow mode.

### Failed bodies are attempts, not failed waves

Keep runtime, journal, and wire unchanged. Derive presentation from existing
data with one surface-only identity:

```text
StepKey = (invocation_id, step_index, iteration)
```

For each body-backed failed assistant turn, retain the failure and exact
`termination_reason`. A later different body with the same key makes it
`retrying` while running and `recovered on retry` once complete. If the same
step remains selected with no active body and the loop is not failed, show
`retry pending`. Otherwise show `Attempt failed`. Bodyless failed turns retain
the neutral `Turn failed` fallback. Never infer terminal step or wave failure
from an attempt.

## Tests

- Rust CLI parsing and exec-door denial cover `lf stop`.
- The stop route signals graceful shutdown; the CLI is idempotent when no live
  endpoint exists.
- Runtime tests prove whitespace-only thoughts are dropped while real thoughts
  survive.
- Shared Swift tests cover hidden empty thoughts and attempt states across
  retrying, recovered, pending, different iterations, and different
  invocations.
- Launcher tests pin the exact `lf stop <wave>` command.
- Swift package tests and the signed macOS `xcodebuild build-for-testing` gate
  compile the visible control, transcript-follow behavior, and failure
  rendering. Executing UI tests remains an explicit host-permissioned action;
  macOS Automation can stop the runner before test bootstrap.

## Review

The lifecycle boundary stays singular: the Mac shells through `lf stop`, the
HTTP route only signals, and `run_listener` remains the sole cleanup owner. The
retry repair is a projection over existing provenance rather than a second
runtime state machine. The scroll fix has no timer or buffered-copy model; it
tracks only the reader's follow intent. The review removed the unused voice
stack instead of carrying a second dictation product, and reconciled PR #849's
signed UI gate with current CI by compiling signed test runners without
requiring hosted Automation permission.

## Done when

- A live WaveChat visibly offers Stop and stopping returns it to the existing
  not-running/Start state without `reset-waves`.
- Blank thought cards disappear for both new and replayed turns.
- Reading older content is never interrupted by streamed updates.
- The capacity receipt reads `Attempt failed · recovered on retry` and exposes
  the exact reason, while the successful retry remains its own turn.
- The final diff contains #849's hardening plus this one coherent product
  repair, with no duplicate #855 history.
