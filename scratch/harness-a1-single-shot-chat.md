# A1 — Single-shot chat

## Problem

We need to learn what the chat UX feels like — how memory should be displayed, whether users would actually edit it, and whether it's clear what the LLM "knows." This is pre-harness: direct LLM calls, no tool dispatch. The architecture must make swapping in the harness (A2) trivial.

The chat system is the user-facing product. It owns memory, displays conversation, and will eventually consume harness events. A1 builds the shell that A2 fills.

## What we expect to learn

We expect manual memory management to feel insufficient. That's the point.

Each API call sends only the current message + memory blocks. No conversation history goes to the model. The chat thread displays all messages (standard bubble UI), but the LLM is amnesic between turns — memory blocks are the only context that survives.

This is a forcing function: users must manually edit memory blocks for continuity. Most won't bother. They'll re-explain context each turn instead. The memory panel becomes furniture.

That's a valid A1 outcome. It tells us:

1. **Does the memory display feel right?** Can you see what the LLM knows at a glance?
2. **Does the editing UX work?** When you do reach for it, is add/edit/delete smooth?
3. **Is manual management enough?** Almost certainly not — which validates A2's agent-managed `memory_edit`.

The harness's `memory_edit` tool exists (schema, dispatch, event emission) but isn't wired to persistence yet. A1 validates the shell. A2 tests whether agent-managed memory makes it sing.

## Approach

Build a chat view inside Concerto as a new surface within the wave detail panel. A "Chat" button in the wave detail header (alongside Current/Runs) switches the detail area to the chat view. The chat displays messages as a standard bubble thread. Memory blocks are visible in a trailing panel, editable by the user, and injected into the system prompt on every call.

**Bold choice: one chat per wave.** Chat and memory are scoped 1:1 to a wave, keyed by `wave_id` (not wave name). Renaming a wave must not break chat continuity.

**Bold choice: memory as named blocks, not a single blob.** Memory is an ordered map of named blocks (e.g., "project-context", "preferences"). Each block is independently editable. This mirrors the harness's `ContextStore` (`HashMap<String, String>`) and the `memory_edit` tool contract (which operates on named blocks with `upsert`/`delete`).

**Bold choice: persist memory in lfd.** Concerto already uses lfd; memory should persist in lfd too. No global `~/.lf/chat-memory.json`.

**Bold choice: call the Anthropic API from Swift, not via `lf-agent`.** A1 is pre-harness. The whole point is to learn about the chat UX without the harness in the way. A thin Swift HTTP client calling `/v1/messages` directly is simpler than shelling out to a Rust binary and parsing JSONL. When A2 replaces the direct call with harness integration, this client gets deleted — it's intentionally throwaway.

**Bold choice: single-shot, not multi-turn API.** Each API call sends only the current user message + memory as system prompt. The conversation history is display-only.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Build as a separate app outside Concerto | Keeps Concerto focused on waves | Forces duplicate design system, auth config, window management. A2 integration requires them to merge anyway. |
| Use `lf-agent` binary for LLM calls | Reuses Rust harness immediately | Adds subprocess management, JSONL parsing, and the harness's tool/turn machinery — all of which A1 explicitly excludes. We'd be testing the harness, not the chat UX. |
| Single string memory (one big text block) | Simpler editing UX | Doesn't map to the harness's block-based `memory_edit` contract. A2 would need a migration. |
| Global file memory (`~/.lf/chat-memory.json`) | Fastest throwaway persistence | Wrong scope for wave-local chat, leaks across waves/repos, no multi-window consistency. |
| Streaming API responses | More responsive UX | Adds complexity (SSE parsing, partial render). A1 is single-shot — latency is acceptable for learning about memory UX. |
| Send full conversation history | More natural chat feel | Masks whether memory is useful — the LLM would "remember" things from context, not from memory blocks. |

## Key decisions

1. **Chat lives inside the wave detail panel.** A "Chat" button in the detail header (next to Current/Runs) switches the detail area to chat.

2. **Memory is `OrderedDictionary<String, String>` — named blocks with stable ordering.** Mirrors the harness contract while preserving UI order.

3. **Scope is by `wave_id` (1:1 with wave).** One chat and one memory namespace per wave.

4. **Display is multi-turn, API is single-shot.** The bubble chat shows conversation during the app session. The model sees only current message + memory blocks.

5. **Memory persistence goes through lfd.** Concerto reads/writes memory blocks via lfd HTTP APIs.

## Architecture

```
┌─────────────────────────────────────────────────────┐
│ Concerto Window                                     │
│ ┌──────────┬───────────────────────────────────────┐│
│ │ Wave     │ Detail Area                           ││
│ │ Sidebar  │                                       ││
│ │          │ [header: name | Current | Runs | Chat]││
│ │          │ ┌─────────────────┬──────────────────┐││
│ │          │ │ Chat            │ Memory Panel     │││
│ │          │ │                 │                  │││
│ │          │ │ bubble thread   │ named blocks     │││
│ │          │ │ (session only)  │ (persisted in    │││
│ │          │ │                 │  lfd by wave_id) │││
│ │          │ └─────────────────┴──────────────────┘││
│ └──────────┴───────────────────────────────────────┘│
└─────────────────────────────────────────────────────┘
```

### Swift types

```swift
import OrderedCollections

struct MemoryStore {
    var blocks: OrderedDictionary<String, String>

    static func load(waveId: String) async throws -> MemoryStore
    func upsert(waveId: String, name: String, content: String, position: Int?) async throws
    func delete(waveId: String, name: String) async throws

    func systemPrompt() -> String
}

@Observable
class ChatState {
    var messages: [ChatMessage] = []       // session-only; not persisted in A1
    var isLoading = false
    var memory = MemoryStore(blocks: [:])

    let waveId: String

    func loadMemory() async
    func send(_ text: String) async
}

struct ChatMessage: Identifiable {
    let id: UUID
    let role: Role  // .user, .assistant, .error
    let content: String
    let timestamp: Date
}

struct AnthropicClient {
    func complete(message: String, system: String) async throws -> String
}
```

### lfd persistence contract (memory blocks)

**Table (new):** `chat_memory_blocks`

- `wave_id TEXT NOT NULL`
- `name TEXT NOT NULL`
- `content TEXT NOT NULL`
- `position INTEGER NOT NULL`
- `updated_at INTEGER NOT NULL`
- `PRIMARY KEY (wave_id, name)`

Ordering in UI and prompt is by `position` ascending.

### lfd HTTP API (memory blocks)

- `GET /v0/waves/:wave_id/memory-blocks`
- `PUT /v0/waves/:wave_id/memory-blocks/:name` (upsert `{ content, position? }`)
- `DELETE /v0/waves/:wave_id/memory-blocks/:name`

(Full reorder/bulk replace is optional for A1.)

### Navigation

A "Chat" button appears in the `WaveDetailPanel` header alongside Current/Runs. Tapping it replaces detail content with chat. Tapping Current/Runs switches back.

### Memory in the system prompt

```xml
<memory>
<block name="project-context">
This project uses Swift and Rust. The harness is in rust/loopflow/.
</block>
<block name="preferences">
Use Lato for body text. Follow the 4pt spacing grid.
</block>
</memory>
```

### Error handling

- **Missing API key:** disable input with inline message.
- **Anthropic failure:** append `.error` message bubble.
- **lfd memory API failure:** keep UI responsive, surface inline error bubble, and allow retry.

## Scope

- **In scope:**
  - Chat view in `WaveDetailPanel`
  - One chat thread per wave (session-only)
  - Memory blocks per wave, persisted in lfd
  - Add/edit/delete memory blocks
  - Direct Anthropic call from Swift (single-shot)
  - `ANTHROPIC_API_KEY` from environment
  - Markdown rendering for assistant responses

- **Out of scope:**
  - Harness turn loop/tool dispatch
  - Streaming responses
  - Persisted conversation history
  - Multiple chats per wave
  - Agent-managed memory edits (`memory_edit` apply path in A2)

## Done when

1. Open a wave and click "Chat"
2. Add block `test = I like Swift`
3. Ask "What do you know about me?" and see memory reflected
4. Switch to a different wave and confirm memory/chat context is different
5. Switch back and confirm original wave memory still present
6. Rename wave and confirm memory still resolves (scope by `wave_id`)
7. Relaunch Concerto: memory blocks persist, conversation thread does not
