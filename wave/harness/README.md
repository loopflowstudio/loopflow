# Harness Roadmap

Two systems that work together: a **chat system** (the product — users send messages, see responses, manage memory) and an **agent harness** (the runtime — manages turns, tools, context windows, notifies consumers via events). Built in parallel to make the boundary tangible rather than theoretical. Both tracks are now proven (A2, B2 shipped); the remaining work is integration (B3).

The chat system is one consumer of the harness. The harness doesn't know about UI. The chat system doesn't know about context windows.

## Vision

Ship a clean product/runtime boundary where the chat system owns user experience + durable memory and the harness owns turn execution + context management. Integration should let waves run multiple agent invocations while preserving memory continuity without coupling the harness to UI concerns.

The boundary is defined by five components: agent harness, chat system, memory, context, and tools (detailed in [Core components](#core-components)).

## Goals

### Invariants

- **`send_message` is the only user-output mechanism.** The harness produces user-visible output exclusively through explicit `send_message` tool calls, not by streaming raw LLM output.
- **Exactly one `send_message(phase="final")` on successful turns.** Zero or more `progress` messages, exactly one `final`. This is the completion contract.
- **Memory is durable across invocations.** The chat system persists memory. When a wave runs multiple agent sessions, memory carries forward.
- **Context is ephemeral within the harness.** The harness's message history lives in-memory for the session. It is not persisted. Token budgeting is a runtime concern.
- **Filesystem effects are ephemeral by default.** Agent file operations happen in an isolated workspace. Nothing is committed to the real repo without explicit action.
- **The harness doesn't know about the chat system.** It emits events and calls tools. Who's listening is not its concern.

### Completion criteria (B3 checkpoint)

A wave runs multiple agent invocations. Memory carries forward via the chat system. lfd orchestrates it.

## Risks

### Open questions (from B3)

- Should chat turn streams persist across lfd restarts? Currently in-memory only. For single-turn chat this is fine, but wave runs spanning hours may need bounded persistence for restart resilience.
- What's the approval UX for `memory_edit`? Auto-apply (current) vs. user confirms before persisting.

### What might change

- **In-memory turn streams** are the main operational risk for B3. A2's `HttpState.chat_turns` HashMap works for single turns but has no eviction. Wave runs with many invocations will accumulate unbounded state. Options: TTL-based eviction, bounded ring buffer, or persist to SQLite and evict from memory.
- **Memory ownership** held up through A2 — the chat system persists, the harness requests edits. But auto-applying `memory_edit` without user approval may not survive real usage. B3 should decide: always auto-apply, always confirm, or let the wave config choose.
- **Sync tool dispatch** works for B2 but shell commands that block for 30s may need async dispatch in B3. The `Tool` trait is sync (`fn call`) — changing to async would touch every tool impl.
- **The two-track approach collapses into one.** A2 and B2 are both shipped; B3 is where they converge. The separation was useful for building the boundary — both sides are now proven and the remaining work is integration, not parallel discovery.

## Metrics

- Successful turns emit exactly one final `send_message` event
- Memory blocks persist across invocations and seed subsequent turns
- Turn stream lifecycle remains bounded (no unbounded in-memory growth)
- Harness runtime stays UI-agnostic while chat UI remains harness-internal-state agnostic

## Roadmap

### Track A — Chat system

A simple chat product that calls LLMs, manages memory, shows conversation.

#### A1 — Single-shot chat ✓

Moved to `scratch/harness-a1-single-shot-chat.md` for implementation.

#### A2 — Multi-turn with harness events ✓

Shipped. Chat turns now route through lfd (`POST /waves/:wave_id/chat`) and stream events via SSE (`GET /waves/:wave_id/chat/events`). `ChatState` in Concerto is an event consumer/state machine (`idle → running → completed|failed`). Assistant text renders only from harness `send_message` events. Wave-scoped memory blocks persist in SQLite/Postgres with full CRUD APIs. `turn::run_with_event_handler()` emits `AgentEvent`s live during execution.

**What we learned:** The harness boundary feels right — routing through `send_message` events works as the sole UI output mechanism. The UX without streaming tokens is acceptable; the event-driven approach (progress → final) gives the UI enough to show activity. SSE is the right transport for real-time event delivery. The in-memory turn stream (`HttpState.chat_turns`) works for the happy path but has no TTL/cleanup — this needs addressing before B3 where multiple agent invocations will accumulate streams. Memory ownership confirmed: chat system persists, harness requests edits via tool calls — the boundary is clean.

### Track B — Agent harness

The runtime that runs LLM turn loops with tool dispatch.

#### B1 — A single turn, end to end ✓

Shipped. Turn loop calls Anthropic Messages API, dispatches tool calls via `ToolRegistry`, feeds results back, loops until text response or limit. `lf-agent` binary runs prompts from the CLI. Guardrails (max iterations, timeout) from day one.

**What we learned:** The turn loop is straightforward — async for the API call, sync tool dispatch within the loop. The foundation contract types (`AgentEvent`, `ChatTurnResult`, completion validation) fit cleanly as the event vocabulary. Adding a new tool takes ~30 LOC (implement `Tool` trait, register it). The `ToolResult { output, event }` design — where boundary tools emit events and internal tools return `None` — keeps the registry generic.

#### B2 — Real tools ✓

Shipped. Eleven tools across three tiers: boundary (`send_message`, `memory_edit`), context (read/write/delete/list with token counting), file/shell (ephemeral workspace with path traversal protection). Events ride on `ToolResult { output, event }` — boundary tools emit `AgentEvent`s, internal tools return `None`. JSONL output from `lf-agent`. Three-level registry: `default_registry()` (4) → `registry_with_context(store)` (8) → `full_registry(store, workspace)` (11).

**What we learned:** `send_message` as a tool works cleanly — the model calls it naturally and the completion contract (`exactly one final`) validates via the same event stream. Context management is simple: `HashMap<String, String>` with approximate token counting (`cl100k_base` via tiktoken-rs) is sufficient for budget visibility. Ephemeral workspace isolation via tempdir + path canonicalization works — both relative (`../`) and absolute path traversal are caught. Constructor injection for tool state (`Arc<Mutex<ContextStore>>`, `PathBuf`) keeps the `Tool::call(&self, input)` signature clean without needing a shared context object. The compress pass after implementation was valuable — merging `registry.rs` into `tools.rs` removed a file without losing clarity.

#### B3 — Wave integration

The two tracks converge. A2 shipped the plumbing (lfd orchestrates turns, SSE streams events, memory persists) — B3 wires this into the wave lifecycle so multiple agent invocations share memory across a wave run.

Remaining work:
- lfd seeds harness context from wave memory blocks at turn start
- Memory blocks carry forward across invocations within a wave run
- Turn stream lifecycle management (TTL/cleanup for in-memory `HttpState.chat_turns`)
- Approval flow for `memory_edit` events (currently auto-applied)

**Resolved questions:**
- What crosses the invocation boundary? Memory blocks — A2 shipped the persistence layer (`007_chat_memory_blocks.sql`) and CRUD APIs. The harness reads them at turn start, requests edits via tool calls.
- Does the harness need to know it's in a wave? No — confirmed by both B2 and A2. lfd provides memory and prompt; the harness runs a turn and emits events.

### Later

- Model abstraction (extract `Model` trait when we add a second provider)
- Compaction/summarization (when context gets too large within a session)
- MemGPT-style memory policies (evidence-based experiment harness)
- E2E hardening (chat turn lifecycle, SSE reconnect, memory consistency across failures)

## Core components

**Agent harness.** Runs turn loops: takes a prompt, calls a model, dispatches tool calls, feeds results back, loops until done. Manages its own context window in-memory. Emits structured events. Guardrails (max iterations, timeouts) from day one.

**Chat system.** The user-facing product. Receives events from the harness (via tool call callbacks like `send_message`). Displays conversation, lets users read/edit memory. Provides the harness with user input and memory state. Doesn't reach into harness internals.

**Memory.** Long-term knowledge that persists across invocations. Owned and displayed by the chat system. Provided to the harness as input — the harness reads it at session start to seed its context, and can request edits via tool calls. The chat system decides whether to apply those edits.

**Context.** The harness's in-memory working state during a session. A vec of messages, token-counted, managed by the harness. Seeded from memory at startup. Not persisted — it lives and dies with the session.

**Tools.** The harness dispatches tool calls from the model. Some tools are internal to the harness (file ops, shell). Some are provided by the consumer — `send_message` and `memory_edit` are tool calls that cross the harness→chat boundary.

## Design decisions

**The harness is a runtime, not a service.** Unlike Letta (where the agent is a persistent stateful service), the harness is a runtime that runs a session and exits. State crosses session boundaries via the consumer (chat system), not via the harness's own persistence. This keeps the harness simple and reusable.

**Memory belongs to the chat system, not the harness.** The harness can *request* memory edits via tool calls, but the chat system decides whether to apply them. The chat system is the authority on what gets remembered.

**Structured semantic events, not opaque streams.** The harness emits `AgentEvent`s (Message, ToolCall, ToolResult, MemoryEdit, Done, Failed) — not raw token deltas. The consumer sees what happened at a meaningful level.

**Tool calls as the harness→consumer boundary.** `send_message` and `memory_edit` aren't special internal operations — they're tool calls that the consumer handles. This makes the boundary explicit and extensible.

**No model SDK dependency.** Raw HTTP + serde for model calls. The adapter is isolated and thin.
