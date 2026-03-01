# 04: Auto-Send on Silence

**Finish line:** In a quiet room with continuous mode on: speak a message, wait 2 seconds, message sends automatically. Speak again when the agent finishes. The full voice conversation loop works hands-free.

## What to build

### Auto-send timer

After transcription completes in VAD mode, start a configurable countdown (default ~2s). If no new speech and no keyboard/mouse interaction during the countdown, auto-send the composed message.

Visual: a subtle countdown indicator near the composer — a shrinking bar or fading ring. Tapping anywhere, pressing a key, or starting to speak again cancels the auto-send and keeps the text in the composer for editing.

### Confidence-based behavior

Confidence score availability varies by engine — WhisperKit provides per-segment scores natively, the Apple dictation path may not expose the same granularity. Design the threshold behavior to degrade gracefully when scores aren't available (default to high-confidence behavior).

Two thresholds when scores are available:

- **High confidence** (>0.9): auto-send countdown starts immediately
- **Low confidence** (<0.9): highlight uncertain words in the composer (underline or muted color), pause auto-send, let the user review

### Continuous conversation mode

With VAD + auto-send, the full loop is: speak -> transcribe -> send -> agent works -> agent finishes -> VAD resumes -> speak again. No hands at any point.

Add a "Continuous" toggle accessible from the voice button's menu. When active, the full loop runs automatically. When off, transcription inserts text but doesn't auto-send.

## Constraints

- Auto-send must be cancellable — any user interaction interrupts
- The cancel window must be visually obvious (not color-only — use underline or bold for confidence highlighting)
- Never auto-send an empty or whitespace-only message
- Apple engine VAD currently uses partial-transcript inactivity timing (not energy-based onset/offset). Silence detection for auto-send triggering may behave differently across engines — test both paths
