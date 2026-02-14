# B2: Real Tools — Summary

Eleven tools across three tiers, making the agent harness a functional runtime.

## Architecture

```
lf-agent → TurnConfig + full_registry(store, workspace)
  → turn::run(prompt, config, registry)
    → loop: API call → make_tool_results → collect events → continue
  → TurnResult { response, events, ... }
  → JSONL to stdout
```

The registry is the tool surface. The turn loop is the execution engine. Events are the observation mechanism.

## Tools

- **Boundary** (`send_message`, `memory_edit`): Cross the harness-consumer boundary. Return confirmation to the model, emit `AgentEvent`s for consumers.
- **Context** (`context_read`, `context_write`, `context_delete`, `context_list`): In-memory `HashMap<String, String>` scratchpad. Approximate token counting via tiktoken-rs.
- **File/shell** (`read_file`, `write_file`, `shell`): Scoped to ephemeral workspace. Path traversal rejected. Shell has 30s timeout, 32KB output truncation.

Three-level registry: `default_registry()` (4 base) → `registry_with_context(store)` (8) → `full_registry(store, workspace)` (11).

## Key decisions

**Constructor injection for tool state.** Context tools hold `Arc<Mutex<ContextStore>>`, file tools hold `PathBuf`. The `Tool::call(&self, input)` signature stays clean. Alternatives rejected: `call(&self, input, ctx: &dyn ToolContext)` (modifies trait for minority concern), `ToolEnvironment` (couples unrelated tools), builder with generics (over-engineered).

**Events ride on `ToolResult`.** `ToolResult { output, event }` — boundary tools set `event`, internal tools return `None`. Turn loop collects during dispatch. No event bus, no channels.

**Path validation with canonicalization.** `validate_path` handles existing and not-yet-existing paths. Walks up to nearest existing ancestor, canonicalizes, checks prefix. Rejects both relative (`../`) and absolute (`/etc/passwd`) traversal.

**Completion contract from B1 carries forward.** `validate_turn_completion(&events)` works on the collected event stream.

## Known limitations (B3 scope)

- **Shell unsandboxed** — working dir is set, but shell can access anything. Real sandboxing (seccomp, git worktrees) is B3.
- **Token counting approximate** — `cl100k_base` may differ from model's tokenizer. Good enough for budget visibility.
- **No `read_file` truncation** — multi-MB files pass through whole. Token-aware truncation is future work.
- **Sync tool dispatch** — shell commands block the turn loop for up to 30s. Async dispatch is B3.
- Model abstraction (extract `Model` trait when second provider arrives)
- Persistent workspaces / git worktrees
- Context compaction / summarization
- Streaming events during turn execution
- Chat system integration (A2/B3)
