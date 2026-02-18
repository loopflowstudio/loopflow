# Design Review: Phase 01D Hardening + Chat System (A2/B2)

## What was implemented

Two major efforts converged on this branch:

1. **Chat turn system (A2/B2).** Waves now support interactive chat turns routed through lfd. `POST /waves/:id/chat` starts a harness agent turn in a background task; `GET /waves/:id/chat/events` streams `AgentEvent`s as SSE until completion. Messages persist to SQLite/Postgres. Memory blocks (named, ordered context) are managed via CRUD endpoints and serialized into the system prompt at turn start. Concerto's `ChatState` is now an event consumer/state machine that renders assistant text only from `send_message` events.

2. **Fork execution hardening (Phase 01D).** Fork branches now run through a proper lifecycle: plan → acquire scheduler slot → create worktree → launch agent → collect results → cleanup. Orphaned forks are recovered on startup. The Docker executor reattaches to running containers after daemon restart. Agent timeout is explicit operator config (`executor.agent_timeout`).

Supporting changes: fork planning extracted to a pure engine layer (`engine/fork.rs`), nightly Docker E2E CI workflow, wave/remote roadmap updated for post-01D learnings.

## Key choices

**`send_message` as sole UI output.** The harness produces user-visible text only through explicit `send_message` tool calls — not by streaming raw LLM tokens. This makes the output contract testable (exactly one `final` message per turn) and keeps the harness/chat boundary clean.

**SSE over WebSocket for chat events.** The wave detail already uses WebSocket for run events, but chat events use SSE (`text/event-stream`). SSE is simpler (unidirectional, auto-reconnect in browsers) and sufficient since the client only sends input via POST.

**Fork planning is pure.** `engine/fork.rs::plan_fork_execution` takes branch specs + directions and returns execution plans with no I/O. The executor layer consumes these plans. This makes fork selection logic testable without git repos or Docker.

**Scheduler slots are per-run, sessions are per-wave.** Slot acquisition (`acquire_guard`) gates concurrency. Session tracking (`has_active_session`) is a separate boolean presence check — scaffolding for interactive session management (not yet wired to callers).

**Docker executor rehydration.** On startup, each in-progress agent is checked against Docker: running containers are reattached, missing ones are marked failed. Orphaned containers (`io.loopflow.managed=true` not in active set) are removed. Per-repo image builds are serialized via `RepoMutationLocks`.

## How it fits together

```
Concerto ──POST /chat──▶ lfd ──spawn──▶ harness turn loop
         ◀──SSE events──                  │
                                          ├── tool dispatch (send_message, memory_edit, shell, file)
                                          └── AgentEvent stream → ChatTurnRegistry → SSE broadcast

Wave run ──executor──▶ flow steps ──fork──▶ plan_fork_execution (pure)
                                           ├── worktree per branch
                                           ├── scheduler slot per branch
                                           └── results via mpsc channel
```

The chat system and wave executor are independent consumers of the same store and scheduler. Chat turns are wave-scoped but don't participate in the wave run lifecycle — they're a parallel interaction mode.

## Risks and bottlenecks

- **In-memory turn streams.** `ChatTurnRegistry` tracks in-flight turns in a `HashMap` with broadcast channels. No TTL or eviction. Wave runs with many chat invocations will accumulate unbounded state. This is noted in wave/harness/README.md as the main operational risk for B3.

- **Fork + Docker unsupported.** `fork(select=all)` fails fast on the Docker executor (no worktree-based parallelism in containers). This is documented in docs/lfd.md but is a capability gap for remote users.

- **Session scaffolding unused.** `register_session`/`unregister_session` are `#[allow(dead_code)]` with no production callers. `has_active_session` always returns false. This is pre-wiring for interactive session management — safe but technically dead code.

## What's not included

- **Token streaming.** Chat turns emit event-granularity updates (progress → final), not token-level streaming. Acceptable for now; the event-driven approach shows activity.

- **Memory edit approval flow.** `memory_edit` events are auto-applied. Approval UX is deferred to B3.

- **Multi-provider model support.** Harness is hardcoded to Anthropic Messages API. Model trait extraction deferred until a second provider is needed.

- **Fork worktree parallelism in Docker.** Explicitly gated and documented as unsupported.

## Gate results

| Check | Result |
|-------|--------|
| `cargo fmt --check` | Pass |
| `cargo clippy -- -D warnings` | Pass |
| `cargo test --all` | Pass (368+ tests) |
| `uv run pytest python/tests/` | Pass (36 tests) |
| `tests/e2e/test_smoke.sh` | Pass |

### Fixes applied during gate

- **`atty` crate → `std::io::IsTerminal`.** `flow.rs` referenced unmaintained `atty` crate not in Cargo.toml. Replaced with stdlib `IsTerminal` (stable since Rust 1.70).
- **`serde_yaml` → `serde_yaml_ng` in test.** Config test used old crate name; fixed to match actual dependency.
- **`#[non_exhaustive]` on `ForkRunStatus`.** Public enum that will grow (Cancelled, TimedOut) — added per style guide.
- **Loop ticker scheduler error logging.** Silent `Err(_) => continue` replaced with `tracing::warn!` to match cron/watch pollers.
