# B2: Real Tools — Design Review

## What was implemented

Eleven tools across three tiers, making the agent harness a functional runtime:

- **Boundary tools** (`send_message`, `memory_edit`): Cross the harness-consumer boundary. Return confirmation to the model and emit `AgentEvent`s that consumers act on.
- **Context tools** (`context_read`, `context_write`, `context_delete`, `context_list`): In-memory `HashMap<String, String>` scratchpad with approximate token counting via tiktoken-rs.
- **File/shell tools** (`read_file`, `write_file`, `shell`): Scoped to an ephemeral workspace. Path traversal rejected. Shell has 30s timeout and 32KB output truncation.

Event collection in the turn loop: `make_tool_results` returns `(Vec<ContentBlock>, Vec<AgentEvent>)`. `TurnResult.events` accumulates events across all iterations.

`lf-agent` emits JSONL to stdout (one event per line), diagnostics to stderr.

## Key choices

**Constructor injection for tool state.** Context tools hold `Arc<Mutex<ContextStore>>`, file tools hold `PathBuf`. The `Tool::call(&self, input)` signature stays clean — no `ToolContext` parameter that most tools would ignore. Alternatives considered and rejected: `call(&self, input, ctx: &dyn ToolContext)`, single `ToolEnvironment`, builder pattern with generics.

**Events ride on `ToolResult`.** `ToolResult { output, event }` — boundary tools set `event`, internal tools return `None`. The turn loop collects from `event` during dispatch. No event bus, no channels.

**Three-level registry construction.** `default_registry()` (4 base tools) → `registry_with_context(store)` (8 tools) → `full_registry(store, workspace)` (11 tools). Consumers pick the level they need.

**Path validation with canonicalization.** `validate_path` handles both existing and not-yet-existing paths. For writes, it walks up to the nearest existing ancestor, canonicalizes, appends remaining components, then checks the prefix. Rejects both relative (`../`) and absolute (`/etc/passwd`) traversal.

**Completion contract from B1 carries forward.** `validate_turn_completion(&events)` works on the collected event stream. Exactly one `Message { phase: Final }` on success, zero on failure.

## How it fits together

```
lf-agent → TurnConfig + full_registry(store, workspace)
  → turn::run(prompt, config, registry)
    → loop: API call → make_tool_results → collect events → continue
  → TurnResult { response, events, ... }
  → JSONL to stdout
```

The registry is the tool surface. The turn loop is the execution engine. Events are the observation mechanism. Three concerns, cleanly separated.

## Risks and bottlenecks

**Shell is unsandboxed.** The working directory is set to the workspace, but `shell` can access anything on the filesystem. Real sandboxing (seccomp, git worktrees) is B3.

**Token counting is approximate.** `cl100k_base` may differ from the model's actual tokenizer. Good enough for budget visibility, not for precise context window management.

**No output truncation on `read_file`.** A model reading a multi-MB file gets the whole thing. Token-aware truncation is a future concern.

**Sync tool dispatch.** Shell commands block the turn loop thread for up to 30s. Async dispatch is B3.

## What's not included

- Model abstraction (extract `Model` trait when second provider arrives)
- Persistent workspaces / git worktrees (B3)
- Context compaction / summarization
- Streaming events during turn execution (JSONL after turn is sufficient for now)
- Chat system integration (A2/B3)
- Shell sandboxing beyond working directory
- Async tool dispatch

## Polish applied during gate

- Fixed `full_registry` doc comment (said "9 tools", actually 11)
- Cached tiktoken BPE instance in `context.rs` (was re-initializing on every `estimate_tokens` call; matches pattern in `engine/prompt.rs`)
- Changed silent error swallowing in `lf-agent.rs` JSONL output to `expect` (serialization of `AgentEvent` can't realistically fail)
- Added test for absolute path traversal rejection (existing test only covered relative `../` paths)
- Renamed `path_traversal_rejected` test to `path_traversal_relative_rejected` for clarity alongside new test
