# Design Review: Session API — Claude Harness + Concerto Migration

Branch: `jack-heart.agentapi.20260223_1429`

## What was implemented

**Rust (lfd):** Claude provider harness for the session API, with full NDJSON stream parsing and event mapping. The old adapter/factory pattern was replaced with a simpler enum + function pointer dispatch. Legacy wave chat HTTP route removed.

**Swift (Concerto):** Interactive UI migrated from legacy chat/memory endpoints to the session API. `ChatState` now drives turns through `POST /sessions/{id}/input` and consumes `SessionEvent` SSE streams. Old `AgentSession` plumbing removed.

**Docs/Wave:** Wave README and phase docs updated to reflect shipped state. Terminology normalized from "adapter" to "harness" throughout. Phase 04 (hardening) updated with Claude-specific gaps discovered during implementation.

## Key choices

**Harness pattern over adapter trait.** The old `adapter/mod.rs` used a factory-style `CreateAdapterFn` function pointer. This branch replaces it with a `HarnessProvider` enum that dispatches directly to `CodexHarness::new()` or `ClaudeHarness::new()`. The enum is simpler, type-safe, and doesn't require trait object indirection for provider selection. The `SessionHarness` trait remains for the actual harness behavior (start/send_input/stop).

**Process-per-turn for Claude.** Each `send_input()` spawns a new `claude -p ... --output-format stream-json` process. Continuation uses `--resume <provider_session_id>`. This avoids persistent subprocess management and maps naturally to Claude's CLI model. The `TurnInProgressGuard` (common.rs) prevents concurrent spawns.

**Separate mapping modules.** `claude_mapping.rs` (290 lines) and `codex_mapping.rs` (232 lines) keep provider-specific JSON parsing isolated from harness lifecycle logic. Tool name inference (`Bash` → `Command`, `Edit`/`Write`/`NotebookEdit` → `File`, others → `Tool`) lives entirely in the mapping layer.

**Legacy route removal.** `routes/chat.rs` and `http/state.rs` (673 lines total) deleted. The wave chat surface was superseded by the session API. No backwards-compatibility shim — Concerto was the only consumer and it migrates in this same branch.

## How it fits together

```
SessionManager (mod.rs, 461 lines)
  ├── creates SessionRuntime per session
  │     ├── Mutex<Box<dyn SessionHarness>>
  │     ├── broadcast::Sender<PersistedSessionEvent>  (SSE fan-out)
  │     └── AtomicI64 next_seq
  ├── spawns event bridge task (harness events → store + broadcast)
  └── delegates to SharedStore for persistence

HarnessProvider enum (harness/mod.rs)
  ├── Codex → CodexHarness (JSON-RPC stdio, persistent subprocess)
  └── Claude → ClaudeHarness (NDJSON stdio, process-per-turn)

HTTP routes (routes/sessions.rs)
  └── create / get / input / events (SSE) / delete
```

Concerto's `ChatState` creates sessions via `POST /sessions`, sends input, and consumes `ChatTurnEvent` from the SSE stream. The `LocalWaveService` bridges between the wave-level chat abstraction and the session API endpoints.

## Risks and bottlenecks

- **Process-per-turn startup latency.** Each Claude turn spawns a new process. Unmeasured in interactive use — could be noticeable on slow machines. Mitigation: the simplicity benefit is worth it until profiling shows otherwise.
- **No restart rehydration.** `SessionRuntime` is in-memory. Daemon restart orphans active sessions. Events survive in the store but the harness process is gone. Tracked in Phase 04.
- **Reader-task-stop race (Claude).** Between `stop()` killing the child and aborting the reader, a normal-exit `TurnCompleted(Completed)` could leak. `AtomicBool` prevents state corruption but event ordering may surprise clients. Tracked in Phase 04.
- **Tool name mapping is manually maintained.** New Claude tools require explicit additions to `claude_mapping.rs`. The generic `Tool` fallback prevents breakage but reduces UI fidelity for unmapped tools.

## What's not included

- **Concerto UI rendering of typed items.** The UI receives typed events but renders them as plain text for now. Rich item chrome (command output, file diffs) is Phase 03 follow-up work.
- **`DiffUpdated` for Claude.** Claude doesn't emit turn-level diffs. The event type exists but is absent from Claude sessions. UI must not depend on it.
- **OpenCode harness.** Third provider validates the abstraction. Planned for Phase 06.
- **Provider layer unification.** Making the harness layer shared between `lf` CLI and `lfd` session HTTP is Phase 07.

## Test results

| Suite | Result |
|-------|--------|
| `cargo fmt --check` | pass |
| `cargo clippy -- -D warnings` | pass |
| `cargo test --all` | 431 tests, 0 failures |
| `uv run pytest python/tests/` | pass |
| `swift test --package-path swift` | 118 tests, 0 failures |
