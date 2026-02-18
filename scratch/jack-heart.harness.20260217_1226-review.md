# Review: chat route extraction, persistent messages, compressed event mapping

Branch: `jack-heart.harness.20260217_1226`

## What was implemented

Extracted chat routes from `waves.rs` into a dedicated `chat.rs` module. Added persistent chat messages (SQLite/Postgres via migration `008_chat_messages.sql`) so conversation history survives page reloads. Simplified the chat model from individually-addressed turns (`/chat/turns/:turn_id`) to one active chat per wave (`/chat`, `/chat/events`, `/chat/messages`). Compressed the event mapping between harness `AgentEvent`s and persisted `ChatMessage` records.

**Rust changes:**
- New `routes/chat.rs` (531 lines) — all chat + memory block handlers, event mapping, system prompt construction, completion validation
- New `state.rs` — `ChatTurnStream` (broadcast channel + completion flag), `ChatTurnRegistry` (one-active-per-wave guard), `HttpState` (added `chat_turns` field)
- New `types/chat_message.rs` — `ChatMessage` domain type
- New `008_chat_messages.sql` migration — persistent message storage with wave FK + created_at index
- Store trait + SQLite/Postgres implementations for `list_chat_messages` and `create_chat_message`
- `turn.rs` — added `extract_text_or_final_event` fallback so turns that produce all output via `send_message` (no raw text block) still succeed

**Swift changes:**
- Deleted `AnthropicClient.swift` (122 lines) — direct model calls replaced by harness-backed chat
- `ChatState` rewritten as event consumer: `send()` calls `startChat` then consumes `streamChatEvents` SSE
- `ChatTurn.swift` — new `ChatMessageRecord`, `ChatTurnPhase`, `ChatTurnEvent` models
- `LocalWaveService` — new `startChat`, `streamChatEvents`, `listChatMessages` + SSE parsing
- `WaveServiceProtocol` — `ChatService` protocol extracted for testability
- `WaveChatView` — loads persisted messages on appear, renders markdown for assistant bubbles

## Key choices

**One chat per wave, not addressed turns.** The turn ID indirection added complexity without value — a wave has at most one active chat turn. Simplified to `POST /chat` (start) and `GET /chat/events` (stream). The registry enforces one-active via `ChatTurnStartError::AlreadyRunning`.

**Persistent messages alongside in-memory event streams.** Messages persist to the DB for reload. The live SSE stream is still in-memory (broadcast channel). This avoids the complexity of replaying from DB on reconnect while still giving persistence for the happy path.

**`ChatMessage` role as string, not enum.** Matches the DB column and JSON wire format directly. The Swift side has `ChatRole` enum for type safety in the UI layer. Adding a Rust enum is a reasonable follow-up but wasn't needed for this PR since the roles are only set in `chat_message_from_agent_event` (controlled code path).

**`Ordering::Relaxed` for completion flag.** The `completed` AtomicBool on `ChatTurnStream` transitions monotonically `false` -> `true`. No correctness depends on seeing the update immediately — worst case is a brief race where a subscriber sees an already-finished stream, which the broadcast channel handles gracefully.

## How it fits together

```
Client                    lfd (HTTP)                      Harness
  |                         |                               |
  |-- POST /chat ---------> |                               |
  |                         |-- persist user message         |
  |                         |-- spawn turn task -----------> |
  |<-- { status: running }  |                               |
  |                         |                               |
  |-- GET /chat/events ---> |                               |
  |                         |<--- AgentEvent (broadcast) ---|
  |<-- SSE: agent_event --- |                               |
  |                         |-- persist messages to DB       |
  |                         |                               |
  |-- GET /chat/messages -> |                               |
  |<-- persisted history    |                               |
```

`ChatTurnRegistry` holds one `ChatTurnStream` per wave. The stream wraps a `broadcast::Sender` for live SSE delivery and an `AtomicBool` for completion gating. When the turn completes (or fails), `mark_completed()` prevents new subscribers and allows the next turn to start.

## Risks and bottlenecks

- **Event race on start.** Events emitted between `start_chat_handler` returning and the client subscribing via `stream_chat_events_handler` are lost (broadcast drops messages with no receivers). Fast progress messages may be missed. Mitigated by persistence — the client can `GET /chat/messages` to see the full history.
- **In-memory turn streams have no TTL.** `ChatTurnRegistry` holds `Arc<ChatTurnStream>` indefinitely. Completed streams accumulate. This is fine for interactive chat but needs eviction before wave-integrated usage (B3).
- **No SSE reconnect replay.** If the SSE connection drops mid-turn, the client must re-subscribe and may miss events. Persisted messages provide eventual consistency but not real-time replay.

## What's not included

- **`ChatMessage.role` as Rust enum.** Deliberately deferred — the string representation matches the DB and JSON wire format. Swift has the enum for UI type safety.
- **Turn stream TTL/eviction.** Noted in the roadmap for B3.
- **Memory edit approval flow.** Currently auto-applied. B3 decision.
- **Streaming token output.** The harness deliberately uses `send_message` events, not raw token streams. This is a design choice, not an omission.

## Gate fixes applied

- Fixed doc URL mismatches: `wave/harness/README.md` and `scratch/harness-a2-multi-turn-with-harness-events.md` referenced old `/chat/turns/:turn_id` URLs instead of the simplified `/chat` routes.
- Added `tracing::warn` for silenced store errors in `persist_turn_events` and `publish_terminal_event` — previously `let _ = store.create_chat_message(...)` would silently swallow persistence failures.
- Added `#[non_exhaustive]` and `thiserror::Error` to `ChatTurnStartError` per style guide requirements for public error enums.
