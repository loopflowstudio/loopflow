---
status: in-flight
proposal: wave/reduce/proposals/session-record-spine.md
worker: Ohm
---

# Remove the conversations subsystem

**Finish line:** Conversation code, API surface, clients, docs, tests, and store
hooks are gone. Nothing in the repo presents conversations as a live loopflow
subsystem.

## Why

Conversation support is dormant product surface. It might be useful later, but
keeping it now creates a parallel model beside lfd sessions and makes the
Session Record design harder to see. Git is the archive. If the need returns,
bring the useful pieces back under the Session Record model.

## Pull every root

- Delete `rust/loopflow/src/lfd/conversations/`.
- Remove lfd startup wiring for `ConversationManager`, orphan recovery, and
  conversation fields in `HttpState`.
- Remove `/v0/conversations/{id}/input`, `/events`, and `/usage` routes.
- Remove conversation usage aggregation or replace usage with the currently
  live source of truth.
- Remove store traits and sqlite/postgres methods for `conversations` and
  `conversation_events`.
- Remove migration references or add a deliberate migration policy for dropping
  unused conversation tables.
- Remove Python `Conversation`, `ConversationConfig`,
  `ConversationEventEnvelope`, client methods, API functions, and tests.
- Remove Swift conversation service methods, protocol requirements,
  `ConversationEventEnvelope`, and `SessionState` stream/input handling.
- Remove docs for the Conversations API.
- Remove e2e/client tests that only prove conversation endpoints exist.
- Refresh generated summaries or checked-in context that still describes
  conversations as active.

## Guardrails

- Do not leave compatibility endpoints.
- Do not keep old/new names.
- Do not add replacement abstractions until a live product path needs them.
- Preserve lfd sessions, runs, execution processes, output, attention, and usage
  only where they are currently used outside the conversation subsystem.

## Verification

- `rg -n "conversation|Conversation|conversations"` returns only ordinary prose
  uses, reduce notes, or unrelated prompt text.
- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test --all`
- `uv run pytest python/tests/`
- `swift test --package-path swift`
