# Rust lfd: Session Connect + Fork Execution

## Problem

The Rust daemon skeleton is 80% complete, but two critical gaps block end-to-end flow execution:

1. **Interactive steps stall.** When `tick_flow` returns `WaitingInteractive`, the wave enters `WaveWaiting` and never resumes. No session connect mechanism exists to attach a terminal and advance the step.

2. **Fork/choose/loop fail.** The `tick_flow` function only handles `FlowItem::Step`. Any flow using fork, choose, or loop_until_empty immediately fails with "non-step flow item not supported in tick".

These gaps mean:
- Flows with `design` (interactive) steps cannot complete.
- Parallel execution patterns (fork→synthesize) do not work.
- The roadmap's `ship-roadmap` and `roadmap-reduce` flows are blocked.

## Approach

Add session connect first. It unblocks the most common interactive pattern (design→implement) and establishes the control/execution boundary the daemon needs. Then extend `tick_flow` to handle fork items, which enables the parallel agent workflows that distinguish loopflow from simple scripts.

### Session connect

A new RPC `ConnectWave(wave_id) → stream Output` attaches a terminal to a waiting interactive step. The connection:

1. Spawns the step as a subprocess with PTY
2. Streams stdout/stderr back through the gRPC response stream
3. Accepts stdin through a bidirectional stream (or follow-up `SendInput` calls)
4. On step exit, calls `EndStepRun` and resumes flow execution

**Key invariant:** The daemon remains the source of truth. The connected terminal is a view, not the controller. If the connection drops, the step continues running; reconnection reattaches to the same PTY.

Implementation path:
- Add `sessions.rs` with PTY management (using `portable-pty` crate)
- Track active sessions in `Scheduler` (wave_id → session handle)
- Implement `ConnectWave` in `server.rs` to return `tonic::Streaming<Output>`
- Add `DisconnectWave` to clean up abandoned sessions
- Modify `loop_ticker` to skip waves in `WaveWaiting` with active sessions

### Fork execution

Extend `tick_flow` to handle `FlowItem::Fork`:

1. When encountering a fork, spawn each branch as a separate worktree
2. Track branch states in a new `ForkRun` table (parent_wave_id, branch_index, status)
3. Each branch ticks independently through its sub-items
4. When all branches complete, run the synthesize step (if present)
5. Advance parent wave past the fork item

This matches the Python daemon's fork behavior:
- Branches share the parent wave's direction
- Each branch gets its own worktree
- Synthesize step receives all branch outputs via context

**Choose and loop_until_empty:** Deferred. Choose requires UI integration; loop_until_empty requires predicate evaluation. Fork alone unblocks the roadmap flows.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Shell to Python `lf` for session connect | Simpler, reuses existing code | Adds latency, breaks control/execution isolation, PTY management crosses process boundaries |
| Implement full flow runtime in Rust | Complete parity, no Python dependency | Too large; fork alone is weeks of work. Incremental is better. |
| Skip session connect, require auto-only flows | Simpler daemon | Blocks interactive patterns like `design`. Loopflow's value is the mix of auto and interactive. |
| Use threads instead of async for PTY | Simpler PTY handling | Inconsistent with Tokio-based architecture; harder to integrate with existing async loops |

## Key decisions

**Protocol first** (from wave principles): Session connect is defined in `control.proto` before implementation. `ConnectWave` returns `stream Output`; no REST bypass.

**Control/execution isolation** (from wave principles): The daemon owns session lifecycle. The PTY subprocess is the execution plane. Crashes in the step process don't destabilize the daemon.

**Worktree per fork branch:** Each branch gets `<parent-worktree>-fork-<index>`. Worktrees are created before branch execution and removed after synthesize. This matches Python behavior and preserves git isolation.

**SQLite schema extension:** Add `fork_runs` table for branch tracking. This is additive; existing waves table unchanged.

**Retry semantics:** Fork branches inherit the wave's `consecutive_failures` counter. If any branch fails 3 times, the entire fork fails and wave enters `WaveError`. This keeps the retry model simple.

## Scope

In scope:
- Session connect via PTY (`portable-pty`)
- `ConnectWave`, `DisconnectWave` RPCs
- Fork execution in `tick_flow`
- Synthesize step after fork completion
- `ForkRun` schema and store operations
- Basic tests for session lifecycle and fork advancement

Out of scope:
- Choose and loop_until_empty (deferred to future work)
- Web-based session UI (Concerto handles this separately)
- Multi-model racing (different problem)
- Branch-level retry limits (use wave-level for now)

## Done when

```bash
# Interactive step completes via session connect
cargo run --bin lfd &
lfd create interactive-test --repo . --stimulus manual
lfd run interactive-test
lfd connect interactive-test  # attaches terminal, step runs
# Step completes, wave advances

# Fork flow executes parallel branches
lfd create fork-test --repo . --flow roadmap-reduce --stimulus manual
lfd run fork-test
# Three branches spawn, run in parallel
# Synthesize step runs after all complete
lfd status fork-test  # shows WaveIdle after completion

# Tests pass
cargo test --package lfd
```

## Open questions

Appended to `scratch/questions.md`:

- **PTY crate choice:** `portable-pty` vs `pty-process` vs raw `nix::pty`. Need to evaluate cross-platform support (macOS primary, Linux for containers).
- **Session timeout:** Should we kill long-idle interactive sessions? If so, what timeout? 4 hours like stuck runs?
- **Fork branch parallelism:** All branches at once, or honor slot limits per branch? Current assumption: each branch acquires a slot.
