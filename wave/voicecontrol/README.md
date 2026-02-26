# Voice Control

Voice input for Concerto's WaveSessionView. Speak instead of type — faster, hands-free, flow-preserving.

## Vision

Voice should be a first-class input modality in Concerto. Not a novelty mic button, but a primary way to interact with agents during high-capacity work. The bar: faster than typing for someone in flow.

The key insight: accuracy beats latency. A 0.5s delay you can trust is better than instant text you have to correct. On-device dictation engines (Apple Speech on latest OS, WhisperKit fallback on older OS) should prioritize trustworthy text over raw speed.

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

## Priority roadmap (top)

This is the current top of the roadmap, in order:

1. **Apple-first engine (macOS 26+/iOS 26+).** Use `SpeechAnalyzer` + `DictationTranscriber` with progressive dictation for live editable output.
2. **Background asset preinstall at app launch.** Use `AssetInventory.status(forModules:)` + installation request download so first mic press does not block on model setup.
3. **Dictation-specific text options.** Enable punctuation/emoji/etiquette replacements for cleaner output by default.
4. **WhisperKit fallback path.** Keep WhisperKit for macOS 15–25 / iOS 18–25 via runtime engine selection.
5. **Custom language model (later).** Use `SFSpeechLanguageModel` + `DictationTranscriber.ContentHint.customizedLanguage(...)` for loopflow terms.

## Tech direction: Apple-first with Whisper fallback

- **Primary path (macOS 26+/iOS 26+):** Apple Speech stack (`SpeechAnalyzer` + `DictationTranscriber` + `AssetInventory`).
- **Fallback path (macOS 15–25 / iOS 18–25):** [WhisperKit](https://github.com/argmaxinc/WhisperKit) tiny model.

This gives us newer dictation behavior and asset management on latest OS versions without dropping support for existing targets.

## Phase boundaries

- **01-push-to-talk**: Mic button in composer, WhisperKit integration, streaming transcription, manual send. *Shipped on branch `jack-heart.voicecontrol.20260226_0925` with `VoiceInputService`, `VoiceInputButton`, inline partial/download/transcribing feedback, denied-permission notice, and unit coverage.*
- **02-apple-engine-foundation**: Apple-first `SpeechAnalyzer` + `DictationTranscriber` engine, background asset preinstall, dictation options, runtime fallback. *Shipped on same branch — `AppleDictationVoiceInputEngine`, `VoiceInputWarmup`, runtime engine selection.*
- **03-vad**: Voice Activity Detection replaces push-to-talk — speech starts/stops recording automatically. *Next.*
- **04-auto-send**: Silence threshold triggers send. Confidence-based — high confidence auto-sends, low confidence highlights uncertain words. *Later.*

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
