# 05: Wave Integration (Track B3)

The two tracks converge. A2 shipped the plumbing (lfd orchestrates turns, SSE streams events, memory persists) — B3 wires this into the wave lifecycle so multiple agent invocations share memory across a wave run.

## What to build

- lfd seeds harness context from wave memory blocks at turn start
- Memory blocks carry forward across invocations within a wave run
- Turn stream lifecycle management (TTL/cleanup for in-memory `HttpState.chat_turns`)
- Approval flow for `memory_edit` events (currently auto-applied)

## Resolved questions

- What crosses the invocation boundary? Memory blocks — A2 shipped the persistence layer (`007_chat_memory_blocks.sql`) and CRUD APIs. The harness reads them at turn start, requests edits via tool calls.
- Does the harness need to know it's in a wave? No — confirmed by both B2 and A2. lfd provides memory and prompt; the harness runs a turn and emits events.

## Open questions

- Should chat turn streams persist across lfd restarts? Currently in-memory only. For single-turn chat this is fine, but wave runs spanning hours may need bounded persistence for restart resilience.
- What's the approval UX for `memory_edit`? Auto-apply (current) vs. user confirms before persisting.

## Done when

A wave runs multiple agent invocations. Memory carries forward via the chat system. lfd orchestrates it.
