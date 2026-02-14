# 01: Foundation Contract

Lock the chat/runtime contract before heavy implementation so all layers agree on turn semantics.

## What exists after this

A shared contract for:

- turn request/response payloads
- `send_message` phases (`progress`, `final`)
- JSONL agent events
- completion rule (exactly one final message on success)
- chat lane snapshot metadata (branch + head sha at turn start)

No model implementation yet. This is type + validation + tests.

## Commit slices

### C1 — Add chat contract types (Rust, ~250-400 LOC)

- Add `ChatTurnRequest`, `ChatTurnResult`, `SendMessageArgs`, `UserMessagePhase`
- Add `AgentEvent` schema (message/tool_call/tool_result/memory_edit/done/failed)
- Add `WorkspaceSnapshot { branch, head_sha_at_start }`

### C2 — Add validation helpers (~200-350 LOC)

- `validate_turn_completion(events) -> Result<()>`
- enforce exactly one final message on successful turns
- enforce phase enum parsing and schema validation

### C3 — Add contract tests + fixtures (~250-450 LOC)

- serde round-trip tests
- completion rule tests
- invalid payload coverage (missing phase, multiple finals, no final)

## Constraints

- Keep schemas stable and versionable (client-facing).
- Keep phase semantics explicit; do not infer from text.
- Do not bind contract types to Anthropic/OpenAI response shapes.

## Done when

```bash
cargo test -p loopflow chat_contract
```

Expected: all contract tests pass.

## Current state

`chat/contract.rs` already has C1 types: `UserMessagePhase`, `SendMessageArgs`, `WorkspaceSnapshot`, `ChatTurnRequest`, `ChatTurnResult`, `AgentEvent`, `ContextSnapshot`, `MemoryEditLog`, `ToolCallLog`, and `parse_send_message_args`.

Remaining work:
- **C2**: `validate_turn_completion(events) -> Result<()>` — enforce exactly one `Message { phase: Final }` among events, enforce no `Final` on failed turns
- **C3**: serde round-trip tests, completion rule tests, invalid payload coverage
