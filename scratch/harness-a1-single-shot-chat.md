# A1 Chat + Memory (Consolidated)

## Status

A1 is implemented and working end-to-end in this branch. This doc now captures the **current contract** and the **remaining work for A2**.

## Current contract (what exists now)

### Chat UX

- Chat is a first-class tab in `WaveDetailPanel` (`Current`, `Runs`, `Chat`).
- Chat state is per-wave in `RepoState` (`wave_id` scoped).
- Message bubbles are session-only (not persisted after app restart).

### Model invocation

- Concerto calls Anthropic directly from Swift (`AnthropicClient`).
- Calls are single-shot: **current user message + memory blocks only**.
- Conversation history is display-only and is not sent back to the model.
- API key: `ANTHROPIC_API_KEY`; model override: `ANTHROPIC_MODEL`.

### Memory model

- Memory is persisted in lfd by `wave_id` (wave rename-safe).
- Memory is block-based (`name`, `content`, `position`, `updated_at`) in `chat_memory_blocks`.
- API surface:
  - `GET /v0/waves/:wave_id/memory-blocks`
  - `PUT /v0/waves/:wave_id/memory-blocks/:name`
  - `DELETE /v0/waves/:wave_id/memory-blocks/:name`
- Prompt formatting uses `<memory><block name="...">...</block></memory>`.

### Reliability and guardrails added

- Memory loading retries after transient failures.
- Ordering is deterministic (`position`, then `name`).
- Server-side append position uses `max(position) + 1`.
- Composer is disabled when Anthropic key is missing.
- Add-memory is blocked for empty names.

## Durable decisions to keep

- Memory ownership is in the chat system (lfd persistence), not in-harness.
- One chat + memory namespace per wave (`wave_id`), not global and not by wave name.
- Named memory blocks (not a single blob) to align with `memory_edit` direction.
- A1 intentionally uses direct Swift→Anthropic calls and is replaceable in A2.

## What is intentionally not in A1

- Harness turn loop / tool dispatch integration.
- Agent-driven memory writes (`memory_edit` apply path).
- Streaming responses.
- Persisted multi-turn transcript.
- Multiple chat threads per wave.

## Remaining work for A2 handoff

1. Replace direct Anthropic call path with harness-driven event consumption.
2. Wire harness `memory_edit` requests to chat memory persistence policy.
3. Keep current UX parity while switching backend path.
4. Decide whether chat transcript persistence is needed before/with A2.
5. Reassess ordering/reorder UX for long-lived memory block sets.

## Known risks

- Manual memory editing is still likely underused (expected A1 learning).
- Non-streaming responses can feel latent on longer completions.
- Position-only ordering may require explicit reorder operations as usage grows.
