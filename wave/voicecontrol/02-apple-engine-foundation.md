# 02: Apple Engine Foundation

Move Concerto voice input to an Apple-first stack on the newest OS versions, while preserving compatibility on current shipping targets.

**Status: shipped** (branch `jack-heart.voicecontrol.20260226_0925`)

## What shipped

1. **Apple-first engine (macOS 26+/iOS 26+)** — `AppleDictationVoiceInputEngine` with `SpeechAnalyzer` + `DictationTranscriber` using progressive short dictation preset for low-latency partial updates.
2. **Background speech asset preinstall** — `VoiceInputWarmup` at app launch checks `AssetInventory.status(forModules:)` and installs missing assets via `assetInstallationRequest(...).downloadAndInstall()`.
3. **Dictation-specific output options** — Punctuation, emoji, and etiquette replacements enabled on the Apple dictation path.
4. **Runtime fallback to WhisperKit** — `defaultEngineFactory()` selects `AppleDictationVoiceInputEngine` on macOS 26+/iOS 26+, falls back to `WhisperKitVoiceInputEngine` on older OS versions.

## Open follow-up work

- **Custom language model** — Add `SFSpeechLanguageModel` + `DictationTranscriber.ContentHint.customizedLanguage(...)` for loopflow-specific terms. Deferred until base Apple engine path is validated in production use.

## Done when

- macOS 26+/iOS 26+ uses Apple dictation engine end-to-end.
- First mic press no longer blocks on cold asset download in normal cases.
- WhisperKit fallback still works on older supported OS versions.
