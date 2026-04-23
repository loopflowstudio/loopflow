---
status: in-progress
claimed_by: jack-heart.chatgui.20260423_1303
claimed_at: 2026-04-23T21:03:04.202487Z
---
# 01: Animation and Smoothness

With the hot path fixed (streaming state is incremental in SessionState), address the visual polish that makes streaming feel fluid.

## Done when

Streaming a long response with tool calls looks smooth — no layout jumps, no animation restarts, no scroll fights. Side-by-side with Claude's web UI, Concerto holds up.

## Already done

- Scroll auto-scroll uses `DesignAnimation.fast(reduceMotion)` instead of bare `withAnimation`
- `StreamingCursorView` identity is pinned via `.id("streaming-cursor-\(message.id)")` — no flicker on deltas
- `DesignAnimation.pulse(_:duration:)` helper for forever-repeating autoreverse animations (DesignSystem.swift)
- `SessionThinkingIndicator` pulsing animation migrated to `DesignAnimation.pulse`
- `StreamingCursorView` visibility animation migrated to `DesignAnimation.pulse`
- Thinking indicator fades + slides in/out on `isLoading` toggles instead of popping
- Tool run groups (`ToolRunView`) animate expand/collapse with opacity + move-from-top transition
- Transcript item details (`TranscriptItemCardView`) animate expand/collapse with opacity + move-from-top transition

## Remaining

- Message appearance animation for new messages vs. streamed content — the row identity is stable across streaming deltas, so a naive `.transition` on the message row would either do nothing (append) or fire on every delta (wrong). Needs a "first render" flag or a separate wrapper view keyed on message.id insertion before wiring the transition.
- Context snapshot expand/collapse already uses `DesignAnimation.standard` + `.transition(.opacity)`. The remaining "transition gap" referenced in the earlier note would need a visual repro before choosing a fix; layout height jumps are the most likely culprit.
