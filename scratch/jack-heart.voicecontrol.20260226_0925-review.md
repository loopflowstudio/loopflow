# Voice Control Stage 1 Review

## What was implemented
- Added `VoiceInputService` in `LoopflowCore` to manage microphone permission, WhisperKit model prep/download, streaming partial transcripts, final transcript assembly, and cancellation.
- Added `VoiceInputButton` in `Concerto` and integrated it into `WaveSessionView` composer with tap-to-toggle and press-and-hold recording.
- Added inline voice feedback UI in the composer area: partial transcript preview, transcribing/model-download status row, and denied-permission notice with settings shortcut.
- Added microphone usage/entitlement wiring (`Info.plist`, `Concerto.entitlements`) and WhisperKit dependency wiring in both SwiftPM and XcodeGen specs.
- Added `VoiceInputServiceTests` covering start/stop flow, denied permission, cancel behavior, partial fallback, and restart-after-cancel behavior.

## Key choices
- Kept speech-to-text fully on-device via WhisperKit `tiny` for low-latency, privacy-preserving push-to-talk.
- Kept voice state orchestration in `LoopflowCore` so UI remains a thin view layer and logic is unit-testable.
- Used explicit UI states (`idle`, `recording`, `transcribing`) to keep button visuals and feedback deterministic.
- Added a defensive pre-start engine cancel in `startRecording()` to prevent stale streaming sessions from suppressing a new recording start after rapid cancel/restart.
- Pinned WhisperKit to the 0.9 minor line (`upToNextMinor` / `minorVersion`) so 0.x breaking shifts do not silently destabilize builds.

## How it fits together
`WaveSessionView` owns `VoiceInputService` state and renders `VoiceInputButton` + feedback rows from that state. The button drives service start/stop/cancel APIs, and final transcript output is inserted into the composer text field for manual edit/send. `VoiceInputService` bridges platform permission checks and WhisperKit engine streaming/finalization, exposing only UI-safe observable state.

## Risks and bottlenecks
- First-run model download is still network-dependent and can feel slow on poor connections.
- Voice quality/latency depends on device performance and ambient noise.
- Full `xcodebuild test` remains flaky locally due `ConcertoUITests-Runner` early exit (`signal kill`) before bootstrapping; non-UI tests pass.
- WhisperKit remains a fast-moving dependency; minor-line pinning reduces but does not eliminate upgrade risk.

## What's not included
- Stage 2/3 wave items: VAD auto-stop and auto-send.
- Any change to send semantics (voice still inserts text; user manually sends).
- Transcript post-processing beyond whitespace normalization (no punctuation cleanup, no command intent parsing).
- UI test harness stabilization for the existing `ConcertoUITests-Runner` bootstrap failure.
