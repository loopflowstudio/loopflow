# B2: Real Tools

## Problem

B1 proved the turn loop works: prompt in, API call, tool dispatch, loop, response out. But the only tools are `get_current_time` and `calculate` — toys. The harness can't communicate with consumers, can't remember anything, can't read or write files, can't run commands.

Without real tools, the harness is a demo. With them, it's a runtime that can power waves.

The people who benefit are wave authors — they need an agent that can talk to users (`send_message`), persist knowledge across sessions (`memory_edit`), manage working state during a session (`context_*`), and interact with the filesystem (`read_file`, `write_file`, `shell`).

## Approach

Nine tools across three tiers. Event collection in the turn loop. JSONL output from `lf-agent`. Four commit slices.

### Tier 1: Boundary tools (C2)

`send_message` and `memory_edit` cross the harness→consumer boundary. They return simple confirmation to the model but emit `AgentEvent`s that the consumer (chat system, JSONL logger, test harness) acts on.

The turn loop collects events from `ToolResult::event` during dispatch. `TurnResult` gains `events: Vec<AgentEvent>`. This is the key change — tool execution becomes observable.

### Tier 2: Context tools (C3)

`context_read`, `context_write`, `context_delete`, `context_list`. An in-memory `HashMap<String, String>` with token counting per block. The agent's working scratchpad during a session.

Context tools need shared mutable state. The `Tool::call` signature is `&self`, so the `ContextStore` lives behind `Arc<Mutex<ContextStore>>` and is injected into each context tool at construction. The tools close over the shared store.

### Tier 3: File + shell tools (C4)

`read_file`, `write_file`, `shell`. Scoped to an ephemeral workspace (tempdir). Path traversal is rejected — all paths must resolve within the workspace root.

Shell runs commands via `std::process::Command` with a 30s timeout and working directory set to the workspace. Output is truncated to a token budget (roughly 8K tokens ≈ 32KB).

### JSONL output (C5)

`lf-agent` serializes each `AgentEvent` as a JSONL line to stdout. One event per line. stderr for diagnostics. Integration test verifies the full pipeline.

## Tool state injection

The central design question: how do tools that need shared state (context store, workspace path) get it?

**Decision: Constructor injection with `Arc<Mutex<T>>` for mutable state, `PathBuf` for immutable config.**

Each tool struct holds what it needs:

```rust
struct ContextRead {
    store: Arc<Mutex<ContextStore>>,
}

struct ReadFile {
    workspace: PathBuf,
}

struct Shell {
    workspace: PathBuf,
    timeout: Duration,
}
```

The `Tool::call(&self, input)` signature stays unchanged. No trait modification needed. Tools that need shared state hold an `Arc` to it. Tools that need config hold a copy.

This is the simplest approach that works. The alternative — adding a `&dyn ToolContext` parameter to `call()` — would touch every tool impl and the trait itself for a problem only 7 of 9 tools have. The other alternative — a single `ToolEnvironment` struct on the registry — couples tools that shouldn't know about each other.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| `call(&self, input, ctx: &dyn ToolContext)` | Clean DI, but forces every tool to accept context it ignores | Modifies the trait for a minority concern. GetCurrentTime doesn't need a workspace. |
| Single `ToolEnvironment` on registry | One place for all state | Couples unrelated tools. Context store and workspace path have nothing to do with each other. |
| Builder pattern on registry with typed state | Type-safe, no `Arc<Mutex>` | Over-engineered for 9 tools. Adds generic parameters to `ToolRegistry`. |
| Event bus / channel for event collection | Decoupled from tool results | Unnecessary indirection. Events already ride on `ToolResult::event`. |

## Key decisions

**Events ride on tool results.** The `ToolResult { output, event }` design from C1 pays off. The turn loop collects `event` from each tool call result during `make_tool_results`. No event bus, no channels, no separate collection mechanism. The existing plumbing just needs to stop discarding `event`.

**Context store behind `Arc<Mutex>`.** The turn loop is single-threaded today (async for API call, sync tool dispatch), so the mutex is uncontended. But `Arc<Mutex>` is the right abstraction because: (1) `Tool: Send + Sync` requires it, (2) it's trivially correct, (3) async tool dispatch in B3 won't require a redesign.

**Token counting is approximate.** `ContextStore` counts tokens per block using `tiktoken-rs` (already a dependency). This is an estimate — the model's actual tokenizer may differ slightly. Good enough for budget decisions. Not good enough for exact context window math (that's a B3 concern).

**Path traversal rejection is strict.** `read_file` and `write_file` canonicalize the resolved path and verify it starts with the workspace root. `..` that escapes the workspace returns an error, not a truncated path. Shell doesn't prevent filesystem access outside the workspace — it just sets the working directory. Real sandboxing is a B3 concern (git worktrees, seccomp, etc.).

**Shell output truncation is byte-based.** Truncate stdout+stderr to 32KB (roughly 8K tokens), append "[truncated]". The model sees enough output to act on. Token-precise truncation would require running tiktoken on potentially huge output — not worth it.

**`TurnResult` gains `events: Vec<AgentEvent>`.** This is a breaking change to the return type. Callers that destructure `TurnResult` will need to handle the new field. The lf-agent binary is the only caller today, so this is safe.

## Scope

In scope:
- 9 tools: `send_message`, `memory_edit`, `context_read`, `context_write`, `context_delete`, `context_list`, `read_file`, `write_file`, `shell`
- `ContextStore` (HashMap + token counting)
- Event collection in turn loop (`make_tool_results` returns events)
- `TurnResult.events: Vec<AgentEvent>`
- JSONL event output in `lf-agent`
- Ephemeral workspace via `tempdir()`
- Unit tests for each tool, integration test for the full pipeline

Out of scope:
- Model abstraction (extract when second provider arrives)
- Persistent workspace / git worktrees (B3)
- Context compaction / summarization (later — when sessions exceed context window)
- Streaming events during turn execution (later — JSONL after turn is fine for now)
- Chat system integration (A2/B3 — the chat system consumes events, doesn't produce them)
- Shell sandboxing beyond workspace working directory (B3)
- Async tool dispatch (B3 — sync is fine when shell timeout is 30s)

## Commit slices

### C2 — Boundary tools + event collection

**What:** `send_message` and `memory_edit` tools. Turn loop collects events.

**Changes:**

`agent/tools.rs` — Add `SendMessage` and `MemoryEdit` structs implementing `Tool`. `send_message` parses `SendMessageArgs` (already exists in `chat/contract.rs`) and returns `ToolResult { output: "message sent", event: Some(AgentEvent::Message { .. }) }`. `memory_edit` returns `ToolResult { output: "edit recorded", event: Some(AgentEvent::MemoryEdit { .. }) }`.

`agent/turn.rs` — `make_tool_results` returns `(Vec<ContentBlock>, Vec<AgentEvent>)` instead of `Vec<ContentBlock>`. The turn loop accumulates events across iterations. `TurnResult` gains `events: Vec<AgentEvent>`.

`agent/tools.rs` — `default_registry()` registers the boundary tools alongside the existing ones.

`bin/lf-agent.rs` — Print events count to stderr for now. Full JSONL in C5.

**Tests:** Boundary tools return correct output + event. Turn loop collects events from multi-tool responses. `default_registry` includes all 4 tools.

~250-350 LOC.

### C3 — Context tools

**What:** `ContextStore` and 4 context tools.

**Changes:**

`agent/context.rs` (new) — `ContextStore` wraps `HashMap<String, String>` with `read`, `write`, `delete`, `list` methods. `list` returns `Vec<(String, usize)>` (name, token count). Token counting via `tiktoken-rs`.

`agent/tools.rs` — Add `ContextRead`, `ContextWrite`, `ContextDelete`, `ContextList` structs. Each holds `Arc<Mutex<ContextStore>>`. Constructor: `fn new(store: Arc<Mutex<ContextStore>>) -> Self`.

`agent/tools.rs` — New factory `fn registry_with_context(store: Arc<Mutex<ContextStore>>) -> ToolRegistry` that registers all 6 tools (2 original + 2 boundary + 4 context). `default_registry` still exists for backward compat (no context tools).

`agent/mod.rs` — Export `context`.

**Tests:** ContextStore CRUD. Token counting smoke test. Context tools via registry dispatch. Shared state across tools (write then read through registry).

~200-300 LOC.

### C4 — File + shell tools

**What:** `read_file`, `write_file`, `shell` tools.

**Changes:**

`agent/tools.rs` — Add `ReadFile`, `WriteFile`, `Shell` structs. Each holds `workspace: PathBuf`. `Shell` also holds `timeout: Duration`.

Path validation: `fn validate_path(workspace: &Path, relative: &str) -> Result<PathBuf, String>` — joins, canonicalizes, checks prefix. Reused by `ReadFile` and `WriteFile`.

Shell: `std::process::Command::new("sh").arg("-c").arg(command).current_dir(&workspace)`. Capture stdout+stderr with timeout via `wait_with_output` + spawn/kill pattern. Truncate combined output to 32KB.

`agent/tools.rs` — New factory `fn full_registry(store: Arc<Mutex<ContextStore>>, workspace: PathBuf) -> ToolRegistry` that registers all 9 tools.

**Tests:** read_file round-trip. write_file creates parent dirs. Path traversal rejection. Shell runs, captures output. Shell timeout. Shell output truncation. Full registry has all 9 tools.

~200-300 LOC.

### C5 — JSONL output + integration

**What:** `lf-agent` emits JSONL. Integration test.

**Changes:**

`bin/lf-agent.rs` — After turn completes, serialize each event as JSONL to stdout. Response text goes to stderr (or is part of the `Message(final)` event — the response text is redundant when events are the primary output). Add `--workspace` flag (defaults to tempdir). Wire up `full_registry`.

Integration test (unit test with mock, not live API):

```rust
#[test]
fn turn_with_boundary_tools_collects_events() {
    // Build registry with send_message + memory_edit
    // Simulate a tool_use response from the "API"
    // Verify TurnResult.events contains the expected AgentEvent variants
    // Verify validate_turn_completion passes
}
```

This test doesn't call the real API. It tests the plumbing: registry → dispatch → event collection → validation.

**Tests:** JSONL serialization of events. Integration test with mock API response. Completion validation on collected events.

~150-250 LOC.

## Done when

```bash
cargo test -p loopflow agent
cargo test -p loopflow chat
cargo fmt --check
cargo clippy -- -D warnings
```

All pass. Plus:

```bash
cargo run --bin lf-agent -- "Tell me hello, then remember my name is Alice"
```

Produces JSONL on stdout with `send_message` and `memory_edit` events. stderr shows tool dispatch diagnostics.
