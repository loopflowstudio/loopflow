# Design Review: Rust Core Engine (Stage 2)

## What was implemented

Added core execution capabilities to `loopflow-engine`:

- **Agent invocation** (`agent.rs`) — Spawns Claude, Codex, or Gemini CLI with assembled prompts. Supports batch, streaming, and interactive modes.
- **Config loading** (`config.rs`) — Reads `~/.lf/config.yaml` and `.lf/config.yaml` with merging semantics matching Python implementation.
- **Context assembly** (`prompt.rs`) — Gathers docs, diff, clipboard, and directions. Formats into structured prompt with `<lf:*>` tags.
- **Token counting** — tiktoken-rs integration with cl100k_base encoding. Falls back to bytes/3 if loading fails.
- **Choose execution** (`runtime.rs`) — Selects first option alphabetically (deterministic for testing).
- **LoopUntilEmpty execution** (`runtime.rs`) — Iterates until `roadmap/<wave>/` is empty, with max_iterations guard.
- **PyO3 bindings** (`python.rs`) — Exposes `run_step`, `gather_context`, `launch_agent` to Python.

## Key choices

1. **tiktoken-rs for token counting.** The `tiktoken-rs` crate wraps OpenAI's tokenizer. It's accurate for Claude (close enough for context budgeting). Alternative: byte-based heuristic would be faster but less accurate for trimming decisions.

2. **Deterministic choose selection.** Current implementation picks first option alphabetically. Production would invoke an LLM to evaluate the prompt. This decision was intentional to enable testing without mocking LLM calls.

3. **Wave detection by directory name.** LoopUntilEmpty infers wave from worktree directory name (e.g., `loopflow.rust.wave-name`), then falls back to current branch, then "default". Matches existing daemon behavior.

4. **LaunchConfig controls output mode.** Three modes: interactive (inherits stdio), batch (captures all), streaming (line-by-line). Caller decides based on use case.

5. **Context priority for trimming.** Drop summaries first, then docs, then diff, then diff_files. Matches Python priority order documented in design doc.

## How it fits together

```
lf (Python CLI)
    │
    ├── via PyO3 ───────────┐
    │                       │
    │                       ▼
lfd (Python daemon)    loopflow-engine (Rust)
    │                       │
    └── via library ────────┤
                            │
                            ├── config.rs (load ~/.lf/, .lf/ config)
                            ├── prompt.rs (gather context, format prompt)
                            ├── agent.rs (spawn claude/codex/gemini)
                            └── runtime.rs (tick_flow with fork/choose/loop)
```

The engine is stateless. Run state (SQLite) is managed by lfd. Python `lf` imports the engine via PyO3 bindings for direct invocation without subprocess overhead.

## Risks and bottlenecks

- **Choose prompt evaluation is stubbed.** The current alphabetical selection is not production-ready. Real choose execution would need to invoke an LLM and parse structured output.

- **Context parity gaps remain.** Summaries are not loaded. Loopflow doc is not embedded. Area parent docs and exclude patterns are not implemented. These are documented in `scratch/questions.md`.

- **tiktoken-rs loading.** The tokenizer loads vocabulary on first use. If this fails (e.g., missing data files), fallback to bytes/3 is less accurate.

- **Worktree cleanup on fork failure.** If a fork branch fails mid-execution, worktrees may persist. The daemon's autoprune handles cleanup, but stale worktrees are annoying.

## What's not included

- **gRPC engine contract** — Deferred to Stage 3 when daemon moves to Rust.
- **Postgres backend** — Deferred to Stage 5.
- **Git workflow operations** — Deferred to Stage 4 (lf ops refactor).
- **Summary loading** — TODOs remain in `prompt.rs:203`.
- **Bundled LOOPFLOW.md** — TODO remains in `prompt.rs:192`.

## Test coverage

30 tests across unit and integration:
- Flow parsing: 2 tests (flow_tests.rs)
- Runtime execution: 5 tests (runtime_tests.rs) covering auto, interactive, fork, choose, loop
- Token counting: 1 test (token_tests.rs)
- Unit tests: 22 in module tests (agent, config, prompt, git)

All tests pass. `cargo clippy -- -D warnings` clean. `cargo fmt` clean.
