# B2: Real Tools

## Problem

B1 proved the turn loop works: prompt → model → tool calls → dispatch → model → response. But the tools are toys (calculate, get_current_time). The harness can't do anything useful yet.

B2 adds the tools that make the harness a real runtime:
- `send_message` — the only way the agent talks to users
- `memory_edit` — the agent requests changes to persistent knowledge
- Context tools — the agent reads/writes its own in-memory context blocks
- File + shell tools — the agent interacts with an ephemeral workspace
- JSONL event emission — consumers see what happened

This is the step where the harness becomes useful to a consumer like the chat system.

## Approach

### Tool dispatch becomes extensible

B1 hardcoded two tools in `agent/tools.rs` with a `match` statement. B2 needs to support ~8-10 tools without the dispatch function becoming a mess. More importantly, some tools (send_message, memory_edit) cross the harness→consumer boundary — their results aren't computed locally, they're callbacks.

Introduce a `ToolRegistry` that the turn loop queries for definitions and dispatches through. Internal tools (calculate, file ops) return results directly. Boundary tools (send_message, memory_edit) invoke callbacks provided by the consumer.

```rust
// agent/registry.rs
pub struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>,
}

pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn definition(&self) -> ToolDefinition;
    fn call(&self, input: &serde_json::Value) -> ToolResult;
}

pub struct ToolResult {
    pub output: String,
    pub event: Option<AgentEvent>,  // emitted to consumers
}
```

The `event` field is the key design choice. When `send_message` is called, the tool returns a result to the model ("message sent") AND emits an `AgentEvent::Message` for consumers. This keeps the turn loop clean — it dispatches tools and collects events, without knowing which tools are boundary tools.

### Event collector in the turn loop

The turn loop currently returns a `TurnResult` with just the response text. Extend it to collect `AgentEvent`s during execution:

```rust
pub struct TurnResult {
    pub response: String,
    pub events: Vec<AgentEvent>,
    pub iterations: u32,
    pub input_tokens: u32,
    pub output_tokens: u32,
}
```

Events accumulate as tools execute. At the end, the caller can validate the event stream with `validate_turn_completion`, serialize to JSONL, or pass to a chat system.

### JSONL output

`lf-agent` emits events as JSONL to stdout, one event per line. The final text response is the last event (a `Message { phase: Final }`). stderr remains for diagnostics.

This is the wire format. Consumers parse JSONL, not the turn loop's internal state.

### The tools

#### send_message (boundary)

```
send_message({ content: "...", phase: "progress" | "final" })
```

Returns "message sent" to the model. Emits `AgentEvent::Message`. The completion contract (exactly one final) is validated post-hoc by the caller, not enforced inside the tool — the tool doesn't know if the turn will succeed or fail.

#### memory_edit (boundary)

```
memory_edit({ op: "upsert" | "delete", block: "block_name", detail: "..." })
```

Returns "edit recorded" to the model. Emits `AgentEvent::MemoryEdit`. The harness doesn't apply the edit — it records the request. The consumer decides what to do with it.

#### context_read (internal)

```
context_read({ block: "block_name" })
```

Reads a named block from the harness's in-memory context. Returns the block content or "not found".

Context blocks are a flat `HashMap<String, String>`. They're seeded from memory at session start and modified during the session. They're not persisted — they're the agent's working scratchpad.

#### context_write (internal)

```
context_write({ block: "block_name", content: "..." })
```

Writes/overwrites a named context block. Returns "written".

#### context_delete (internal)

```
context_delete({ block: "block_name" })
```

Deletes a context block. Returns "deleted" or "not found".

#### context_list (internal)

```
context_list({})
```

Returns a list of block names and their token counts.

#### read_file (internal)

```
read_file({ path: "relative/path.txt" })
```

Reads a file from the ephemeral workspace. Paths are relative to workspace root. Returns file content or error. No access outside the workspace.

#### write_file (internal)

```
write_file({ path: "relative/path.txt", content: "..." })
```

Writes a file to the ephemeral workspace. Creates parent directories. Returns "written".

#### shell (internal)

```
shell({ command: "cargo test" })
```

Runs a command in the ephemeral workspace. Returns stdout+stderr, truncated to a token budget. Times out after 30s.

### Workspace isolation

The ephemeral workspace is a temp directory. Files created during a session live there. The harness doesn't touch the real repo.

For B2, the workspace is a `tempdir()`. B3 will use git worktrees for real isolation. Don't over-engineer this now.

### Turn loop changes

The turn loop gains a `ToolRegistry` parameter. The `TurnConfig` grows to include workspace path and initial context blocks. The loop dispatches through the registry instead of the hardcoded `tools::dispatch`.

```rust
pub async fn run(
    prompt: &str,
    config: &TurnConfig,
    registry: &ToolRegistry,
) -> Result<TurnResult, TurnError>
```

B1's direct `tools::definitions()` and `tools::make_tool_results()` calls are replaced by registry methods.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Async tool trait | Enables async file/shell ops | Adds complexity. B2 tools are fast enough sync. Add async when we need streaming or long-running tools. |
| Tool middleware (pre/post hooks) | Could log, rate-limit, etc. | YAGNI. Add when there's a second use case. |
| Separate event channel (mpsc) | Decouples event emission from tool return | Over-engineering. The tool returning `Option<AgentEvent>` is simpler and sufficient. Events are collected synchronously in the turn loop. |
| Skip context tools, use file-based scratchpad | Simpler — just files | Context blocks are token-counted and in-memory. Files aren't. The agent needs to know how much context budget it's using. |
| Dynamic tool loading (plugins) | Extensible | Way too early. Hardcoded registry with trait objects is fine for 10 tools. |

## Key decisions

**Tool trait, not function pointers.** The `Tool` trait lets boundary tools carry state (callback closures). Internal tools are stateless but implement the same interface. This follows the roadmap principle: "Tool calls as the harness→consumer boundary."

**Events ride on tool results.** A tool returns both a model-facing result and an optional consumer-facing event. The turn loop collects both without knowing the difference. This keeps send_message and memory_edit from being special-cased in the loop.

**Context is a HashMap, not a Vec.** Named blocks with O(1) lookup. Token counting per block. The agent can manage its own working memory without knowing about the chat system's memory format.

**Sync tools for B2.** Shell commands block, but they have a 30s timeout and the turn loop is already async (for the API call). The tool dispatch itself is sync within the async loop. Add async tool dispatch in B3 if needed.

**JSONL to stdout, diagnostics to stderr.** Clean separation. Consumers pipe stdout. Humans read stderr.

## Scope

In scope:
- `ToolRegistry` + `Tool` trait
- 9 tools: send_message, memory_edit, context_read/write/delete/list, read_file, write_file, shell
- Event collection in turn loop
- JSONL output in lf-agent
- Ephemeral workspace (tempdir)
- Turn loop refactored to use registry
- Tests for each tool + integration test for turn loop with real tools

Out of scope:
- Model abstraction (Later)
- Persistent workspace / git worktrees (B3)
- Context compaction / summarization (Later)
- Streaming events during turn execution (Later — events are collected then emitted)
- Chat system integration (A2/B3)

## Commit slices

### C1 — Tool registry + trait (~200-300 LOC)

- `agent/registry.rs`: `Tool` trait, `ToolRegistry`, `ToolResult`
- Migrate `get_current_time` and `calculate` to trait impls
- Turn loop uses registry instead of hardcoded dispatch
- Existing `lf-agent` binary still works

### C2 — Boundary tools + event collection (~250-350 LOC)

- `send_message` tool (emits `AgentEvent::Message`)
- `memory_edit` tool (emits `AgentEvent::MemoryEdit`)
- Turn loop collects events from tool results
- `TurnResult` includes `events: Vec<AgentEvent>`

### C3 — Context tools (~200-300 LOC)

- `agent/context.rs`: `ContextStore` (HashMap wrapper with token counting)
- `context_read`, `context_write`, `context_delete`, `context_list` tools
- `TurnConfig` accepts initial context blocks

### C4 — File + shell tools (~200-300 LOC)

- `read_file`, `write_file` tools (workspace-scoped)
- `shell` tool (workspace-scoped, 30s timeout, output truncation)
- Workspace path in `TurnConfig`

### C5 — JSONL output + integration (~150-250 LOC)

- `lf-agent` emits JSONL events to stdout
- Integration test: turn loop with all tools, verify event stream
- Completion validation on the collected event stream

## Done when

```bash
cargo test -p loopflow agent
cargo test -p loopflow chat
```

All pass. Plus:

```bash
cargo run --bin lf-agent -- "Tell me hello, then remember my name is Alice"
```

Produces JSONL on stdout with `send_message` and `memory_edit` events. stderr shows tool dispatch diagnostics.
