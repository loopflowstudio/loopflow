# Harness: lf-agent runtime + roadmap revision

## What was implemented

Two things in one branch:

1. **`lf-agent` binary and agent runtime** (`rust/loopflow/src/agent/`). A working Anthropic Messages API client with tool dispatch and a turn loop. Three modules:
   - `anthropic.rs` — Request/response types and raw HTTP client (reqwest + serde, no SDK)
   - `tools.rs` — Tool definitions and dispatch. Two demo tools: `get_current_time` and `calculate` (arithmetic evaluator with operator precedence)
   - `turn.rs` — Turn loop that sends prompts, dispatches tool calls, feeds results back, loops until `stop_reason != "tool_use"`. Guardrails: max iterations (20) and timeout (300s)

2. **Harness roadmap rewrite** (`roadmap/harness/README.md`). Collapsed 10 separate phase files into a single two-track roadmap (Track A: chat system, Track B: agent harness) with iterative checkpoints.

Also added:
- `update-roadmap` step and `ship-roadmap` flow for post-ship roadmap revision
- Revised `roadmap` step with vision-first structure

## Key choices

**Raw HTTP, no SDK.** The Anthropic adapter is ~130 lines of hand-rolled types + reqwest. No `anthropic-rs` dependency. This keeps the adapter thin and makes the data model explicit.

**Concrete tools, not a framework.** The tool system is a simple `dispatch` function with a match arm per tool. No trait-based registry, no dynamic loading. Adding a tool means adding a function and a match arm.

**Two-track roadmap.** The old roadmap was 11 sequential phases in separate files with a dependency graph. The new one has two parallel tracks (chat system + agent harness) that converge at B3. Phases have learning checkpoints and "Try it" sections for feel-testing.

**Deleted 10 roadmap files.** The separate phase files were premature detail for work that hadn't started. The README captures everything needed at this stage.

## How it fits together

```
lf-agent (binary)
  └── turn::run(prompt, config)
        ├── anthropic::call(request)    ← HTTP to Anthropic API
        └── tools::make_tool_results()  ← dispatch tool calls
              └── tools::dispatch()     ← match on tool name
```

The agent module is self-contained under `src/agent/`. It doesn't depend on any other loopflow modules. The binary is a thin CLI wrapper around `turn::run`.

## Risks and bottlenecks

- **New `reqwest::Client` per API call** — prevents HTTP connection reuse. Fine for B1 (single turns), will matter for B2 (multi-turn sessions). Easy fix: pass client through or use `once_cell`.
- **No streaming** — full request/response only. The roadmap explicitly calls out that `send_message` as a tool call might not survive UX testing in A2.
- **Token counting is not done** — the turn loop sends the full message history every iteration. No budget management yet. This is intentional (deferred to B2).
- **Demo tools only** — `calculate` and `get_current_time` exist to prove the tool dispatch works. Real tools (`send_message`, file ops, shell) are B2.

## What's not included

- Model abstraction (no `Model` trait — extract when adding a second provider)
- Streaming (not needed for tool-call-only output model)
- Token budgeting / compaction
- Persistence (context is ephemeral by design)
- Chat system (Track A — not started)
- `send_message` / `memory_edit` tools (B2)
- Event emission (B2)
