# A1 chat + memory persistence review

## What was implemented

- Added A1 chat UX in Concerto (`WaveChatView`) and a new Chat tab in `WaveDetailPanel` with per-wave `ChatState` instances in `RepoState`.
- Added direct Anthropic single-shot completion support (`AnthropicClient`) using `ANTHROPIC_API_KEY` and optional `ANTHROPIC_MODEL`.
- Added wave-scoped memory block persistence end-to-end:
  - New `chat_memory_blocks` storage migration and `ChatMemoryBlock` type in lfd.
  - Store CRUD in SQLite/Postgres via `RunStore` trait.
  - New HTTP routes: list/upsert/delete memory blocks under `/v0/waves/:wave_id/memory-blocks`.
  - Swift `LocalWaveService` + protocol updates for memory APIs.
- Added resilience improvements in this polish pass:
  - Memory load retries if first load fails (instead of permanently latching failed state).
  - Deterministic block ordering for equal positions (position, then name).
  - Default server append position now uses max position + 1, not item count.
  - Composer disabled when API key missing and add-memory blocked for empty names.
- Added/expanded tests:
  - Rust turn fallback tests and memory position/name helper tests.
  - Swift `ChatState` tests for prompt formatting, missing key behavior, and memory retry behavior.

## Key choices

- **Single-shot model calls with memory-only context:** conversation remains UI-only; only current prompt + memory is sent to the model.
- **Memory scoped by `wave_id`:** wave renames do not break continuity.
- **Named memory blocks instead of one blob:** aligns with harness `memory_edit` direction for A2.
- **Direct Swift→Anthropic call for A1:** avoids premature harness coupling; this path is intentionally replaceable.
- **Retry-on-failure memory load:** better UX for transient lfd startup/network failures without forcing app restart.

## How it fits together

Concerto owns chat session UI state per wave (`RepoState` → `ChatState`) and renders bubbles + memory editing in `WaveChatView`. Memory operations flow through `LocalWaveService` to new lfd HTTP routes, which persist to `chat_memory_blocks` in SQLite/Postgres. Message sends call Anthropic directly with a generated `<memory>` system prompt from sorted memory blocks.

## Risks and bottlenecks

- **Manual memory management remains a product risk:** expected for A1, but users may ignore memory editing.
- **Anthropic call is non-streaming:** acceptable for A1 but latency may feel slow on long responses.
- **UI test environment coupling:** full `xcodebuild test` can fail when macOS local authentication prompts interfere with UI test runner initialization.
- **Position-based ordering can still drift over many edits:** deterministic sorting helps, but no dedicated reorder API exists yet.

## What's not included

- Harness turn loop/tool dispatch integration.
- Persisted multi-turn conversation history.
- Agent-managed memory edits (`memory_edit` application path).
- Streaming responses.
- Multi-chat threads per wave.
