# Voice Control

Voice input for Concerto's WaveSessionView. Speak instead of type — faster, hands-free, flow-preserving.

## Vision

Voice should be a first-class input modality in Concerto. Not a novelty mic button, but a primary way to interact with agents during high-capacity work. The bar: faster than typing for someone in flow.

The key insight: accuracy beats latency. A 0.5s delay you can trust is better than instant text you have to correct. WhisperKit on Apple Silicon gives us Whisper-grade accuracy with real-time performance, plus built-in VAD for hands-free operation.

### Not here

- Text-to-speech / agent reading responses aloud
- Voice commands ("next wave", "run tests") — dictation only for now
- Ambient always-listening mode beyond VAD start/stop
- Custom wake words

## Goals

- Ship push-to-talk voice input with streaming transcription into the composer
- Graduate to hands-free via Voice Activity Detection — no button needed
- Add auto-send on silence so the full loop (speak → transcribe → send) requires zero hand interaction
- Prime the recognizer with contextual vocabulary (wave names, file paths, loopflow terms)

## Tech decision: WhisperKit

[WhisperKit](https://github.com/argmaxinc/WhisperKit) by Argmax. Native Swift, SPM integration, Core ML on Apple Silicon Neural Engine. Built-in streaming, built-in VAD, custom vocabulary support. 27x+ real-time on M4 with the tiny model.

Alternatives considered:
- **SFSpeechRecognizer** — streaming but poor accuracy on technical speech, no custom vocabulary
- **SpeechAnalyzer** (WWDC 2025) — promising but requires macOS 26/iOS 26, still in beta
- **whisper.cpp** — same models but C++ with a thin Swift wrapper; WhisperKit is the native Swift equivalent with better Apple Silicon optimization

## Phase boundaries

- **01-push-to-talk**: Mic button in composer, WhisperKit integration, streaming transcription, manual send. *Shipped on branch `jack-heart.voicecontrol.20260226_0925` with `VoiceInputService`, `VoiceInputButton`, inline partial/download/transcribing feedback, denied-permission notice, and unit coverage.*
- **02-vad**: Voice Activity Detection replaces push-to-talk — speech starts/stops recording automatically. *Next.*
- **03-auto-send**: Silence threshold triggers send. Confidence-based — high confidence auto-sends, low confidence highlights uncertain words. *Later.*

## Risks

- **WhisperKit model size.** Even the tiny model is ~40MB. Need to handle first-run download gracefully.
- **Microphone permissions.** macOS and iOS both require explicit permission. First-use UX matters.
- **Accuracy on technical terms.** Whisper is good but not perfect on "lfd", "worktree", etc. Custom vocabulary priming helps but won't be 100%.
- **VAD false positives.** Background noise, music, other people talking. Need tunable sensitivity.
- **Audio session conflicts.** If the user is on a call or playing music, we need to handle shared audio gracefully.

## Metrics

- Time from thought to message sent (voice vs typing)
- Transcription accuracy on loopflow-specific terms
- False start/stop rate with VAD
- Correction rate (how often users edit transcribed text before sending)
