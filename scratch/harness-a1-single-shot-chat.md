# A1 — Single-shot chat

## Problem

We need to learn what the chat UX feels like — how memory should be displayed, whether users would actually edit it, and whether it's clear what the LLM "knows." This is pre-harness: direct LLM calls, no tool dispatch. The architecture must make swapping in the harness (A2) trivial.

The chat system is the user-facing product. It owns memory, displays conversation, and will eventually consume harness events. A1 builds the shell that A2 fills.

## Approach

Build a chat view inside Concerto (the existing macOS app) as a new top-level surface alongside the wave sidebar. The chat is a single-turn exchange: user types a message, the app calls the Anthropic Messages API directly, displays the response. Memory blocks are visible in a side panel, editable by the user, and injected into the system prompt on every call.

**Bold choice: memory as named blocks, not a single blob.** Memory is a `[String: String]` dictionary of named blocks (e.g., "project-context", "preferences", "recent-decisions"). Each block is independently editable. This mirrors the harness's `ContextStore` (`HashMap<String, String>`) and the `memory_edit` tool contract (which operates on named blocks with `upsert`/`delete`). When A2 arrives, the harness can request edits to specific blocks and the chat system applies or rejects them — the data model is already right.

**Bold choice: call the Anthropic API from Swift, not via `lf-agent`.** A1 is pre-harness. The whole point is to learn about the chat UX without the harness in the way. A thin Swift HTTP client calling `/v1/messages` directly is simpler than shelling out to a Rust binary and parsing JSONL. When A2 replaces the direct call with harness integration, this client gets deleted — it's intentionally throwaway.

**Bold choice: memory persists as a JSON file, not SQLite.** At A1 scale (one user, manual edits only), a `~/.lf/chat-memory.json` file is the simplest durable store. It's human-readable, debuggable with `cat`, and trivially replaceable when A2 introduces a real persistence layer.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Build as a separate app outside Concerto | Keeps Concerto focused on waves | Forces duplicate design system, auth config, window management. A2 integration requires them to merge anyway. |
| Use `lf-agent` binary for LLM calls | Reuses Rust harness immediately | Adds subprocess management, JSONL parsing, and the harness's tool/turn machinery — all of which A1 explicitly excludes. We'd be testing the harness, not the chat UX. |
| Single string memory (one big text block) | Simpler editing UX | Doesn't map to the harness's block-based `memory_edit` contract. A2 would need a migration. Named blocks also let users see what the LLM "knows" at a glance. |
| SQLite for memory | More robust persistence | Overkill for manual edits by one user. JSON is inspectable without tooling. |
| Streaming API responses | More responsive UX | Adds complexity (SSE parsing, partial render). A1 is single-shot — latency is acceptable for learning about the memory UX, which is the real goal. |

## Key decisions

1. **Chat lives inside Concerto as a peer to the wave view.** The harness roadmap says "the chat system is the user-facing product" and Concerto is that product. Adding a chat surface to Concerto — not building a separate app — follows the wave principle that "the harness doesn't know about UI."

2. **Memory is `[String: String]` — named blocks.** This directly mirrors the harness contract: `MemoryEdit { op, block, detail }` operates on named blocks. The `ContextStore` is `HashMap<String, String>`. Choosing the same shape means A2 is a wiring change, not a data model migration. Quote from harness README: "Memory is long-term knowledge that persists across invocations. Owned and displayed by the chat system. Provided to the harness as input."

3. **No streaming, no tools, no turn loop.** A1 is a single request/response. The Anthropic Messages API returns a complete response. This isolates the variable we're testing (memory UX) from variables we're not (streaming feel, tool dispatch). A2 tests those.

4. **The API client is intentionally throwaway.** A thin `AnthropicClient` in Swift that calls `/v1/messages` with a system prompt containing memory blocks. When A2 arrives, this gets replaced by harness event consumption. No abstraction layer, no protocol — just a struct with one async method.

5. **Memory panel is always visible, not hidden behind a toggle.** The point of A1 is to learn whether memory is useful and whether users would edit it. Hiding it defeats the experiment. The panel shows each block's name and content, with inline editing and add/delete. This follows the Concerto design principle of transparency: "Show plans before execution."

## Architecture

```
┌─────────────────────────────────────────────────┐
│ Concerto Window                                 │
│ ┌──────────┬──────────────────┬────────────────┐│
│ │ Wave     │ Chat View        │ Memory Panel   ││
│ │ Sidebar  │                  │                ││
│ │          │ ┌──────────────┐ │ [project-ctx]  ││
│ │          │ │ Messages     │ │ "This is a..." ││
│ │          │ │              │ │                ││
│ │          │ │ User: hello  │ │ [preferences]  ││
│ │          │ │ AI: Hi! I... │ │ "Use Swift..." ││
│ │          │ │              │ │                ││
│ │          │ └──────────────┘ │ [+ Add block]  ││
│ │          │ ┌──────────────┐ │                ││
│ │          │ │ 💬 Type...   │ │                ││
│ │          │ └──────────────┘ │                ││
│ └──────────┴──────────────────┴────────────────┘│
└─────────────────────────────────────────────────┘
```

### Swift types

```swift
// Memory — persisted as JSON
struct MemoryStore {
    var blocks: [String: String]  // name → content

    func systemPrompt() -> String  // renders blocks into system prompt
    mutating func upsert(block: String, content: String)
    mutating func delete(block: String)
    func save()  // writes to ~/.lf/chat-memory.json
    static func load() -> MemoryStore  // reads from disk
}

// Chat state
@Observable
class ChatState {
    var messages: [ChatMessage] = []
    var isLoading = false
    var memory = MemoryStore.load()

    func send(_ text: String) async  // calls Anthropic API, appends result
}

struct ChatMessage: Identifiable {
    let id: UUID
    let role: Role  // .user or .assistant
    let content: String
    let timestamp: Date
}

// Throwaway API client
struct AnthropicClient {
    func complete(messages: [ChatMessage], system: String) async throws -> String
}
```

### Navigation

The chat view appears when no wave is selected (replacing the current `StartWaveView`) or via a dedicated tab/button. This makes chat discoverable without disrupting the wave workflow that Concerto already provides.

### Memory in the system prompt

Memory blocks are rendered into the system prompt as labeled sections:

```
<memory>
<block name="project-context">
This project uses Swift and Rust. The harness is in rust/loopflow/.
</block>
<block name="preferences">
Use Lato for body text. Follow the 4pt spacing grid.
</block>
</memory>
```

This format is parseable (for A2 when the harness needs to read it back) and readable (for debugging). It follows the existing pattern in the harness where context blocks are named and individually addressable.

## Scope

- **In scope:**
  - Chat view in Concerto with message list and input field
  - Memory panel showing named blocks with inline editing (add, edit, delete)
  - Direct Anthropic Messages API call from Swift (no streaming)
  - Memory persistence as `~/.lf/chat-memory.json`
  - Conversation history within a session (not persisted across app restarts)
  - `ANTHROPIC_API_KEY` from environment variable (same as Rust harness)
  - Markdown rendering for assistant responses

- **Out of scope:**
  - Streaming responses (A2)
  - Tool dispatch / agent loop (A2+)
  - Harness integration (A2)
  - Conversation persistence across sessions
  - Multiple conversations
  - Memory auto-editing by the LLM (A2 via `memory_edit` tool)
  - Model selection UI (use `claude-sonnet-4-5-20250929` like the harness)

## Done when

1. Launch Concerto, see a chat view with an empty message area and a memory panel
2. Add a memory block called "test" with content "I like Swift"
3. Type "What do you know about me?" and press Enter
4. See a response that references the memory content
5. Edit the memory block, send another message, confirm the updated memory appears in the response
6. Quit and relaunch — memory blocks persist, conversation does not
7. Verify `~/.lf/chat-memory.json` contains the blocks as expected
