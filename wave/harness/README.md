# Harness

Two systems that work together: a **chat system** (the product — users send messages, see responses, manage memory) and an **agent harness** (the runtime — manages turns, tools, context windows, notifies consumers via events). Built in parallel to make the boundary tangible rather than theoretical. Both tracks are now proven (A2, B2 shipped); the remaining work is integration (B3).

The chat system is one consumer of the harness. The harness doesn't know about UI. The chat system doesn't know about context windows.

## Vision

Ship a clean product/runtime boundary where the chat system owns user experience + durable memory and the harness owns turn execution + context management. Integration should let waves run multiple agent invocations while preserving memory continuity without coupling the harness to UI concerns.

The boundary is defined by five components: agent harness, chat system, memory, context, and tools.

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

