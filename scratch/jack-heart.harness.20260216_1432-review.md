# Wave Chat: A1 Single-Shot Chat

## What was implemented

Chat tab on waves in Concerto with persisted memory blocks. Three layers:

1. **Backend (Rust)**: `chat_memory_blocks` table with CRUD API — `GET/PUT/DELETE` on `/waves/:id/memory-blocks/:name`. Migration 006. Position-ordered, wave-scoped, cascading on wave delete.

2. **Swift core**: `ChatMemoryBlock` model in LoopflowCore. `LocalWaveService` gains `listMemoryBlocks`, `upsertMemoryBlock`, `deleteMemoryBlock`.

3. **Concerto UI**: `WaveChatView` (chat thread + memory panel), `ChatState` (message history, memory store, Anthropic API calls), `AnthropicClient` (direct Messages API integration). New "Chat" tab in `WaveDetailPanel`.

Also on this branch: `lf ops lint` / `lf ops test` commands, config fields `lint:` / `test:` replacing `lint_check:`, simplified `ops/lint.rs` to use configured commands instead of hard-coded ruff.

Also: `extract_text_or_final_event` in `agent/turn.rs` — falls back to a single `Final` event when the model returns no text content blocks.

## Key choices

- **Direct Anthropic API** rather than routing through `lfd`. Chat is user-initiated, single-shot, no agent loop needed. Memory blocks are the only persisted state — conversation history is ephemeral (session-scoped in SwiftUI).
- **Memory as XML in system prompt**. Blocks are XML-escaped and wrapped in `<memory><block name="...">` tags. Simple, transparent to the user, easy to debug.
- **Position-based ordering** for memory blocks. Tie-breaks on name. Upsert preserves existing position by default; new blocks append after the highest.
- **`ChatMemoryService` protocol** decouples `ChatState` from `LocalWaveService`, enabling test mocking via actor-based `MockChatMemoryService`.
- **`lint_check` renamed to `lint`**, `test` added. Both are plain shell commands run via `sh -c`. `lf ops lint` and `lf ops test` expose them from CLI.

## How it fits together

```
WaveDetailPanel
  └── Chat tab → WaveChatView
       ├── ChatState (message list, Anthropic calls)
       │    ├── AnthropicClient (ANTHROPIC_API_KEY from env)
       │    └── ChatMemoryService → LocalWaveService
       │         └── lfd HTTP API → RunStore (SQLite/Postgres)
       └── MemoryStore (sorted blocks → XML system prompt)
```

`RepoState` caches `ChatState` per wave ID so tab switches don't lose conversation history.

## Risks and bottlenecks

- **API key in environment only.** No keychain integration or settings UI. Users must launch Concerto from a shell with `ANTHROPIC_API_KEY` set.
- **No streaming.** Single-shot request/response. Long responses will feel slow. A2 (multi-turn with harness events) would address this.
- **No conversation persistence.** Messages are lost on app restart. Memory blocks persist; messages don't.
- **1024 max_tokens default.** Hardcoded in `AnthropicClient`. May truncate longer responses.

## What's not included

- Multi-turn conversation history (only memory blocks carry context between turns)
- Streaming responses
- Tool use / agent loop
- Conversation persistence to disk or server
- API key management UI
