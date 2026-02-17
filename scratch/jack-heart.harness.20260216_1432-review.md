# A1 Single-Shot Chat — Design Review

## What was implemented

Added a chat tab to WaveDetailPanel that lets users send single-shot messages to Anthropic with per-wave memory blocks as context. Memory blocks are CRUD-managed through lfd (persisted in SQLite/Postgres) and sent as XML-formatted system prompt on every request.

### Rust (lfd server)

- New `chat_memory_blocks` table (migration 006) with `(wave_id, name)` primary key, `position` ordering, `ON DELETE CASCADE` from waves.
- Three `RunStore` trait methods: `list_chat_memory_blocks`, `upsert_chat_memory_block`, `delete_chat_memory_block` — implemented for both SQLite and Postgres.
- Three HTTP endpoints: `GET /waves/:wave_id/memory-blocks`, `PUT /waves/:wave_id/memory-blocks/:name`, `DELETE /waves/:wave_id/memory-blocks/:name`.
- Server-side append position: new blocks default to `max(position) + 1`; existing blocks keep their position on content-only updates.
- Input validation: block names are trimmed, empty names rejected with 400.
- `extract_text_or_final_event` in `turn.rs`: fallback for turns where the model produces no text content block but a `Final` event exists.

### Swift (Concerto app)

- `ChatMemoryBlock` model in LoopflowCore (Identifiable, Sendable, Hashable).
- `AnthropicClient`: direct HTTP client for Anthropic Messages API, reads `ANTHROPIC_API_KEY` and `ANTHROPIC_MODEL` from env.
- `ChatState`: MainActor-isolated Observable that manages messages, memory loading, send/upsert/rename/delete flows.
- `MemoryStore`: value-type memory cache with position-based sorting and XML system prompt generation.
- `WaveChatView`: HSplitView with chat thread (left) and memory editor panel (right).
- `WaveDetailPanel`: new "Chat" tab alongside "Current" and "Runs".
- `ChatMemoryService` protocol + `LocalWaveService` conformance for memory block API calls.
- `ChatStateTests`: memory prompt ordering/escaping, missing API key error bubble, load-retry behavior.

## Key choices

**Direct Anthropic calls (not harness).** A1 intentionally bypasses the agent harness. Single-shot HTTP → model, no turn loop, no tools. This is throwaway plumbing — A2 replaces it with harness events. The alternative (building harness integration first) would have delayed getting the chat UX in front of users.

**Memory as named blocks, not a single blob.** Aligns with the `memory_edit` tool design in Track B. Each block has a name, content, and position. The model doesn't see block metadata — just `<memory><block name="...">content</block></memory>` in the system prompt.

**Session-only messages.** Chat transcript is not persisted. Messages live in `ChatState` and are lost on app restart. This is intentional for A1 — transcript persistence is an A2 question.

**`ChatMemoryService` protocol for testability.** Decouples `ChatState` from `LocalWaveService`, enabling mock-based tests without network calls.

**XML escaping in system prompt.** Content is escaped (`&`, `<`, `>`, `"`, `'`) to prevent memory block content from breaking the XML structure. Block names are also escaped.

## How it fits together

```
WaveDetailPanel → WaveChatView → ChatState → AnthropicClient (HTTP)
                                           → LocalWaveService → lfd HTTP API → SQLite/Postgres
```

`RepoState` owns a `[String: ChatState]` cache keyed by wave ID, lazily creating states on first access. Memory loads on tab open (`loadMemoryIfNeeded`). Failed loads allow retry on next tab visit.

## Risks and bottlenecks

- **Non-streaming responses.** Longer completions will feel slow — no progress indication beyond the spinner. A2 will use harness events which can stream.
- **1024 max_tokens.** Hardcoded in `AnthropicClient`. Fine for A1's exploratory use, but may truncate longer responses.
- **No rate limiting or request cancellation.** Rapid sends could stack up requests. `isLoading` prevents concurrent sends from the UI, but there's no Task cancellation if the user navigates away.

## What's not included

- Harness integration (A2).
- Agent-driven memory writes (`memory_edit` tool).
- Streaming responses.
- Persisted chat transcript.
- Multiple chat threads per wave.
- Memory block reordering UI (drag-and-drop).

## Gate polish applied

- Removed redundant client-side sort in `listMemoryBlocks` — server already returns blocks in `(position, name)` order.
- Ran `cargo fmt` to fix import line wrapping in files touched by this branch.
- All Rust tests pass (clippy clean, no warnings).
- All Swift tests pass (108 tests, 20 suites).
