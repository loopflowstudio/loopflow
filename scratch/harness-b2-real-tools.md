# B2: Real Tools

B1 proved the turn loop works. B2 makes it useful — real tools, event collection, JSONL output.

## What's done

**Foundation contract** (shipped): `AgentEvent` enum (Message, ToolCall, ToolResult, MemoryEdit, Done, Failed), `ChatTurnRequest`, `ChatTurnResult`, completion validation (`validate_turn_completion`). 22 tests. Lives in `chat/`.

**Tool registry** (C1, shipped): `Tool` trait + `ToolRegistry` in `agent/registry.rs`. `GetCurrentTime` and `Calculate` migrated to trait impls. Turn loop accepts `&ToolRegistry`. `ToolResult { output, event }` — internal tools return `event: None`, boundary tools will emit `AgentEvent`s.

Key design: events ride on tool results. The turn loop collects `ToolResult::event` without knowing which tools are boundary tools.

```
ChatTurnRequest ──> [turn loop] ──> ChatTurnResult
                        │
                   ToolRegistry
                   ├── GetCurrentTime (internal, event: None)
                   ├── Calculate      (internal, event: None)
                   └── send_message   (boundary, event: Some(AgentEvent::Message))  ← C2
                        │
                   Vec<AgentEvent> ──> validate_turn_completion()
```

Known risks:
- `make_tool_results` in `turn.rs` currently discards `ToolResult::event` — C2 fixes this
- Linear tool lookup in registry — fine for 10 tools, switch to HashMap at 50+

## What's left

C2-C5 add the remaining tools and wire up event collection:
- `send_message`, `memory_edit` (boundary tools that emit events)
- Context tools (in-memory named blocks with token counting)
- File + shell tools (ephemeral workspace)
- JSONL output + integration tests

## Approach

### Event collection (C2)

The turn loop collects `AgentEvent`s from `ToolResult::event` during execution. `TurnResult` gains `events: Vec<AgentEvent>`. Callers validate with `validate_turn_completion`, serialize to JSONL, or pass to the chat system.

### JSONL output (C5)

`lf-agent` emits events as JSONL to stdout, one event per line. stderr for diagnostics.

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

## Key decisions

**Context is a HashMap, not a Vec.** Named blocks with O(1) lookup. Token counting per block. The agent manages its own working memory without knowing about the chat system's memory format.

**Sync tools for B2.** Shell commands block with a 30s timeout. The turn loop is async for the API call; tool dispatch is sync within the loop. Add async tool dispatch in B3 if needed.

**JSONL to stdout, diagnostics to stderr.** Clean separation. Consumers pipe stdout. Humans read stderr.

**Workspace is a tempdir.** B3 will use git worktrees. Don't over-engineer now.

## Scope

Remaining:
- 9 tools: send_message, memory_edit, context_read/write/delete/list, read_file, write_file, shell
- Event collection in turn loop
- JSONL output in lf-agent
- Ephemeral workspace (tempdir)
- Tests for each tool + integration test

Out of scope:
- Model abstraction (Later)
- Persistent workspace / git worktrees (B3)
- Context compaction / summarization (Later)
- Streaming events during turn execution (Later)
- Chat system integration (A2/B3)

## Commit slices

### C1 — Tool registry + trait ✓

Shipped. `agent/registry.rs` with `Tool` trait, `ToolRegistry`, `ToolResult`. Existing tools migrated. Turn loop uses registry.

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
