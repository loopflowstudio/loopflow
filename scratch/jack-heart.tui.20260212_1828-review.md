# Branch Review: jack-heart.tui.20260212_1828

## What was implemented

- Added a new `chat` contract module in `rust/loopflow/src/chat/` with shared request/response/event types (`ChatTurnRequest`, `ChatTurnResult`, `AgentEvent`, `SendMessageArgs`, snapshots/log types).
- Added completion helpers that enforce the turn rule: successful turns must emit exactly one `final` message.
- Added parser/validation entry point for `send_message` JSON arguments.
- Added focused contract tests for serde round-trips, completion edge-cases, and invalid tool payloads.
- Added a numbered harness roadmap (`roadmap/harness/01-11`) plus roadmap index documenting sequencing and invariants.

## Key choices

- **Explicit phase contract**: `UserMessagePhase` is a required enum (`progress` | `final`) rather than inferred from text.
- **Completion as validation helper**: kept runtime behavior out of this commit and shipped contract-first validation primitives.
- **Forward-compatible enums**: marked public chat enums and completion errors `#[non_exhaustive]` so event/error surfaces can grow without breaking downstream users.
- **Token budget in request shape now**: `token_history_budget` is part of the API even before persistence/runtime implementation.

## How it fits together

`chat::contract` defines the wire/data model for chat turns and event streaming. `chat::completion` provides pure helpers over `AgentEvent` streams (`is_user_message`, `final_message_count`, `validate_turn_completion`). `chat::mod` re-exports the contract so future runtime/persistence layers can depend on one stable surface.

## Risks and bottlenecks

- `MemoryEditLog`, `ToolCallLog`, and `ContextSnapshot` are currently minimal placeholders; downstream persistence/API work must confirm final wire compatibility.
- `validate_turn_completion` assumes it is called on successful-turn event streams; failure-stream semantics are deferred to later runtime integration.
- Roadmap “done when” test filters are planning-level commands and may drift unless kept synchronized with real test names.

## What's not included

- No model/provider integration (Anthropic/OpenAI/Gemini).
- No `lf-agent` process runtime or tool dispatch loop.
- No DB migrations or lfd chat endpoints.
- No streaming transport wiring.
- No memory compaction algorithm.
