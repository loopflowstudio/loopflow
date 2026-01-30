# Research: Loopflow Rust Migration

## System understanding

Loopflow is a multi-language system for orchestrating AI coding agents. The codebase spans three languages—Python (CLI and daemon), Rust (core engine and daemon), and Swift (Concerto macOS app)—with a staged migration from Python to Rust underway.

### Architecture

**Three-layer stack:**

1. **lf-core** (Rust library, `rust/lf-core/`) — Domain model and execution engine
   - Flow parsing and validation (`flow.rs`)
   - Runtime state machine with tick-based execution (`runtime.rs`)
   - Git operations: rebase, push, branch, land (`git.rs`)
   - Worktree management (`worktree.rs`)
   - Context assembly and token counting (`prompt.rs`)

2. **lfd** (Rust daemon, `rust/lfd/`) — Control plane and scheduling
   - gRPC server implementing ControlService (`server.rs`, ~800 lines)
   - HTTP observability endpoints (`http.rs`)
   - SQLite persistence (`store/sqlite.rs`, ~1000 lines)
   - Scheduler for concurrency control (`scheduler.rs`)
   - Background loops for cron, watch, recovery (`loops/`)

3. **Python CLI** (`src/loopflow/`) — User interface and legacy daemon
   - `lf` CLI for step/flow execution
   - `lfd` Python daemon (being migrated)
   - `lfops` git workflow commands (transitioning to `lf ops`)

**Protocol boundary:**

The gRPC schema (`proto/loopflow/control/v1/control.proto`) defines the contract between clients (lf CLI, Concerto) and the daemon. Key entities:
- **Wave**: Named work unit with area, direction, flow, stimulus
- **Stimulus**: Trigger configuration (once/loop/watch/cron)
- **StepRun**: Individual step execution record
- **FlowRun**: Flow execution state (step_index, status)

### Data flow

**Step execution path:**

```
User: lf <step>
  → Python CLI assembles context (gather_prompt_components)
  → Launches agent (claude/codex/gemini) with prompt
  → Agent runs in terminal

Daemon mode: lfd loop <wave>
  → Creates Wave in SQLite
  → Scheduler acquires slot
  → tick_flow() in lf-core advances FlowRun
  → Shells to `lf --step <step> --worktree <path> --auto`
  → On interactive step: pauses, emits wave.waiting event
  → ConnectWave RPC resumes via PTY session
```

**Protocol flow:**

```
Concerto/lf CLI
    ↓ gRPC (port 50051)
lfd server.rs → ControlService
    ↓ spawn_blocking
SqliteStore → ~/.lf/lfd.db

Events:
lfd → Subscribe RPC → streaming events to Concerto
Concerto also uses Unix socket for real-time output
```

### Key abstractions

**RunStore trait** (`lf-core/src/store.rs`): Persistence interface for flow/step runs. Implemented by `SqliteStore` in lfd. Enables testing with `MemoryStore`.

**StepRunner trait** (`lf-core/src/runtime.rs`): Abstracts step execution. `CommandStepRunner` shells to `lf` CLI. Enables testing with `FakeRunner`.

**FlowItem enum**: Represents flow structure—Step, Fork, Choose, LoopUntilEmpty. Fork executes branches in parallel with worktree isolation.

**Stimulus**: Decoupled from Wave in recent refactor. Multiple stimuli can trigger one wave. Supports coalescing via `PendingActivation`.

## Tensions

**Python ↔ Rust boundary**: The Rust daemon shells to Python `lf` CLI for step execution. This creates process overhead and complicates error handling. Stage 4 roadmap proposes moving git operations to lf-core and having Python call Rust.

**Two daemons coexist**: Python `lfd` and Rust `lfd` both exist. The Python daemon is the default. Rust daemon has feature parity gaps—Subscribe RPC returns empty stream, ListFlows/ListWorktrees unimplemented.

**Store trait vs proto types**: lf-core's `RunStore` trait uses domain types (`FlowRun`, `StepRun`), while lfd's `SqliteStore` uses proto-generated types (`Wave`, `Stimulus`). The mapping happens at runtime:runtime.rs:161-166 where `store.get_run()` returns a `FlowRun` but lfd stores `Wave`.

**Fork worktree cleanup**: Fork branches create temporary worktrees (`{base}-fork-{n}`). Cleanup happens after fork completion, but failure mid-fork can orphan worktrees.

## Observations

### Complexity

**server.rs:458-592 (ConnectWave RPC)**: 130 lines handling interactive step connection. Spawns PTY session, updates step run status, manages wave state transitions, handles cleanup on error. Multiple error paths with manual resource cleanup.

**store/sqlite.rs**: Schema management embeds SQL as string literals (~130 lines of CREATE TABLE). Migration from waves.stimulus_* columns to separate stimuli table is in-place with backwards compat columns.

**runtime.rs:255-416 (run_fork_item)**: Fork execution is ~160 lines handling worktree creation, branch iteration, synthesize step, cleanup. Lots of early returns and state management.

### Quality

**Test coverage varies:**
- lf-core has focused unit tests: `flow_tests.rs`, `runtime_tests.rs`, `token_tests.rs`
- lfd has no dedicated test directory
- Python tests are comprehensive: 35 test files covering CLI, daemon, ops

**Error handling:**
- lf-core uses `thiserror` with structured error types (`CoreError`, `GitError`, `LoadError`, `StoreError`)
- lfd maps all store errors to gRPC Status codes at server.rs:93-100
- Some gRPC RPCs return stub responses (ListFlows returns empty, Subscribe returns empty stream)

**Documentation:**
- Proto file has excellent comments explaining each RPC and type
- Rust code has minimal doc comments
- CLAUDE.md/AGENTS.md provide clear style guidance

### Potential

**Stimulus coalescing**: `PendingActivation` infrastructure exists but isn't fully wired. Could enable smart batching of watch triggers.

**RunStore abstraction**: Clean trait enables alternative backends. Stage 5 roadmap plans Postgres for managed clusters.

**Event streaming**: Subscribe RPC exists but returns empty. Infrastructure is ready for real-time event push to clients.

**StepRunner injection**: Trait-based runner enables testing without subprocess. Could enable in-process step execution.

## Open questions

- Should lf-core's `RunStore` trait align with proto types directly, or maintain separate domain types?
- How should the Subscribe RPC be implemented—broadcast channel, tokio watch, or something else?
- What's the migration path for existing Python lfd users when Rust lfd becomes default?
- Should worktree cleanup failures block fork completion or be logged and continued?

## Recommendations

### Implement Subscribe RPC event streaming

**Observation**: Subscribe RPC exists but returns empty stream (server.rs:769-775). Concerto falls back to Unix socket polling.

**Cost**: Medium—requires adding broadcast channel to ControlServer, wiring event emission points.

**Benefit**: Unifies event delivery, enables remote clients without Unix socket access.

**Verdict**: Worth doing—critical for remote client support (Stage 4 goal).

### Add lfd integration tests

**Observation**: No test coverage for gRPC service layer or SQLite store interactions.

**Cost**: Medium—need test fixtures, temp DB setup.

**Benefit**: Catches regressions in RPC handling, store queries, error mapping.

**Verdict**: Worth doing—current coverage is smoke tests via manual testing only.

### Document Rust ↔ Python interaction contract

**Observation**: `CommandStepRunner` assumes `lf --step <name> --worktree <path> --auto --direction <d>` CLI interface. This contract is implicit.

**Cost**: Low—add doc comments and/or a test that verifies CLI flags.

**Benefit**: Prevents silent breakage when either side changes.

**Verdict**: Worth doing—low effort, high payoff for maintainability.

### Consolidate error types

**Observation**: lf-core has `StoreError` with `RunNotFound(String)`. lfd has its own `StoreError` with different variants. Conversion happens implicitly.

**Cost**: Low—align error variants or add explicit conversion.

**Benefit**: Clearer error handling, easier debugging.

**Verdict**: Worth doing—reduces confusion about which error type is in scope.
