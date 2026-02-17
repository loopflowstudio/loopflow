# A2 — Multi-turn with harness events

## Problem

A1 proved the chat surface and manual memory UX. The next risk is architecture: can Concerto run chat through the harness boundary without regressing UX? We need to replace direct Swift→Anthropic calls with harness turns so the product validates the real contract (`send_message`, `memory_edit`, completion events) before B3 wave orchestration expands it.

Who benefits: users get the same chat feel with better reliability and memory continuity, and the team gets hard evidence that the harness/chat boundary is viable.

Why now: A1 is shipped and the harness already supports turn loops, tools, and memory-edit events.

## Approach

Route all chat turns through lfd-managed harness sessions and consume semantic events in real time.

1. Add a wave-scoped "chat turn" API in lfd:
   - `POST /waves/{wave}/chat/turns` starts a harness turn with user message + current memory blocks.
   - `GET /waves/{wave}/chat/turns/{id}/events` streams `AgentEvent` frames (SSE).
2. Keep the boundary strict:
   - UI text only comes from `send_message` events.
   - Turn success requires exactly one `send_message(phase="final")`; otherwise show an error state.
3. Memory flow:
   - `memory_edit` events are applied by the chat system (not harness) and persisted immediately to wave-scoped chat memory blocks.
   - UI shows "Agent updated memory" inline badges so edits are visible.
4. Swift `ChatState` becomes an event consumer/state machine (`idle -> running -> completed|failed`) and never parses model raw output.
5. Preserve A1 UX shape (message list, input, memory panel), but assistant messages now arrive as progress/final events.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep direct Swift→Anthropic calls and emulate events in client code | Faster to wire, less backend work | Fails the core goal: we would not test real harness boundaries or completion invariants. |
| One-shot lfd endpoint returning only final text | Simpler transport, no stream handling | Hides progress semantics, prevents debugging tool/memory events, and makes A2 too different from B3 runtime behavior. |
| Embed harness in Swift (FFI/local process from app) | Lower network overhead | High integration complexity and couples UI runtime to harness internals; violates clean runtime boundary. |

## Key decisions

1. **Event-stream transport (SSE), not polling.** A2 must prove real-time harness semantics, not "request/response with extras."
2. **`send_message` remains the only UI output channel.** Following wave invariant: **"`send_message` is the only user-output mechanism."**
3. **Hard completion contract in product path.** Turn considered successful only when invariant holds: **"Exactly one `send_message(phase=\"final\")` on successful turns."**
4. **Memory ownership stays with chat system.** Harness can request edits; chat persists and displays them, matching: **"Memory is durable across invocations"** and **"Memory belongs to the chat system, not the harness."**
5. **Harness stays UI-agnostic.** lfd and Concerto consume events; harness gets prompt/tools/memory only, aligned with: **"The harness doesn't know about the chat system."**

## Scope

- In scope:
  - lfd endpoints to start chat turns and stream per-turn events
  - Wave-scoped memory block read/write APIs used by A2 flow
  - Swift `ChatState` event-driven turn lifecycle
  - UI rendering of progress/final assistant messages and memory-edit indicators
  - Error states for timeout, failed turn, or invalid completion contract
- Out of scope:
  - Token streaming of raw model deltas
  - Human approval workflow for memory edits
  - Multi-conversation/thread management
  - Model/provider selection UI
  - B3 wave-level multi-invocation orchestration

## Done when

- Manual:
  1. Launch Concerto, open chat, send a prompt.
  2. Assistant reply appears via harness `send_message` events (not direct Anthropic client path).
  3. A harness-issued `memory_edit` updates the memory panel and persists.
  4. Relaunch app; memory remains for that wave.
- Observable contract:
  - Every successful turn emits exactly one `final` message event; invalid sequences surface explicit UI error.
- Verification commands:
  - `cargo test -p loopflow lfd::http::routes::waves`
  - `swift test --package-path swift --filter ChatStateTests`
