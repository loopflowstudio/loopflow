# 02: Animation and Smoothness

With the hot path fixed, address the visual polish that makes streaming feel fluid.

## Done when

Streaming a long response with tool calls looks smooth — no layout jumps, no animation restarts, no scroll fights. Side-by-side with Claude's web UI, Concerto holds up.

## Areas

- Scroll behavior during active streaming (stick-to-bottom without fighting)
- Transitions when tool run groups expand/collapse during streaming
- Thinking indicator entry/exit transitions
- Message appearance animation for new messages vs. streamed content
- Context snapshot expand/collapse (SessionContextView transition gap)
- Ensure all `withAnimation` calls use `DesignAnimation` helpers
