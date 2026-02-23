# 02: Multi-Turn with Harness Events (Track A2) — Shipped

Chat turns now route through lfd (`POST /waves/:wave_id/chat`) and stream events via SSE (`GET /waves/:wave_id/chat/events`). `ChatState` in Concerto is an event consumer/state machine (`idle → running → completed|failed`). Assistant text renders only from harness `send_message` events. Wave-scoped memory blocks persist in SQLite/Postgres with full CRUD APIs. `turn::run_with_event_handler()` emits `AgentEvent`s live during execution.

## What we learned

The harness boundary feels right — routing through `send_message` events works as the sole UI output mechanism. The UX without streaming tokens is acceptable; the event-driven approach (progress → final) gives the UI enough to show activity. SSE is the right transport for real-time event delivery. The in-memory turn stream (`HttpState.chat_turns`) works for the happy path but has no TTL/cleanup — this needs addressing before B3 where multiple agent invocations will accumulate streams. Memory ownership confirmed: chat system persists, harness requests edits via tool calls — the boundary is clean.
