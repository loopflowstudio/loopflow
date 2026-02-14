# Design Review: Foundation Contract + Tool Registry (B2 C1)

## What was implemented

Two pieces of harness infrastructure across 8 commits:

**Foundation contract** (`chat/`): The type-level boundary between the agent harness and the chat system. `AgentEvent` enum with 6 variants (Message, ToolCall, ToolResult, MemoryEdit, Done, Failed), supporting types (`ChatTurnRequest`, `ChatTurnResult`, `WorkspaceSnapshot`, `ContextSnapshot`, `SendMessageArgs`), and completion validation (`validate_turn_completion` enforcing exactly-one-final-message on success, no-final-on-failure).

**Tool registry** (`agent/registry.rs`): `Tool` trait + `ToolRegistry` that replaces B1's hardcoded `match` dispatch. Existing `GetCurrentTime` and `Calculate` tools migrated to trait impls. Turn loop updated to accept `&ToolRegistry` parameter.

## Key choices

| Choice | Why | Alternatives rejected |
|--------|-----|----------------------|
| `Tool` trait with `ToolResult { output, event }` | Boundary tools (send_message) need to emit events alongside model-facing results. Trait objects let boundary tools carry state (closures). | Function pointers (can't carry state), separate event channel (over-engineering) |
| `CompletionError` via `thiserror` | Style guide requires it for library error types callers match on. Three variants: `MissingFinalMessage`, `MultipleFinalMessages`, `FinalMessageOnFailedTurn`. | `anyhow` (no match ergonomics), custom `Display` (boilerplate) |
| `#[non_exhaustive]` on enums, no schema versioning | Sufficient for forward compatibility. Version when there's a second consumer or wire protocol. | Explicit version field (premature) |
| Sync `Tool::call` | B2 tools are fast enough sync. The async boundary is the API call in the turn loop, not tool dispatch. | Async trait (adds complexity for no current benefit) |
| `ToolResult::event` is `Option<AgentEvent>` | Internal tools return `None`, boundary tools emit events. Turn loop collects both without special-casing. | Separate `BoundaryTool` trait (unnecessary indirection) |

## How it fits together

```
ChatTurnRequest ──> [turn loop] ──> ChatTurnResult
                        │
                   ToolRegistry
                   ├── GetCurrentTime (internal, event: None)
                   ├── Calculate      (internal, event: None)
                   └── send_message   (boundary, event: Some(AgentEvent::Message))  ← C2
                        │
                   Vec<AgentEvent> ──> validate_turn_completion()
```

`chat/` defines the contract (types + validation). `agent/` implements the runtime (turn loop + tool dispatch). The boundary is explicit: `AgentEvent` is the shared vocabulary, `ToolResult::event` is the mechanism.

## Risks and bottlenecks

- **`make_tool_results` in `turn.rs` currently discards `ToolResult::event`**. This is correct for C1 (event collection comes in C2), but the event field is unused code until then. Acceptable trade-off — the registry API is designed for C2.
- **`final_message_count` is pub-exported but only used internally.** Keeping it public is intentional — consumers may want count-based inspection without full validation.
- **Linear tool lookup.** `ToolRegistry::dispatch` does a linear scan of `Vec<Box<dyn Tool>>`. Fine for 10 tools. If tool count grows to 50+, switch to a `HashMap`.

## What's not included

- Boundary tool implementations (`send_message`, `memory_edit`) — C2
- Event collection in the turn loop — C2
- Context tools (`context_read/write/delete/list`) — C3
- File + shell tools — C4
- JSONL wire format — C5
- Model abstraction — Later
