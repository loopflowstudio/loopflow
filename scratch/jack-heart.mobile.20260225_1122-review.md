# Review: Action Buttons + Session Rename + Structured Replies

Branch: `jack-heart.mobile.20260225_1122`
~85 files changed, ~1900 insertions, ~670 deletions

## What was implemented

Three interconnected features shipped as one milestone:

1. **Structured reply pipeline** — Agents emit `<lf:suggest_actions>` tags in their text output. A streaming parser (`LfTagParser`) intercepts these tags, parses the JSON payload, and converts them to `SuggestedActions` session events. The prompt engine injects guidance telling the agent how and when to emit these tags, with context-awareness (compact mode caps at 3 actions, action_style shapes the suggestion tone).

2. **Action buttons UI** — `ActionButtonsView` (shared in LoopflowCore) renders suggested actions as tappable buttons. The `WaveSessionView` displays these below the transcript and clears them on user send. A 300ms hit-testing debounce prevents accidental taps during action transitions.

3. **Chat → Session rename** — Internal types renamed: `ChatState` → `SessionState`, `WaveChatView` → `WaveSessionView`. User-facing tab label intentionally kept as "Chat" (more intuitive for users than "Session").

Supporting changes:
- `Surface` enum replaces `run_mode` string for prompt surface context (headless/cli/concerto_mac/concerto_iphone)
- `action_style` frontmatter field on steps (procedural/exploratory) shapes suggestion guidance
- `LaunchPromptInput` struct consolidates launch-prep inputs shared by CLI and session API
- Step builtins updated with `action_style` annotations
- Flow parser refactored for cleaner step metadata handling
- Wave executor passes `client_has_ui` and `client_compact` through session creation

## Key choices

| Decision | Why | Alternative rejected |
|----------|-----|---------------------|
| Tags in text stream, not separate tool calls | Works with all harnesses; no tool schema coordination needed | Harness-specific tool registration (couples to each provider) |
| Server-side tag parsing (`LfTagParser`) | Client stays thin; all harnesses benefit | Client-side parsing (duplicates logic per platform) |
| `action_style` on step frontmatter | Steps have natural affinity for interaction patterns | Runtime config (too far from step intent) |
| `Surface` enum over `run_mode` string | Type-safe, extensible for new surfaces | Keep strings (error-prone, no exhaustiveness checking) |
| 300ms debounce on action buttons | Prevents accidental taps when actions change rapidly | No debounce (false taps during streaming) |

## How it fits together

```
Agent text stream
  → LfTagParser (server, per-session)
    → strips <lf:suggest_actions> from text
    → emits SuggestedActions session event
      → SSE to client
        → SessionState.suggestedActions
          → ActionButtonsView
```

The prompt engine (`structured_reply.rs`) generates guidance injected into the system prompt. `ClientContext` (has_ui, compact) and step `action_style` shape this guidance. The `LfTagParser` is stateful per-session to handle tags split across streaming chunks.

## Risks and bottlenecks

- **Tag parsing correctness**: Malformed tags or tags embedded in code examples could cause issues. The parser requires tags at line start (with optional whitespace) to avoid matching inline code examples. Invalid JSON payloads are silently dropped.
- **Agent compliance**: The agent must actually emit the tags. The guidance is in the system prompt but there's no guarantee. If the agent doesn't emit tags, no actions appear — graceful degradation.
- **Transcript grouping performance**: `groupedTranscript` is a computed property recalculated on every body evaluation. For very long sessions this could cause frame drops. Acceptable for now; cacheable if needed.

## What's not included

- No persistence of suggested actions across session reconnect (actions are live-stream only)
- No custom action_style values beyond procedural/exploratory
- No action buttons on iOS output tab (only in session/chat tab)
- Pre-existing debug `print()` statements in `DirectionTypeahead.swift` and `FlowTypeahead.swift` not cleaned up (out of scope for this branch)

## Test coverage

- Rust: `structured_reply.rs` — 5 tests covering UI context gating, compact limits, action_style guidance, and rendering
- Rust: `lf_tag.rs` — 5 tests covering normal parsing, split-chunk streaming, invalid JSON, unclosed tags, and inline tag examples
- Rust: `launch.rs` — 5 tests covering model selection, direction merging, area config, and structured reply injection
- Swift: `SessionStateTests` — 3 tests covering suggested action sanitization/capping, clear-on-send, and latest-payload-wins
- Swift: `RepoStateInteractiveSessionTests` — 2 tests covering session routing
- All existing tests continue to pass (151 Swift tests, 51 Python tests, Rust tests all green)

## Gate status

- cargo fmt: pass
- cargo clippy: pass (zero warnings)
- cargo test: pass
- swift test: pass (151/151)
- pytest: pass (51/51)
