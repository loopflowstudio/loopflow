# 02: Voice Activity Detection

Replace push-to-talk with hands-free operation. WhisperKit's built-in VAD detects speech start/stop automatically. No button press needed to begin or end recording.

**Status: planned (next)**

## What to build

### VAD mode in VoiceInputService

Extend `VoiceInputService` with a VAD mode:

```swift
enum InputMode { case pushToTalk, voiceActivityDetection }

func startListening() async throws   // VAD: continuous mic, auto-start/stop on speech
func stopListening()                  // exit VAD mode entirely
```

In VAD mode:
1. Mic stays open, WhisperKit's VAD monitors for speech onset
2. When speech detected → transition to `.recording`, begin transcription
3. When silence detected (configurable threshold, default ~1.5s) → stop recording, insert transcript
4. Return to listening for next speech segment

### VoiceInputButton update

The button gains a third mode. Tap cycles: idle → push-to-talk → VAD → idle. Or: single-tap for push-to-talk, long-press to enter VAD mode.

Visual state for VAD listening: `mic.badge.waveform` or similar, muted accent color. Distinct from actively recording (which stays red/burgundy).

### Sensitivity control

Add a VAD sensitivity setting — accessible from a long-press menu or settings. Three presets: quiet room, normal, noisy. Maps to WhisperKit's VAD energy threshold.

### Session-aware activation

VAD should only be active when the session is in a state where input makes sense — `turnState == .completed` or `.idle`. Auto-pause VAD while the agent is running (`.running`). Resume listening when the turn completes.

## Constraints

- VAD false-positive rate must be low enough that background noise doesn't trigger constant recordings
- Battery/CPU: VAD monitoring must be lightweight — WhisperKit's VAD runs on the Neural Engine
- Graceful degradation: if VAD misfires, user can always fall back to push-to-talk

## Done when

You can speak without pressing any button. Text appears in the composer when you stop talking. Works reliably in a quiet room. Push-to-talk remains available as fallback.
