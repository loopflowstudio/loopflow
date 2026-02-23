# 01: Single-Shot Chat (Track A1) — Shipped

A simple chat product that calls LLMs, manages memory, shows conversation.

Moved to `scratch/harness-a1-single-shot-chat.md` for implementation.

## Core components (wave-wide)

**Agent harness.** Runs turn loops: takes a prompt, calls a model, dispatches tool calls, feeds results back, loops until done. Manages its own context window in-memory. Emits structured events. Guardrails (max iterations, timeouts) from day one.

**Chat system.** The user-facing product. Receives events from the harness (via tool call callbacks like `send_message`). Displays conversation, lets users read/edit memory. Provides the harness with user input and memory state. Doesn't reach into harness internals.

**Memory.** Long-term knowledge that persists across invocations. Owned and displayed by the chat system. Provided to the harness as input — the harness reads it at session start to seed its context, and can request edits via tool calls. The chat system decides whether to apply those edits.

**Context.** The harness's in-memory working state during a session. A vec of messages, token-counted, managed by the harness. Seeded from memory at startup. Not persisted — it lives and dies with the session.

**Tools.** The harness dispatches tool calls from the model. Some tools are internal to the harness (file ops, shell). Some are provided by the consumer — `send_message` and `memory_edit` are tool calls that cross the harness→chat boundary.

## Design decisions (wave-wide)

**The harness is a runtime, not a service.** Unlike Letta (where the agent is a persistent stateful service), the harness is a runtime that runs a session and exits. State crosses session boundaries via the consumer (chat system), not via the harness's own persistence. This keeps the harness simple and reusable.

**Memory belongs to the chat system, not the harness.** The harness can *request* memory edits via tool calls, but the chat system decides whether to apply them. The chat system is the authority on what gets remembered.

**Structured semantic events, not opaque streams.** The harness emits `AgentEvent`s (Message, ToolCall, ToolResult, MemoryEdit, Done, Failed) — not raw token deltas. The consumer sees what happened at a meaningful level.

**Tool calls as the harness→consumer boundary.** `send_message` and `memory_edit` aren't special internal operations — they're tool calls that the consumer handles. This makes the boundary explicit and extensible.

**No model SDK dependency.** Raw HTTP + serde for model calls. The adapter is isolated and thin.
