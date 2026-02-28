# Voice Control

## Vision

Voice input for Concerto's WaveSessionView. Speak instead of type — faster, hands-free, flow-preserving. Not text-to-speech, not voice commands ("next wave", "run tests"), not ambient always-listening beyond VAD, not custom wake words.

The bar: faster than typing for someone in flow.

## Strategy

Accuracy beats latency. A 0.5s delay you can trust is better than instant text you have to correct.

Apple-first with WhisperKit fallback: `SpeechAnalyzer` + `DictationTranscriber` on macOS 26+/iOS 26+, WhisperKit tiny model on older OS versions. Runtime engine selection via `defaultEngineFactory()`.

Push-to-talk (Phase 01) and Apple engine foundation (Phase 02) shipped — `VoiceInputService` in LoopflowCore, `VoiceInputButton` with tap-to-toggle and press-and-hold, `AppleDictationVoiceInputEngine` with progressive dictation, `VoiceInputWarmup` for background asset preinstall, dictation-specific output options (punctuation/emoji/etiquette), and runtime fallback to WhisperKit.

Custom language model (`SFSpeechLanguageModel` + `DictationTranscriber.ContentHint.customizedLanguage(...)` for loopflow terms) deferred until base Apple engine path is validated in production use.

## Goals

- Ship push-to-talk voice input with streaming transcription into the composer
- Graduate to hands-free via Voice Activity Detection — no button needed
- Add auto-send on silence so the full loop (speak → transcribe → send) requires zero hand interaction
- Prime the recognizer with contextual vocabulary (wave names, file paths, loopflow terms)

## Risks

- **WhisperKit model size.** Even the tiny model is ~40MB. Need to handle first-run download gracefully.
- **Microphone permissions.** macOS and iOS both require explicit permission. First-use UX matters.
- **Accuracy on technical terms.** Whisper is good but not perfect on "lfd", "worktree", etc. Custom vocabulary priming helps but won't be 100%.
- **VAD false positives.** Background noise, music, other people talking. Need tunable sensitivity.
- **Audio session conflicts.** If the user is on a call or playing music, we need to handle shared audio gracefully.

## Metrics

- Time from tap-to-talk to message sent: seconds (target: <3s for a short instruction, vs ~10s typing)
- Word error rate on loopflow terms (lfd, worktree, wave, sprint): % (target: <10%)
- VAD false activation rate: false starts per hour of ambient use (target: <2/hr)
- Correction rate: % of transcriptions edited before sending (target: <20%)
