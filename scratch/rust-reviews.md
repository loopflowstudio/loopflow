# Rust Reviews

## Rust core engine (Stage 2)

### What was implemented
- Added core execution capabilities to `loopflow-engine`.
- Agent invocation (`agent.rs`) for Claude/Codex/Gemini with batch, streaming, and interactive modes.
- Config loading (`config.rs`) with `~/.lf/config.yaml` and `.lf/config.yaml` merging matching Python behavior.
- Context assembly (`prompt.rs`) for docs, diff, clipboard, and directions into `<lf:*>`-tagged prompt.
- Token counting via `tiktoken-rs` with byte/3 fallback.
- Deterministic choose execution (`runtime.rs`) for testability.
- LoopUntilEmpty execution (`runtime.rs`) with max-iterations guard.
- PyO3 bindings (`python.rs`) for `run_step`, `gather_context`, `launch_agent`.

### Key choices
- `tiktoken-rs` for accuracy; fallback is less precise but safe.
- Choose uses alphabetical selection to avoid LLM calls in tests.
- Wave detection uses worktree directory name, then branch, then "default".
- LaunchConfig controls interactive vs batch vs streaming.
- Trim priority: summaries → docs → diff → diff_files to match Python ordering.

### Fit
Python `lf` and `lfd` call into Rust engine via PyO3. The engine is stateless; run state stays in the daemon.

### Risks and bottlenecks
- Choose execution is stubbed and not production-ready.
- Context parity gaps: summaries, embedded LOOPFLOW.md, area parent docs, exclude patterns.
- `tiktoken-rs` vocab load failure falls back to rough byte heuristics.
- Fork failure can leave stale worktrees until autoprune.

### Not included
- gRPC engine contract (Stage 3).
- Postgres backend (Stage 5).
- Git workflow operations (Stage 4).
- Summary loading and LOOPFLOW.md embedding (TODOs in `prompt.rs`).

### Test coverage
30 tests across flow parsing, runtime execution, token counting, and module units. `cargo fmt` and `cargo clippy -- -D warnings` clean.

---

## Rust lf ops + engine bridge (Stage 4)

### What was implemented
- Added Rust `lf-engine` CLI and expanded `loopflow-engine` with prompt/runtime/agent/git APIs.
- Added PyO3 bindings to expose Rust APIs to Python.
- Routed `lf ops` git helpers through Rust when `internal.use_rust` is enabled, with Python fallbacks.
- Added `lf --version` path that defers to Rust when enabled and available.
- Updated docs to reference `lf ops` instead of the `lfops` binary.

### Key choices
- Shell to `lf-engine` for git/ops behavior via JSON bridge instead of reimplementing in Python.
- Rust token counting uses `tiktoken` with fallback; prompt assembly mirrors Python priority.
- Keep Python fallbacks when Rust is unavailable or disabled.
- Remove the `lfops` console script in favor of `lf ops`.

### Fit
Python CLI uses `lf-engine` for version reporting, and `lf ops` routes git operations to Rust via JSON subprocess calls when enabled. Rust engine remains the shared core for prompt/runtime/agent/git operations.

### Risks and bottlenecks
- `lf --version` depends on config parsing; invalid config can block Rust reporting (Python fallback remains).
- External references to `lfops` may still exist outside repo docs.
- Rust `lf-engine` JSON output must remain stable for Python parsing, including error paths.

### Not included
- Full deprecation cleanup of `lfops` references in other internal docs or Swift/Concerto integration.
- Rust `lfd` daemon or protocol changes (Stage 3+).
- Summary loading and LOOPFLOW.md embedding in prompt assembly remain TODOs in Rust.
