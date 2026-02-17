# Review: A2 harness chat integration + gate command plumbing

## What was implemented

- Added wave-scoped chat memory APIs in `lfd`:
  - `GET /waves/:wave_id/memory-blocks`
  - `PUT /waves/:wave_id/memory-blocks/:name`
  - `DELETE /waves/:wave_id/memory-blocks/:name`
- Added harness-backed chat turn APIs in `lfd`:
  - `POST /waves/:wave_id/chat/turns`
  - `GET /waves/:wave_id/chat/turns/:turn_id/events` (SSE)
- Wired chat turns to the harness runtime by adding `turn::run_with_event_handler(...)`, so `AgentEvent`s are emitted live during tool execution and streamed to clients.
- Added persistent wave-scoped chat memory storage in SQLite/Postgres via migration `007_chat_memory_blocks.sql` and store trait implementations.
- Converted Concerto chat to event-driven state management:
  - `ChatState` now consumes streamed `ChatTurnEvent`s
  - completion contract enforced (`exactly one final`)
  - `memory_edit` events are applied immediately through memory block APIs
  - `WaveDetailPanel` now includes a dedicated **Chat** tab.
- Extended Loopflow config + ops UX for gates:
  - config fields `lint` and `test`
  - new commands `lf ops lint` and `lf ops test`
  - docs updated in `docs/config.md`, `docs/lfops.md`, `TESTING.md`, and gate/lint step prompts.
- Polish during gate pass: hardened `docker_mount_spec_resolves_allowlisted_credentials` to avoid HOME-env race flakiness in parallel test runs.

## Key choices

- **Strict output boundary:** UI only renders harness `send_message` events (not raw model text).
- **Strict success contract:** turns are only successful when one and only one `phase="final"` message exists.
- **Chat memory ownership stays outside harness:** harness emits `memory_edit`; chat system persists + displays.
- **SSE over polling:** gives real-time event semantics and preserves parity with future wave orchestration.
- **Project-defined gate commands:** lint/test execution now comes from `.lf/config.yaml` instead of hardcoded defaults.

## How it fits together

Concerto starts a turn via `POST /chat/turns`, then subscribes to SSE events for that turn. `lfd` runs the harness turn loop, publishes `AgentEvent`s into a turn stream registry, and exposes history + live events over SSE. `ChatState` updates the timeline from `message` events, applies `memory_edit` events through memory-block endpoints, and marks success/failure from terminal events + completion invariants.

## Risks and bottlenecks

- Chat turn streams are in-memory (`HttpState.chat_turns`) and currently have no TTL/cleanup policy.
- SSE delivery depends on long-lived connection stability; reconnect behavior is limited to history replay for existing turn IDs.
- Memory edits are auto-applied; no approval workflow exists yet.
- Turn context token accounting in completion `Done` is lightweight (`memory_tokens` currently set to 0 in this path).

## What's not included

- Multi-thread/conversation management.
- Token-level assistant streaming.
- Human approval flow for `memory_edit`.
- Cross-restart persistence/replay of chat turns/events.
- B3 wave-level multi-invocation orchestration.
