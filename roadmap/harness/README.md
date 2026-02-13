# Harness Roadmap

Two systems that work together: a **chat system** (the product — users send messages, see responses, manage memory) and an **agent harness** (the runtime — manages turns, tools, context windows, notifies consumers via events). Building both in parallel makes the boundary between them tangible rather than theoretical.

The chat system is one consumer of the harness. The harness doesn't know about UI. The chat system doesn't know about context windows.

## Core components

**Agent harness.** Runs turn loops: takes a prompt, calls a model, dispatches tool calls, feeds results back, loops until done. Manages its own context window in-memory. Emits structured events. Guardrails (max iterations, timeouts) from day one.

**Chat system.** The user-facing product. Receives events from the harness (via tool call callbacks like `send_message`). Displays conversation, lets users read/edit memory. Provides the harness with user input and memory state. Doesn't reach into harness internals.

**Memory.** Long-term knowledge that persists across invocations. Owned and displayed by the chat system. Provided to the harness as input — the harness reads it at session start to seed its context, and can request edits via tool calls. The chat system decides whether to apply those edits.

**Context.** The harness's in-memory working state during a session. A vec of messages, token-counted, managed by the harness. Seeded from memory at startup. Not persisted — it lives and dies with the session.

**Tools.** The harness dispatches tool calls from the model. Some tools are internal to the harness (file ops, shell). Some are provided by the consumer — `send_message` and `memory_edit` are tool calls that cross the harness→chat boundary.

## Invariants

- **`send_message` is the only user-output mechanism.** The harness produces user-visible output exclusively through explicit `send_message` tool calls, not by streaming raw LLM output.
- **Exactly one `send_message(phase="final")` on successful turns.** Zero or more `progress` messages, exactly one `final`. This is the completion contract.
- **Memory is durable across invocations.** The chat system persists memory. When a wave runs multiple agent sessions, memory carries forward.
- **Context is ephemeral within the harness.** The harness's message history lives in-memory for the session. It is not persisted. Token budgeting is a runtime concern.
- **Filesystem effects are ephemeral by default.** Agent file operations happen in an isolated workspace. Nothing is committed to the real repo without explicit action.
- **The harness doesn't know about the chat system.** It emits events and calls tools. Who's listening is not its concern.

## Design decisions

**The harness is a runtime, not a service.** Unlike Letta (where the agent is a persistent stateful service), the harness is a runtime that runs a session and exits. State crosses session boundaries via the consumer (chat system), not via the harness's own persistence. This keeps the harness simple and reusable.

**Memory belongs to the chat system, not the harness.** The harness can *request* memory edits via tool calls, but the chat system decides whether to apply them. The chat system is the authority on what gets remembered.

**Structured semantic events, not opaque streams.** The harness emits `AgentEvent`s (Message, ToolCall, ToolResult, MemoryEdit, Done, Failed) — not raw token deltas. The consumer sees what happened at a meaningful level.

**Tool calls as the harness→consumer boundary.** `send_message` and `memory_edit` aren't special internal operations — they're tool calls that the consumer handles. This makes the boundary explicit and extensible.

**No model SDK dependency.** Raw HTTP + serde for model calls. The adapter is isolated and thin.

## Two tracks

### Track A — Chat system

A simple chat product that calls LLMs, manages memory, shows conversation.

#### A1 — Single-shot chat

The simplest possible chat: user sends a message, LLM responds, response is displayed. Memory exists and is included in the prompt, but only edited manually by the user. No agent loop, no tools.

**What we'll learn:** What the chat UX feels like. How memory should be displayed and edited.

**Checkpoint:** A user can send a message, see a response, read/edit memory, and have that memory included in the next prompt.

**Try it:**
- Verification: send a message, get a response, edit a memory block, send another message, confirm the memory is in the prompt
- Feel: does the memory feel useful? Is it clear what the LLM "knows"? Would you actually edit it?

#### A2 — Multi-turn with harness events

Replace the direct LLM call with the agent harness. The chat system receives `send_message` events instead of raw responses. Memory edits come as tool call requests from the harness.

**What we'll learn:** Whether the harness boundary feels right. Whether `send_message` as a tool call (vs streaming) is the right UX.

**Checkpoint:** Same UX as A1, but now powered by the harness underneath.

### Track B — Agent harness

The runtime that runs LLM turn loops with tool dispatch.

#### B1 — A single turn, end to end

Call Anthropic Messages API directly. Handle a tool call: model requests a tool, harness dispatches it, feeds the result back, model responds. One concrete tool for testing. In-memory message vec. Guardrails (max iterations, timeout).

No traits, no abstractions. Just the working thing.

**What we'll learn:** What the turn loop actually needs. Whether the foundation contract types fit or need revision.

**Checkpoint:** Run `lf-agent` with a prompt, get a response, see a tool call dispatched and result fed back.

**Try it:**
- Verification: `cargo run -p lf-agent -- "what's 2+2?"` produces a response
- Feel: read the code. Is the turn loop obvious? Could you add a new tool in 5 minutes?

#### B2 — Real tools

- `send_message` (progress/final phases) — the harness→consumer output channel
- Context tools (read/edit/delete blocks, in-memory)
- File + shell tools (ephemeral workspace)
- Event emission (JSONL to stdout)

**What we'll learn:** Whether `send_message` as a tool is the right interface. What context management actually needs. How ephemeral workspace isolation works.

**Checkpoint:** Agent runs a multi-turn session with real tools. Events stream as JSONL.

**Try it:**
- Verification: `cargo test -p lf-agent tools turn_loop` — all pass
- Feel: run the agent against a real prompt with real files. Does `send_message(progress)` vs `send_message(final)` make sense?

#### B3 — Wave integration

The two tracks converge:
- The chat system provides memory to the harness at session start
- The harness emits `memory_edit` tool calls back to the chat system
- lfd spawns and monitors agent processes
- Events stream to consumers

**Open questions (resolve during B2):**
- What exactly crosses the invocation boundary? Memory blocks only? Or also a summary?
- Does the harness need to know it's in a wave, or is it always just "run this prompt with these tools and this memory"?

**Checkpoint:** A wave runs multiple agent invocations. Memory carries forward via the chat system. lfd orchestrates it.

### Later

- Model abstraction (extract `Model` trait when we add a second provider)
- Compaction/summarization (when context gets too large within a session)
- MemGPT-style memory policies (evidence-based experiment harness)
- Swift viewer UI
- E2E hardening

## What might change

- **`send_message` as a tool call** might feel wrong once we build A2. If the chat UX needs streaming tokens, the tool-call-only model breaks.
- **Memory ownership** (chat system vs harness) might need to shift if the agent needs to update memory autonomously.
- **Track A and Track B sequencing** will depend on which surface is fastest to iterate on.
- **The two-track approach itself** might collapse into one track if we discover the boundary is in the wrong place.
