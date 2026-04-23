# chatgui — wave memory

## Patterns

- **Animation abstraction**: UI animations in Concerto must route through `DesignAnimation` (in `swift/LoopflowCore/Design/DesignSystem.swift`) so `reduceMotion` is respected uniformly. New repeating patterns (pulse, blink, shimmer) should be added as helpers — don't sprinkle bare `withAnimation(.easeInOut(...).repeatForever(...))` calls in views.
- **Transitions need an anchor**: `.transition(...)` only fires when the containing view tree sees an explicit animation. Typical pattern:
  - child: `.transition(.opacity.combined(with: .move(edge: ...)))`
  - parent: `.animation(DesignAnimation.standard(reduceMotion), value: <trigger>)` OR wrap the toggling mutation in `withAnimation(DesignAnimation.standard(reduceMotion)) { ... }`.
- **Transcript transition traps**: `state.groupedTranscript` rows are stable across streaming deltas (same id, growing content). Adding a naive insertion `.transition` on rows causes either no-op (append) or flicker-every-delta (updates). If we later want new-message entry animation, we need to separate "newly inserted row" from "content updated row" — likely via a small wrapper view keyed on message.id that only animates `.onAppear`.
- **SourceKit noise**: Live-editing WaveSessionView.swift and DesignSystem.swift surfaces spurious SourceKit diagnostics about `@Bindable init(wrappedValue:)` and missing colors (`loopflowCream`, `statusError`). These resolve on a full build; ignore them while editing.

## Preferences

- Headless runs claim a scratch doc via frontmatter (`status`, `claimed_by`, `claimed_at`) before touching it. Keep the frontmatter when updating; only edit the body.
- Prefer small, complete milestones per commit in headless mode — doc updates and code changes land together so review sees a consistent delta.

## Learnings

- `swift test --package-path swift` is the fast feedback loop for this area. Took ~40s here with 325 tests passing.
- `DesignAnimation.pulse(_:duration:)` covers both the thinking indicator (1.2s) and streaming cursor (0.6s). Parameterizing duration keeps one helper instead of two (pulse/blink).
