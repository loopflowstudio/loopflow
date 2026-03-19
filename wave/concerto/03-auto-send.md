---
linear_id: dba6c46e-5ba0-422a-9ad0-0a4f2dd09e57
---
# 03: Auto-Send on Silence

**Finish line:** In continuous voice mode, speak a message, pause, and Concerto sends it automatically; when the agent finishes, listening resumes without extra taps.

## Context

Concerto already has the pieces up to the edge of this experience. `VoiceInputButton` supports push-to-talk and VAD entry, `VoiceInputService` can pause/resume listening around agent turns, and transcripts already land in the composer. `WaveSessionView.handleTurnStateChange` already pauses listening while a turn runs, resumes on `.completed` and `.idle`, and leaves `.failed` untouched, so continuous mode needs an explicit failure policy instead of accidentally stranding VAD in a paused state. The remaining gap is the final handoff from "transcript in composer" to "message sent".

This branch also made reply staging real: queued replies are now editable, reorderable, and assembled through `ReplyQueue`. Auto-send must respect that send path rather than bypass it. If the user has staged replies, continuous mode should still send the same assembled message and honor the same empty-input rules.

## What to build

1. Start a short countdown after a VAD utterance finishes and the transcript is stable.
2. Show an obvious cancel affordance near the composer; any user interaction cancels auto-send and leaves the text in place.
3. Add a continuous-mode toggle to the existing voice controls so people opt into silence-based sending explicitly.
4. Route auto-send through the same send path as the normal Send button, including `ReplyQueue.assembleMessage(...)`.
5. Make the behavior degrade cleanly across engines: use confidence cues when available, but do not block the feature on engine-specific scoring APIs.

## Constraints

- Never auto-send empty or whitespace-only content
- Any new speech, queue mutation, keyboard input, or explicit cancel action stops the countdown
- The cancel window has to be visible without relying on color alone
- Agent turn transitions must not leave VAD stuck paused or listening at the wrong time

## What this item should teach us

- Whether the current VAD event stream is trustworthy enough for hands-free sending
- Which cancellation signals matter most in real use
- Whether confidence-based highlighting earns its complexity or just delays a simpler auto-send loop

## Done when

- Continuous mode can send a dictated message after a short silence without tapping Send
- Cancelling during the countdown leaves composer and queued replies untouched
- Auto-send uses the same assembled message format as manual send
- Listening resumes after the agent finishes when continuous mode is still on
- `swift test --package-path swift` and the relevant Concerto UI/manual checks cover the new loop
