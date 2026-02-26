# 04: Auto-Send on Silence

Close the loop: speak → transcribe → send with zero hand interaction. Silence after speech triggers an automatic send, with a brief cancel window.

**Status: planned (later)**

## What to build

### Auto-send timer

After transcription completes in VAD mode, start a configurable countdown (default ~2s). If no new speech and no keyboard/mouse interaction during the countdown, auto-send the composed message.

Visual: a subtle countdown indicator near the composer — a shrinking bar or fading ring. Tapping anywhere, pressing a key, or starting to speak again cancels the auto-send and keeps the text in the composer for editing.

### Confidence-based behavior

WhisperKit provides per-segment confidence scores. Two thresholds:

- **High confidence** (>0.9): auto-send countdown starts immediately
- **Low confidence** (<0.9): highlight uncertain words in the composer (underline or muted color), pause auto-send, let the user review

This keeps the "zero touch" flow for clear speech while catching potential errors before they ship.

### Continuous conversation mode

With VAD + auto-send, the full loop is: speak → transcribe → send → agent works → agent finishes → VAD resumes → speak again. No hands at any point. This is the ambient mode.

Add a "Continuous" toggle accessible from the voice button's menu. When active, the full loop runs automatically. When off, transcription inserts text but doesn't auto-send.

## Constraints

- Auto-send must be cancellable — any user interaction interrupts
- The cancel window must be visually obvious (not just a timer the user can't see)
- Never auto-send an empty or whitespace-only message
- Confidence highlighting must be accessible (not color-only — use underline or bold)

## Done when

In a quiet room with continuous mode on: speak a message, wait 2 seconds, message sends automatically. Speak again when the agent finishes. The full voice conversation loop works hands-free.
