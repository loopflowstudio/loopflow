# Chat Harness for Waves — Current Design and Execution Plan

This document is the active design source for wave chat work on this branch.

## Objective

Ship a memory-first chat runtime for waves where:

- memory is durable across turns,
- filesystem effects are ephemeral by default,
- history is token-bounded,
- user-visible output is explicit `send_message` tool calls,
- successful turns emit exactly one `phase="final"` message (with `0..∞` progress messages allowed).

## Current state (February 13, 2026)

### Already implemented in this branch

1. Chat contract module in `rust/loopflow/src/chat/`:
   - request/response types (`ChatTurnRequest`, `ChatTurnResult`)
   - event schema (`AgentEvent`)
   - explicit `UserMessagePhase` + `SendMessageArgs`
2. Completion helpers:
   - `is_user_message`
   - `final_message_count`
   - `validate_turn_completion` (exactly one final message)
3. Parsing/validation entry point:
   - `parse_send_message_args`
4. Contract-focused tests for serde round-trips and completion edge cases.
5. Numbered roadmap under `roadmap/harness/01-11`.

### Not implemented yet

- `lf-agent` runtime process and loop
- model integration (Anthropic adapter/runtime wiring)
- persistence + lfd chat endpoints
- process spawning/integration lane for chat turns
- Python chat client surface
- Swift chat viewer surface
- compaction implementation
- hardening + e2e verification for full slice

## Locked product/runtime invariants

- Chat always runs in a wave context.
- Memory writes persist and survive turn boundaries.
- Filesystem writes are non-durable by default (ephemeral turn workspace behavior).
- Prompt context includes: system prompt + memory + token-bounded harness history + current user message.
- `send_message` is the only user-output path.
- Completion requires one and only one final message for successful turns.
- Progress messages are first-class explicit events (`phase="progress"`), not inferred status text.

## Architecture target

```text
Client (Python for local testing, Swift for product UI)
   -> lfd chat API/process management
      -> lf-agent process (model + tools + loop)
         -> tool calls (memory + wave tools)
         -> JSONL events streamed back to lfd
```

## Ordered implementation plan (source of truth)

Roadmap files in `roadmap/harness/` are the execution sequence:

1. Foundation contract ✅
2. Persistence + token history
3. lf-agent skeleton
4. Anthropic model adapter
5. Tools + turn loop
6. lfd process integration
7. Python local client
8. Swift client + memory viewer UI
9. MemGPT/Letta exploration pass
10. Compaction rollout
11. Hardening + e2e

## Scope boundaries

### In scope for v1

- Anthropic-first model support (other providers later)
- memory tools + wave tools (`read_file`, `write_file`, `shell`)
- explicit progress/final message streaming
- manual compaction endpoint/workflow

### Out of scope for v1

- streaming partial model tokens
- OpenAI/Gemini providers
- MCP tool integration
- auto-compaction policies
- implicit persistent branch mutation flows

## Durability and safety model

- Durable: memory blocks, chat message logs, context snapshots.
- Ephemeral by default: worktree mutations and shell side effects during a turn.
- Guardrails required in runtime phase:
  - max iteration cap per turn
  - wall-clock timeout per turn
  - clear failure path when final message contract is not satisfied

## Open technical decisions to resolve during upcoming slices

1. Whether placeholder contract shapes (`MemoryEditLog`, `ToolCallLog`, `ContextSnapshot`) need wire changes before persistence/API lock-in.
2. Failure closure behavior when the agent exits before emitting a final message (raw error only vs synthetic user-facing final error message).
3. Exact persistence semantics for memory edit durability (per tool call transaction boundaries).

## Immediate next build target

Implement roadmap item **02-persistence-token-history**:

- SQLite schema for chat memory + message log
- repository APIs for loading token-bounded history
- persistence tests that enforce durability and retrieval invariants

Keep contracts from step 01 stable unless a hard wire-compatibility issue is discovered.
