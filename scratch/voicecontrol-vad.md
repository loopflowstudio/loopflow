# Voice Activity Detection

## Problem

Voice input requires a button press. In flow, reaching for the mic button breaks concentration. Users who want hands-free operation — speaking naturally to their agent — have no path. VAD eliminates that friction: mic stays open, speech triggers transcription automatically, silence inserts the transcript. Push-to-talk remains the default; VAD is the power-user upgrade.

The wave vision: "faster than typing for someone in flow." Push-to-talk got us to parity. VAD gets us past it — you speak without context-switching to the UI at all.

## Approach

Engine-level VAD with a new protocol surface. Each engine uses its native VAD capabilities:

- **Apple path (macOS 26+/iOS 26+):** Energy-based VAD on the existing `AVAudioEngine` tap. When speech is detected, start a `SpeechAnalyzer` session with `DictationTranscriber`. When silence is detected, finalize the session. Restart monitoring.
- **WhisperKit path (older OS):** Set `useVAD: true` on `AudioStreamTranscriber`. Monitor `relativeEnergy` for speech onset/offset. Silence timer triggers finalization.

The service manages the high-level state machine (mode, session-aware pausing, UI state). The engine handles the audio-level details.

### Protocol extension

```swift
enum VADSensitivity: String, CaseIterable {
    case quiet    // low energy threshold, longer silence duration
    case normal   // balanced defaults
    case noisy    // high energy threshold, shorter silence duration
}

enum VADEvent: Sendable {
    case speechStarted
    case partial(String)
    case utteranceComplete(String)
}

protocol VoiceInputEngine: Sendable {
    // Existing push-to-talk surface (unchanged)
    func prepareModel(onProgress: @escaping @Sendable (Double?) -> Void) async throws
    func startStreaming(onPartial: @escaping @Sendable (String) -> Void) async throws
    func stopStreamingAndFinalizeTranscript() async throws -> String
    func cancelStreaming() async

    // VAD
    func startVADSession(
        sensitivity: VADSensitivity,
        onEvent: @escaping @Sendable (VADEvent) -> Void
    ) async throws
    func stopVADSession() async
}
```

Default protocol extension for `startVADSession`/`stopVADSession` throws `transcriptionUnavailable` — mock engines and future engines get a safe default.

### Service state machine

```swift
enum InputMode: Equatable {
    case pushToTalk
    case voiceActivityDetection
}

enum State: Equatable {
    case idle                 // no mic activity
    case listening            // VAD: mic open, waiting for speech
    case recording            // speech in progress (push-to-talk or VAD-triggered)
    case transcribing         // finalizing transcript
}
```

New public API:

```swift
// VAD mode
public private(set) var inputMode: InputMode = .pushToTalk
public private(set) var vadSensitivity: VADSensitivity = .normal

public func startListening() async throws    // enter VAD mode
public func stopListening()                   // exit VAD mode
public func setVADSensitivity(_ sensitivity: VADSensitivity)

// Session-aware pause/resume (caller-driven)
public func pauseListening()                  // mute VAD without teardown
public func resumeListening() async throws    // unmute VAD
```

State transitions:

```
Push-to-talk:
  idle → recording → transcribing → idle

VAD mode:
  idle → listening → recording → transcribing → listening → ...
                                                  ↓ (stopListening)
                                                 idle
```

In VAD mode, the service receives `VADEvent`s from the engine:
- `.speechStarted` → set state to `.recording`
- `.partial(text)` → update `partialTranscript`
- `.utteranceComplete(text)` → fire the `onTranscript` callback, set state back to `.listening`

### WhisperKit VAD implementation

```swift
func startVADSession(
    sensitivity: VADSensitivity,
    onEvent: @escaping @Sendable (VADEvent) -> Void
) async throws {
    // 1. Create AudioStreamTranscriber with useVAD: true
    // 2. Configure silenceThreshold from sensitivity preset
    // 3. Start streaming transcription
    // 4. Monitor relativeEnergy in the state callback:
    //    - Track whether speech is active (energy above threshold)
    //    - On transition from silent → speech: fire .speechStarted
    //    - On partial transcript updates: fire .partial(text)
    //    - On transition from speech → silent for silenceDuration:
    //      stop transcription, get final, fire .utteranceComplete(text)
    //      restart monitoring for next utterance
}
```

Sensitivity presets:

| Preset | Energy threshold | Silence duration | Use case |
|--------|-----------------|------------------|----------|
| quiet  | 0.08            | 2.0s             | Home office, quiet room |
| normal | 0.15            | 1.5s             | Some ambient noise |
| noisy  | 0.30            | 1.0s             | Coffee shop, open office |

### Apple VAD implementation

The Apple engine already has an `AVAudioEngine` tap (line 456). Extend it:

1. Install a parallel energy monitor on the audio tap — RMS calculation via `vDSP_rmsqv` from Accelerate.
2. Speech onset: energy exceeds threshold for 3+ consecutive frames (~300ms).
3. Speech active: start `SpeechAnalyzer` session with `DictationTranscriber`, feed buffered + live audio.
4. Speech offset: energy below threshold for `silenceDuration`.
5. Finalize session, fire `.utteranceComplete`, restart monitoring.

The key detail: buffer ~500ms of pre-onset audio so the transcriber doesn't miss the first syllable. Ring buffer of recent audio frames, fed to the analyzer when speech starts.

### VoiceInputButton update

**Interaction model:**
- **Tap** (idle): start push-to-talk recording (unchanged)
- **Tap** (recording): stop recording, insert transcript (unchanged)
- **Long press** (idle): enter VAD mode → `.listening`
- **Tap** (listening): exit VAD mode → `.idle`
- **Tap** (recording in VAD mode): cancel current utterance, stay in `.listening`

**Visual states:**

| State | SF Symbol | Color | Animation |
|-------|-----------|-------|-----------|
| idle | `mic` | textSecondary | none |
| listening | `mic.badge.waveform` | textSecondary with accent badge | subtle breathe (respects reduceMotion) |
| recording | `mic.fill` | accent (burgundy) | pulse (existing) |
| transcribing | `waveform` | statusInfo | none |

The `.listening` state needs to be visually distinct from both idle and recording — it signals "mic is open, I'm waiting." The badge provides that without being as attention-grabbing as the filled recording state.

**Context menu** (long-press when already in VAD mode, or secondary click):
- Sensitivity: Quiet room / Normal / Noisy (radio group)
- "Stop listening" action

### Session-aware activation

The caller (WaveSessionView or equivalent) manages VAD pausing based on session state:

```swift
.onChange(of: session.turnState) { _, newState in
    switch newState {
    case .running:
        voiceService.pauseListening()
    case .completed, .idle:
        Task { try? await voiceService.resumeListening() }
    case .failed:
        break
    }
}
```

`pauseListening()` mutes VAD detection (ignores energy levels) without tearing down the audio session. `resumeListening()` re-enables detection. This avoids the mic open/close latency on every turn.

### Audio pre-roll buffer

Both engines need a ring buffer of recent audio (~500ms) so that when VAD fires "speech started," the transcriber has the beginning of the utterance. Without this, the first word gets clipped.

Implementation: a simple circular buffer of `AVAudioPCMBuffer` frames. When speech onset fires, flush the buffer into the transcriber before switching to live audio feed.

For WhisperKit with `useVAD: true`, this is handled internally — the transcriber buffers audio and only transcribes voiced segments. No extra work needed.

For the Apple path, the audio tap already captures to a stream. Buffer the last N frames and yield them to the `SpeechAnalyzer` input stream when speech starts.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Shared AudioLevelMonitor (engine-agnostic VAD) | Single VAD implementation for both engines; simpler protocol | Requires coordinating two audio taps or sharing AVAudioEngine instances. WhisperKit's AudioProcessor manages its own capture — fighting it adds complexity. Each engine's native VAD is better tuned. |
| SpeechDetector module in Apple path | Apple's first-party VAD; potentially higher accuracy | SpeechDetector is oriented toward analyzing recorded audio regions, not real-time onset/offset detection for continuous monitoring. Energy-based VAD on the live tap is simpler, faster, and gives us direct control over sensitivity. Can revisit if SpeechDetector proves to have a streaming mode. |
| Timer-based heuristics (no energy monitoring) | Zero new audio code; just watch for gaps in partial transcripts | Unreliable — transcription gaps don't always mean silence. Background noise can produce empty partials. Energy monitoring is the standard approach for good reason. |
| Always-on transcription with post-hoc utterance splitting | Simpler lifecycle; no start/stop cycling | Higher CPU/battery. Apple's short dictation preset expects discrete utterances. WhisperKit accuracy degrades on very long continuous streams. |

## Key decisions

**Engine-level VAD, not service-level.** The engines have different audio capture stacks (WhisperKit's `AudioProcessor` vs our `AVAudioEngine` tap). Putting VAD in each engine avoids coordinating competing audio sessions. The protocol abstracts the difference cleanly.

**Long-press to enter VAD mode, not a three-way toggle.** A tap cycle (idle → PTT → VAD → idle) is confusing — users would accidentally enter VAD mode while trying to push-to-talk. Long-press is progressive disclosure: most users never discover it, power users find it natural. Matches iOS conventions (long-press for advanced options).

**Sensitivity as three presets, not a slider.** Users don't have intuition for energy thresholds. "Quiet room" vs "Noisy" maps to their environment. Three presets keep the UI minimal and cover the practical range. We can always add a custom slider later if presets prove insufficient.

**Caller-driven session pausing, not service-internal observation.** The service doesn't know about session state — and shouldn't. The view layer already observes `turnState` and can trivially call `pauseListening()`/`resumeListening()`. This keeps the service focused on audio, not session semantics.

**Pause mutes detection, doesn't tear down audio.** Stopping and restarting `AVAudioEngine` adds 100-200ms latency and can cause audible clicks. Pausing just ignores energy readings, keeping the audio session warm. Resume is instant.

## Scope

**In scope:**
- `InputMode` enum and `.listening` state on `VoiceInputService`
- `startListening()`, `stopListening()`, `pauseListening()`, `resumeListening()` API
- `VADSensitivity` presets and `setVADSensitivity()` API
- `VADEvent` enum on `VoiceInputEngine` protocol
- `startVADSession()`/`stopVADSession()` protocol methods with default implementations
- WhisperKit VAD: `useVAD: true`, energy monitoring, silence timer, utterance lifecycle
- Apple VAD: energy-based detection on audio tap, session start/stop cycling
- VoiceInputButton: long-press to enter VAD, visual states, context menu for sensitivity
- Session-aware pause/resume wiring in WaveSessionView
- Tests for VAD state transitions, sensitivity presets, pause/resume, utterance lifecycle
- Fix duplicate `cancelStreaming()` call on branch (existing bug)

**Out of scope:**
- Auto-send after transcription (wave item 02-auto-send)
- Custom vocabulary / language model priming
- Audio session conflict handling (calls, music playback)
- Wake words or voice commands
- SpeechDetector integration (revisit if it has a streaming API)

## Done when

```bash
swift test --package-path swift
cd swift && xcodegen generate && xcodebuild test \
  -project LoopflowSwift.xcodeproj \
  -scheme Concerto \
  -destination 'platform=macOS' \
  CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO
```

Observable: Long-press the mic button → state changes to "listening" → speak in a quiet room → text appears in composer → stop talking → after ~1.5s silence, transcript inserts and mic returns to listening. Tap mic to exit VAD mode. Push-to-talk still works on regular tap.

Wave goals advanced:
- "Graduate to hands-free via Voice Activity Detection — no button needed" — this is the deliverable
- "VAD false positives" risk mitigated by sensitivity presets and session-aware pausing
