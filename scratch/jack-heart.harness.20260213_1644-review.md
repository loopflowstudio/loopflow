# Foundation Contract — Review

## What was implemented

Chat/runtime contract types and validation for the agent harness, completing all three commit slices (C1-C3) from the roadmap item:

- **Contract types** (`chat/contract.rs`): `AgentEvent` enum (Message, ToolCall, ToolResult, MemoryEdit, Done, Failed), `ChatTurnRequest`, `ChatTurnResult`, `WorkspaceSnapshot`, `ContextSnapshot`, `SendMessageArgs`, `UserMessagePhase`, plus helper types (`MemoryEditLog`, `ToolCallLog`).
- **Completion validation** (`chat/completion.rs`): `validate_turn_completion` enforces exactly-one-final on success, no-final-on-failure. `CompletionError` uses `thiserror` for human-readable messages. Three error variants: `MissingFinalMessage`, `MultipleFinalMessages`, `FinalMessageOnFailedTurn`.
- **Tests** (`chat/contract_test.rs` + inline): 22 tests covering serde round-trips for all types, completion rules, failed-turn validation, empty streams, and Display output.

Roadmap item deleted from `roadmap/harness/`, design doc created at `scratch/harness-foundation-contract.md`.

## Key choices

| Decision | Why |
|----------|-----|
| `thiserror` for `CompletionError` | Style guide says "use thiserror for library error types callers need to match on." Gives us `Display` for free. |
| Failed-turn validation inside `validate_turn_completion` | One function checks all completion rules rather than splitting into `validate_success` / `validate_failure`. Simpler API — caller doesn't classify the outcome. |
| `#[non_exhaustive]` on enums, no schema versioning | Sufficient for forward compatibility. Versioning is premature until there's a second consumer or wire protocol. |
| Model-agnostic event vocabulary | `AgentEvent` uses semantic concepts (Message, ToolCall, Done) not model-specific ones (content_block_delta, tool_use). Anthropic types in `agent/` stay separate. |

## How it fits together

The `chat/` module defines the boundary between the agent harness (B-track) and the chat system (A-track). `ChatTurnRequest` flows in, `AgentEvent` stream flows out, `ChatTurnResult` summarizes a completed turn. The harness emits events; the chat system consumes them. `validate_turn_completion` is the contract's enforcement point — consumers call it on the event stream to verify the turn satisfied the protocol.

## Risks and bottlenecks

- **`AgentEvent` field evolution**: Adding fields to existing variants is non-breaking (serde ignores unknown fields by default), but removing or renaming fields would break consumers. The `#[non_exhaustive]` only protects against new *variants*.
- **No streaming consideration**: The contract assumes post-hoc validation of a collected event vec. If B2 needs to validate incrementally during streaming, the API shape may need to change.

## What's not included

- Runtime behavior (B1 — turn loop, model calls)
- Wire format / JSONL serialization (B2)
- `send_message` tool registration (B2)
- Schema versioning
