# A2 — Multi-turn with harness events (current state)

## Objective

Route Concerto chat turns through the harness boundary so chat UX is driven by semantic harness events (`send_message`, `memory_edit`, completion), not direct model output.

## Final architecture

- `lfd` starts chat turns with `POST /waves/:wave_id/chat`.
- `lfd` streams turn events with `GET /waves/:wave_id/chat/events` (SSE).
- `ChatState` in Concerto is an event consumer/state machine (`idle -> running -> completed|failed`).
- Assistant text is rendered only from harness `send_message` events.
- `memory_edit` events are applied by chat APIs and persisted as wave-scoped memory blocks.

## Shipped behavior

- Added wave-scoped chat memory block APIs:
  - `GET /waves/:wave_id/memory-blocks`
  - `PUT /waves/:wave_id/memory-blocks/:name`
  - `DELETE /waves/:wave_id/memory-blocks/:name`
- Added harness-backed chat turn APIs:
  - `POST /waves/:wave_id/chat`
  - `GET /waves/:wave_id/chat/events`
- Added `turn::run_with_event_handler(...)` so harness `AgentEvent`s are emitted live during turn execution.
- Added persistent chat memory storage in SQLite/Postgres (`007_chat_memory_blocks.sql`).
- Added chat UI integration in Concerto (`WaveDetailPanel` chat tab, event-driven chat timeline, inline memory update indicators).

## Contract decisions

- Success requires exactly one `send_message(phase="final")`.
- Invalid completion sequences surface explicit failure in UI.
- Memory ownership stays in the chat system (harness requests edits, chat persists/display them).
- SSE is used for real-time event semantics (not polling).

## Related branch work

- Gate command plumbing now reads lint/test commands from `.lf/config.yaml`.
- Added `lf ops lint` and `lf ops test` commands.

## Known limits / follow-ups

- Chat turn streams are currently in-memory (`HttpState.chat_turns`) with no TTL/cleanup policy.
- SSE reconnect semantics are limited to replaying retained history for an existing turn ID.
- No approval flow yet for `memory_edit` events.
- No cross-restart persistence/replay for chat turn event streams.
