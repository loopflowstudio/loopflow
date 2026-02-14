# Foundation Contract

Chat/runtime contract types and validation for the agent harness. Completed — all three commit slices (C1-C3) done.

## What shipped

- **Contract types** (`chat/contract.rs`): `AgentEvent` enum (Message, ToolCall, ToolResult, MemoryEdit, Done, Failed), `ChatTurnRequest`, `ChatTurnResult`, `WorkspaceSnapshot`, `ContextSnapshot`, `SendMessageArgs`, `UserMessagePhase`, plus helper types (`MemoryEditLog`, `ToolCallLog`).
- **Completion validation** (`chat/completion.rs`): `validate_turn_completion` enforces exactly-one-final on success, no-final-on-failure. `CompletionError` uses `thiserror`. Three variants: `MissingFinalMessage`, `MultipleFinalMessages`, `FinalMessageOnFailedTurn`.
- **Tests** (`chat/contract_test.rs` + inline): 22 tests covering serde round-trips for all types, completion rules, failed-turn validation, empty streams, and Display output.

## Key decisions

| Decision | Why |
|----------|-----|
| `thiserror` for `CompletionError` | Style guide: "use thiserror for library error types callers need to match on." |
| Failed-turn validation inside `validate_turn_completion` | One function checks all completion rules. Caller doesn't classify the outcome. |
| `#[non_exhaustive]` on enums, no schema versioning | Sufficient for forward compatibility. Version when there's a second consumer or wire protocol. |
| Model-agnostic event vocabulary | `AgentEvent` uses semantic concepts (Message, ToolCall, Done) not model-specific ones. Anthropic types in `agent/` stay separate. |
| Validation over event sequences, not individual events | `validate_turn_completion` takes `&[AgentEvent]`. Completion is a cross-cutting concern. |

## How it fits together

The `chat/` module defines the boundary between the agent harness (B-track) and the chat system (A-track). `ChatTurnRequest` flows in, `AgentEvent` stream flows out, `ChatTurnResult` summarizes a completed turn. `validate_turn_completion` is the contract's enforcement point.

## Risks for B1

- **Field evolution**: Adding fields to existing `AgentEvent` variants is non-breaking (serde ignores unknown fields), but removing or renaming fields breaks consumers. `#[non_exhaustive]` only protects against new *variants*.
- **Post-hoc validation**: The contract assumes validation of a collected event vec. If B2 needs incremental validation during streaming, the API shape may need to change.

## Not included (deferred to later tracks)

- Runtime behavior (B1 — turn loop, model calls)
- Wire format / JSONL serialization (B2)
- `send_message` tool registration (B2)
- Schema versioning
