# VAD Implementation Review

## What was implemented

Voice Activity Detection across both engines (WhisperKit and Apple Dictation), with a service-level state machine, button interaction update, and session-aware pause/resume.

**Protocol surface:** `VADEvent`, `VADSensitivity`, `startVADSession(sensitivity:onEvent:)`, `stopVADSession()` on `VoiceInputEngine`. Default extension throws `transcriptionUnavailable` for engines that don't support it.

**Service API:** `startListening()`, `stopListening()`, `pauseListening()`, `resumeListening()`, `cancelCurrentUtterance()`, `setVADSensitivity(_:)`. New `.listening` state. `InputMode` enum (`.pushToTalk` / `.voiceActivityDetection`).

**WhisperKit engine:** Uses `AudioStreamTranscriber` with `useVAD: true`. Energy monitoring via `bufferEnergy` for speech onset/offset detection. `VADActivityTracker` actor + `makeVADMonitorTask` free function handle the silence timer and utterance lifecycle.

**Apple engine:** Reuses `beginStreaming` for the transcription pipeline. VAD segmentation is driven by partial-transcript inactivity (not energy-based onset/offset — see "What's not included").

**VoiceInputButton:** Replaced `DragGesture` hold-to-record with `Button` + `LongPressGesture`. Long press enters VAD mode. Context menu for sensitivity presets. New `.listening` visual state with palette-rendered `mic.badge.waveform` and breathe animation.

**WaveSessionView:** Session-aware pause/resume wiring — pauses VAD when agent is running, resumes on completion.

**Tests:** 6 new tests covering VAD state transitions, pause/resume, sensitivity changes, stop, and cancel-current-utterance. `MockVoiceInputEngine` extended with `startVADSession`/`stopVADSession`/`emitVAD`.

## Key choices

**`transcriptHandler` callback pattern.** VAD utterances deliver transcripts asynchronously (not via return value like push-to-talk's `stopRecording()`). A stored `transcriptHandler` on the service is set/cleared by `VoiceInputButton.onAppear/onDisappear`. This avoids threading the callback through the engine protocol.

**`VADActivityTracker` as an actor.** Energy callbacks from WhisperKit fire on arbitrary threads. The tracker serializes speech detection state without blocking the audio pipeline.

**`makeVADMonitorTask` as a free function.** Shared between WhisperKit and Apple engines. Handles the silence-timer polling, finalization, and restart cycle.

**`dropNextVADUtterance` flag.** When VAD is paused mid-speech or an utterance is cancelled, the in-flight engine finalization still fires. This flag discards the stale result cleanly without engine-level coordination.

**`ignoreNextTap` in VoiceInputButton.** SwiftUI fires both the `LongPressGesture` and the `Button` action on long press due to `simultaneousGesture`. The flag prevents the tap handler from executing after a long press.

## How it fits together

```
User long-presses → VoiceInputButton.handleLongPress()
  → VoiceInputService.startListening()
    → engine.startVADSession(sensitivity:onEvent:)
      → engine monitors audio, fires VADEvents

VADEvent flow:
  .speechStarted → service.state = .recording
  .partial(text)  → service.partialTranscript updated
  .utteranceComplete(text) → transcriptHandler fires
                           → service.state = .listening
                           → cycle restarts

Turn state wiring (WaveSessionView):
  agent running → voiceService.pauseListening()
  agent done    → voiceService.resumeListening()
```

## Risks and bottlenecks

**Apple engine VAD quality.** The Apple path uses partial-transcript inactivity timing, not energy-based onset/offset detection. This means onset clipping is possible (no pre-roll ring buffer), and false-positive tuning is limited. Documented in `scratch/questions.md`. Follow-up pass needed.

**Silence timer polling.** `makeVADMonitorTask` polls every 120ms to check if the silence duration has elapsed. This is simple and correct but not zero-cost. In practice, the overhead is negligible compared to the audio pipeline.

**`setVADSensitivity` restarts the VAD session.** Changing sensitivity tears down and rebuilds the engine session. This introduces a brief gap (~100-200ms) where speech could be missed. Acceptable since sensitivity changes are infrequent user actions.

**`ignoreNextTap` edge case.** If `startListening` fails after a long press, the next tap is swallowed. User needs to tap again. Low severity — error message provides feedback.

## What's not included

- **Energy-based onset/offset detection for Apple engine** — deferred per `scratch/questions.md`
- **Pre-roll ring buffer** — not needed for WhisperKit (handled internally); deferred for Apple path
- **Auto-send after transcription** — wave item 02-auto-send
- **Hold-to-record gesture** — intentionally removed in favor of long-press-for-VAD
- **Custom sensitivity slider** — three presets only (quiet/normal/noisy)
