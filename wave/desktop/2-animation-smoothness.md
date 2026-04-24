# 01: Animation and Smoothness

With the hot path fixed (streaming state is incremental in SessionState), address the visual polish that makes streaming feel fluid.

## Done when

Streaming a long response with tool calls looks smooth — no layout jumps, no animation restarts, no scroll fights. Side-by-side with Claude's web UI, Concerto holds up.

## Already done

- Scroll auto-scroll uses `DesignAnimation.fast(reduceMotion)` instead of bare `withAnimation`
- `StreamingCursorView` identity is pinned via `.id("streaming-cursor-\(message.id)")` — no flicker on deltas

## Remaining

- Transitions when tool run groups expand/collapse during streaming
- Thinking indicator entry/exit transitions
- Message appearance animation for new messages vs. streamed content
- Context snapshot expand/collapse (SessionContextView transition gap)
- Migrate two `withAnimation` calls in `WaveSessionView.swift` that use bare `.easeInOut` instead of `DesignAnimation` helpers: `SessionThinkingIndicator` pulsing animation (~line 390) and `StreamingCursorView` visibility animation (~line 450). Both respect `reduceMotion` via guards but bypass the `DesignAnimation` abstraction.
