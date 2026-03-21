---
linear_id: b854c1c9-b49f-47f6-be6f-381f7c7cb1b0
---
# Real CLI Executor

## Problem

The daemon reimplements flow execution semantics — step sequencing, xor routing, loop bodies, fork/and coordination, fast-path, pre/post sync, prompt assembly — in ~2,600 lines across `WaveExecutor` and its helpers that parallel the CLI's ~700-line `lf/commands/flow.rs`. Every new flow construct needs two implementations. Every behavior difference between "ran manually" and "ran by daemon" is a latent bug.

The journal contract is shipped and bidirectional. The daemon already reads journal events from any `lf` process. It's time to stop reimplementing `lf` inside `lfd`.

## Approach

Extract flow iteration into a shared `FlowEngine`. Both the CLI and daemon use the same engine for flow expansion, sequencing, xor routing, loops, and fork/and. They differ only in how they execute individual steps.

### FlowEngine (shared)

A pure flow iterator. Given a flow definition, it expands it and drives step-by-step dispatch through a trait:

```rust
trait StepExecutor {
    async fn run_step(&self, step: &str, ctx: &StepContext) -> Result<()>;
    async fn read_verdict(&self, path: &Path) -> Result<String>;
}

struct FlowEngine<E: StepExecutor> {
    executor: E,
}

impl<E: StepExecutor> FlowEngine<E> {
    async fn run(&self, flow: &Flow) -> Result<()>;
    // Handles: sequencing, xor routing, loops, fork/and
    // Does NOT handle: how steps execute, how output is captured,
    //                   how processes are supervised
}
```

The engine owns: flow expansion, step ordering, xor verdict reading and path selection, loop iteration and exit conditions, fork/and branch enumeration and synthesis.

The engine does NOT own: process spawning, output streaming, commit/push/rebase, agent configuration, cancellation, timeouts.

### CLI executor

The CLI provides an in-process `StepExecutor`:

```rust
struct CliExecutor { /* current run context */ }

impl StepExecutor for CliExecutor {
    async fn run_step(&self, step: &str, ctx: &StepContext) -> Result<()> {
        // Same as today's flow.rs run_step — runs the agent in-process
    }
}
```

This replaces the inline flow logic in `flow.rs` with `FlowEngine<CliExecutor>`. Behavior is unchanged — it's a refactor.

### Daemon executor

The daemon provides a process-supervision `StepExecutor`:

```rust
struct DaemonExecutor { /* store, output_hub, wave context */ }

impl StepExecutor for DaemonExecutor {
    async fn run_step(&self, step: &str, ctx: &StepContext) -> Result<()> {
        if ctx.interactive {
            // Launch into hosted tmux session
            self.run_interactive(step, ctx).await
        } else {
            // Spawn `lf <step> -b` as child process
            self.run_headless(step, ctx).await
        }
    }
}
```

This replaces `WaveExecutor::execute()`. The daemon still owns:
- Scheduling & queueing
- Worktree lifecycle
- Process supervision (spawn, capture output, track PID, enforce timeout, reap)
- Exit status reconciliation
- Pre/post run ops (git polling, auto-PR, branch advancement, triggers)
- Provider auth injection

The daemon stops reimplementing: flow expansion, step iteration, xor routing, loop bodies, fork dispatch, prompt assembly, summary refresh.

### Interactive steps

Interactive steps launch into hosted tmux sessions that clients (Concerto) can attach to. The daemon determines interactivity from the step definition, not from runtime signals.

For headless steps: `lf <step> -b` as a child process.
For interactive steps: `lf <step>` inside a tmux session.

Both cases: the daemon waits for the step to complete (process exit for headless, session completion for interactive), then advances to the next step via the shared FlowEngine.

No `--start-index`. No pause/resume protocol. No escalation signals. The daemon drives the flow step-by-step and launches each step appropriately.

This works for both local and remote Concerto — tmux sessions are attachable regardless of where the client runs.

### Per-step sync

Per-step sync (commit, push, rebase between steps) stays in the daemon. The daemon wraps each step execution:

```
pre_step_sync()  →  spawn lf <step>  →  post_step_sync()
```

This is a supervision concern — the daemon manages the worktree's git state around step execution. The CLI doesn't need to know about it.

### Environment injection

Before spawning `lf`, set:

| Var | Value | Purpose |
|-----|-------|---------|
| `LFD_RUN_ID` | run ID | Correlate journal events with daemon run records |
| `LFD_WAVE_ID` | wave ID | Attribute run to wave |
| `LFD_SESSION_ID` | session ID | Group runs within a session |
| `LF_RUN_ID` | run ID | CLI uses this as its journal run directory name |

### Journal-based progress

The spawned `lf` process writes journal events to `.lf/journal/runs/<run_id>/events.jsonl`. The existing `LfObserver` picks these up and fans them through the EventHub. Clients get step-level progress without new plumbing.

Step-index tracking: the daemon updates `run.step_index` directly after each step completes (it's driving the flow), rather than deriving it from journal events.

### Run-scoped overrides

| Snapshot field | CLI arg |
|----------------|---------|
| `flow` | positional arg to `lf` |
| `direction` | `-d <dir>` (repeatable) |
| `area` | `-a <area>` (repeatable) |

## Key decisions

**Shared FlowEngine, not opaque process.** The daemon and CLI share flow iteration logic. They differ in step execution — in-process vs process supervision. This eliminates the duplication without making the daemon blind to flow structure.

**Step-by-step dispatch, not whole-flow process.** The daemon spawns one `lf <step>` per step, not one `lf <flow>`. This makes interactive steps natural — just launch into a tmux session instead of headless. No pause/resume protocol needed.

**Clean cut, no dual path.** The legacy executor is deleted. No feature flag, no side-by-side comparison. The shared FlowEngine is tested directly.

**Interactive steps via tmux sessions.** The daemon hosts tmux sessions for interactive steps. Concerto attaches to them. Works local and remote.

**Per-step sync stays in daemon.** Commit/push/rebase between steps is a supervision concern, not a flow concern. The daemon wraps step execution with git ops.

**Journal for observability, not control flow.** The daemon doesn't need journal events to drive flow iteration — it's doing that itself via FlowEngine. Journal events provide observability for clients (Concerto, lfq).

## Alternatives considered

| Approach | Why not |
|----------|---------|
| Whole-flow process (`lf <flow> -b`) | Makes interactive steps hard — need pause/resume protocol, `--start-index`, escalation signals |
| Dual-path transition with feature flag | The legacy executor has no external consumers. Clean cut is simpler. |
| Keep WaveExecutor, extract shared lib | Was the original "no" — but sharing the flow *engine* while varying step *execution* is the right split |
| In-process library call | Process boundary gives clean cancellation, crash isolation, timeout enforcement |

## Scope

**In scope:**
- Shared `FlowEngine` with `StepExecutor` trait
- CLI refactor to use `FlowEngine<CliExecutor>`
- Daemon refactor to use `FlowEngine<DaemonExecutor>`
- Interactive steps launched into hosted tmux sessions (demo target)
- Environment variable injection
- Deletion of legacy `WaveExecutor::execute()` flow logic
- Journal observer updates to track step progress from events

**Out of scope:**
- Full daemon-hosted shell infrastructure (item 02 — this item demos the minimal version for interactive steps)
- Push-based journal ingestion (1s poll is fine)
- Changes to scheduling, queueing, or trigger logic
- Docker executor changes

## Done when

- `FlowEngine` is shared between CLI and daemon, tested independently
- CLI uses `FlowEngine<CliExecutor>` — behavior unchanged from today
- Daemon uses `FlowEngine<DaemonExecutor>` — spawns `lf <step> -b` for headless, tmux session for interactive
- Interactive step demo: daemon launches an interactive step into a tmux session, Concerto can attach
- `WaveExecutor::execute()` is ~100 lines of FlowEngine setup + supervision, not ~2,600 lines of flow interpretation
- Legacy flow-interleaved-with-execution code is deleted
- Integration tests cover: serialized waves, parallel waves, xor routing, loop flows, fork/and, cancellation, failure propagation
