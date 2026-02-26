# Voice Control: Push-to-Talk + Apple Engine Foundation

Review doc for branch `jack-heart.voicecontrol.20260226_0925`.

## What was implemented

Push-to-talk voice input for Concerto's WaveSessionView, with two speech engines:

- **AppleDictationVoiceInputEngine** (macOS 26+/iOS 26+) — `SpeechAnalyzer` + `DictationTranscriber` with progressive short dictation preset. Installs speech assets in background at app launch.
- **WhisperKitVoiceInputEngine** (macOS 15–25/iOS 18–25) — WhisperKit `tiny` model with on-demand download and disk caching.

UI: `VoiceInputButton` in the composer with tap-to-toggle and press-and-hold modes. Partial transcript streams into the composer text field. Model download progress, transcribing status, and permission denial shown inline.

This covers wave items 01 (push-to-talk) and 02 (apple-engine-foundation).

## Key choices

**Two-engine architecture with protocol abstraction.** `VoiceInputEngine` protocol lets `VoiceInputService` be engine-agnostic. Runtime selection in `defaultEngineFactory()` via `#available`. This keeps the service testable with mock engines and allows adding future engines without touching the service.

**`@unchecked Sendable` for engine classes.** Both engines hold non-Sendable types (WhisperKit, SpeechAnalyzer). Access is serialized through the `@MainActor` service, documented with safety comments. This avoids fighting Swift 6 concurrency for types we don't control.

**DragGesture for press detection.** `VoiceInputButton` uses `DragGesture(minimumDistance: 0)` to distinguish tap (~220ms threshold) from hold. This is a standard SwiftUI pattern since `onLongPressGesture` doesn't cleanly support the "release to stop" flow.

**Transcript cleaning via regex chain.** `cleanedVoiceTranscript()` strips WhisperKit control tokens, blank-audio markers, and hallucinated sound descriptions. Applied on both partial and final transcripts. The Apple engine doesn't produce these artifacts, but the cleaning is harmless and keeps the contract consistent.

**Separate warmup service instance.** `VoiceInputWarmup` in `ConcertoApp` pre-downloads/pre-installs speech assets at launch. It uses its own `VoiceInputService` instance — the session view creates a separate one. The warmup benefits the session indirectly: WhisperKit finds the downloaded model on disk; Apple's AssetInventory manages assets system-wide. In-memory model preparation still happens per-instance.

## How it fits together

```
ConcertoApp
  └── VoiceInputWarmup.start()          (background asset preinstall)

WaveSessionView
  ├── @State voiceService: VoiceInputService
  ├── VoiceInputButton(voiceService:onTranscript:)
  ├── .onChange(voiceService.state)      → handleVoiceStateChange
  ├── .onChange(voiceService.partialTranscript) → syncVoicePartialIntoComposer
  └── voiceFeedback                     (status row, permission notice, errors)

VoiceInputService
  ├── state: .idle | .recording | .transcribing
  ├── permissionClient → AVCaptureDevice authorization
  └── engine: VoiceInputEngine
        ├── AppleDictationVoiceInputEngine  (macOS 26+)
        └── WhisperKitVoiceInputEngine      (fallback)
```

## Risks and bottlenecks

- **Cold start on first use.** WhisperKit model download (~40MB) and Apple asset installation both require network. The prewarm at app launch mitigates this, but first-ever launch may still block on model prep when the user presses the mic button. Download progress is shown inline.
- **Apple Speech availability.** `SpeechAnalyzer` requires macOS 26+ which is currently in beta. The `@available` gates protect against compilation issues, but real-world behavior needs validation on shipping OS releases.
- **Audio session conflicts.** Neither engine explicitly configures `AVAudioSession` category/mode. If the user is on a call or playing audio, the tap may fail silently or produce garbled input. This is deferred to a later phase.
- **Warmup doesn't share in-memory state.** The warmup service's prepared engine isn't reused by session views. Each session view cold-prepares its own engine. For WhisperKit this means re-loading CoreML models; for Apple this means re-calling `prepareToAnalyze`. The disk/system assets are cached, so it's a CPU cost, not a network cost.

## What's not included

- **VAD (wave 03)** — hands-free recording with automatic speech start/stop detection
- **Auto-send (wave 04)** — silence-triggered send with confidence scoring
- **Custom language model** — vocabulary priming for loopflow-specific terms
- **Audio session configuration** — explicit category/mode for coexistence with other audio
- **UI tests** — Concerto UI test bootstrapping is still unreliable (known issue)

## Gate fixes applied

- Deduplicated identical catch blocks in `ensureModelPrepared` (2 occurrences — specific timeout + generic had identical bodies)
- Added safety comment to `AppleDictationVoiceInputEngine` matching the existing one on `WhisperKitVoiceInputEngine`
- Renamed `02-vad.md` → `03-vad.md` and `03-auto-send.md` → `04-auto-send.md` to match content numbering after `02-apple-engine-foundation.md` was inserted
- Updated `02-apple-engine-foundation.md` status from "in progress" to "shipped"
- Updated wave README phase boundaries to reflect 02 as shipped
- Removed completed 02 from 01's follow-up list
