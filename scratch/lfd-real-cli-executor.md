---
linear_id: b854c1c9-b49f-47f6-be6f-381f7c7cb1b0
---
# Real CLI Executor

## Problem

The daemon reimplements flow execution semantics — step sequencing, xor routing, loop bodies, fork/and coordination, fast-path, pre/post sync, prompt assembly — in ~1200 lines of `WaveExecutor::execute()` that parallel the CLI's `lf/commands/flow.rs`. Every new flow construct (or, nested loops) needs two implementations. Every behavior difference between "ran manually" and "ran by daemon" is a latent bug. The journal contract is shipped and bidirectional. The daemon already reads journal events from any `lf` process. It's time to stop reimplementing `lf` inside `lfd`.

Who benefits: anyone running automated waves. The daemon becomes a process supervisor, not a second flow interpreter. New flow constructs land once in the CLI and work everywhere.

Why now: the journal v2 contract makes this possible without losing observability. The old runtime types are deleted. The WaveExecutor is the last piece of duplicate execution logic.

## Approach

Replace `WaveExecutor::execute()` with process supervision of `lf <flow> -b` (or `lf <step> -b` for single-step runs). The daemon spawns a child process, captures stdout/stderr, reconciles exit status, and reads structured progress from the journal — exactly what it already does for manual CLI runs, but now for daemon-initiated runs too.

### Command construction

```
lf <flow-or-step> -b \
  -d <direction1> -d <direction2> \
  -a <area1> -a <area2>
```

Working directory: the run's worktree path. The `-b` (batch/headless) flag is already how the CLI runs non-interactively.

### Environment injection

Before spawning `lf`, set:

| Var | Value | Purpose |
|-----|-------|---------|
| `LFD_RUN_ID` | run ID | Correlate journal events with daemon run records |
| `LFD_WAVE_ID` | wave ID | Attribute run to wave |
| `LFD_SESSION_ID` | session ID | Group runs within a session |
| `LF_RUN_ID` | run ID | CLI uses this as its journal run directory name |

The CLI's existing wave detection (filesystem-derived from sibling worktree naming) handles wave attribution. These env vars add daemon correlation on top.

### What the daemon keeps

The daemon remains responsible for everything that isn't flow execution:

1. **Scheduling & queueing** — serialized vs parallel dispatch, slot acquisition, pending activation queue, coalescing, cron/loop tickers
2. **Worktree lifecycle** — creating ephemeral worktrees and branches before the run, cleaning up after
3. **Process supervision** — spawning `lf`, capturing stdout/stderr into OutputHub, tracking PID for cancellation, enforcing agent timeout, reaping on completion
4. **Exit status reconciliation** — mapping exit code to WaveRunStatus (0 → Completed, non-zero → Failed), triggering repair chains with backoff
5. **Pre/post run ops** — git state polling, auto-PR creation on completion, branch advancement for recurring waves, queue reconciliation, trigger listener dispatch
6. **Interactive wait** — creating terminal sessions for interactive steps (WaitInteractive), watching for session completion
7. **Provider auth injection** — filtering env vars, injecting DB-backed provider tokens

### What the daemon stops doing

All flow-internal execution logic moves out of WaveExecutor:

- Flow expansion (`expand_flow`, `load_flow`)
- Step-by-step iteration with `next_action`
- Xor routing (verdict writing, reading, sub-flow execution)
- Loop body iteration and exit routing
- Fork/and worktree creation and parallel branch execution
- Fast-path attempts
- Per-step pre/post sync (commit, push, rebase)
- Step prompt assembly and agent config building
- Summary refresh between steps
- `run_step`, `run_inline_items`, `run_inline_xor`, `run_fork`

### Supervision loop

```rust
// Pseudocode for the new execute()
async fn execute(&self, run_id: &LfdId) -> Result<()> {
    let run = self.store.get_wave_run(run_id).await?;
    let wave = self.store.get_wave(&run.wave_id).await?;

    // Git state poller (unchanged)
    let _poller = self.spawn_git_state_poller(...);

    // Build command
    let cmd = build_lf_command(&run.snapshot);
    let mut child = spawn_supervised(cmd, &run, &wave).await?;

    // Stream output to OutputHub (unchanged from current agent streaming)
    let stdout_task = tokio::spawn(read_stream(child.stdout, output_ctx.clone()));
    let stderr_task = tokio::spawn(read_stream(child.stderr, output_ctx));

    // Wait for completion or cancellation
    let exit_code = tokio::select! {
        status = child.wait() => status?.code().unwrap_or(1),
        _ = cancellation_signal(run_id) => {
            kill_process(child.id());
            1
        }
    };

    // Reconcile
    if exit_code == 0 {
        complete_run(&mut run, &wave).await?;
    } else {
        fail_run(&mut run, &wave, exit_code).await?;
    }
}
```

### Journal-based progress

The spawned `lf` process writes journal events to `.lf/journal/runs/<run_id>/events.jsonl`. The existing `LfObserver` polling loop in `lfd/journal.rs` picks these up and fans them through the EventHub. Clients get step-level progress (step started, step completed, flow completed) without any new plumbing.

The daemon's `LF_RUN_ID` env var ensures the CLI writes its journal under the daemon's run ID, so events correlate automatically.

### Step-index tracking

Today the daemon tracks `run.step_index` and updates it in the store after each step. With the CLI owning step iteration, the daemon can derive step progress from journal events instead. The `StepStarted` / `StepCompleted` events carry `index` fields. The observer can update `run.step_index` as events arrive, giving the daemon the same state without executing steps itself.

### Interactive steps

Interactive steps (`WaitInteractive`) need special handling. The CLI currently treats these the same as regular steps. The daemon needs to pause and create a terminal session.

Approach: the CLI emits a `Step.escalated` event (or a new `Step.waiting` event) when it encounters an interactive step in headless mode. The daemon's journal observer catches this and transitions the run to `Waiting` status, creates the terminal session, and returns. When the terminal session completes, the daemon spawns a new `lf` process to resume from the next step.

This requires the CLI to support `--start-index N` to resume mid-flow. The CLI already has `run.step_index` tracking — this extends it to accept an initial offset.

### Run-scoped overrides

Today the daemon reads `run.snapshot` (flow, direction, area) and passes them into the flow expansion. With the CLI executor, these map directly to CLI args:

| Snapshot field | CLI arg |
|----------------|---------|
| `flow` | positional arg to `lf` |
| `direction` | `-d <dir>` (repeatable) |
| `area` | `-a <area>` (repeatable) |

The run snapshot remains the source of truth. The daemon just serializes it into CLI args.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep WaveExecutor, extract shared engine lib | Still two callers of the same logic, divergence risk remains | Doesn't actually converge — just moves the duplication |
| Daemon calls into CLI as a library (in-process) | Tighter coupling, shared async runtime complications, harder to kill/timeout | Process boundary is the right isolation for supervised execution |
| gRPC/IPC between daemon and CLI | Over-engineered for local execution; the filesystem journal already provides the communication channel | Adding a protocol when file I/O works is unnecessary complexity |
| Gradual per-construct migration (xor first, then loop, then and) | Safer incremental path | Creates a longer dual-path period where both old and new code run. The wave item guidance says "dual-path first, swap default once parity is proven" — but we should minimize the dual-path window |

## Key decisions

**Process boundary, not library call.** The daemon spawns `lf` as a child process, not as an in-process function call. This gives us: clean cancellation (kill the process), timeout enforcement, output capture, crash isolation, and the ability to run different `lf` versions during upgrades.

**Journal for progress, not stdout parsing.** The daemon does not parse CLI stdout to understand step progress. It reads structured journal events. This is already implemented and working for manual runs.

**`--start-index` for resume.** Rather than requiring the CLI to checkpoint and resume internally, the daemon can restart a flow from a specific step index. This keeps the CLI stateless across invocations while letting the daemon manage paused/resumed runs.

**Dual-path transition.** Ship behind a feature flag (`executor: cli` vs `executor: legacy` in wave config or daemon config). Both paths coexist temporarily. Integration tests run both paths and compare outcomes. Once parity is proven across serialized/parallel/reactive/ci-fix runs, the legacy path is deleted.

**Pre/post run ops stay in daemon.** Worktree creation, auto-PR, branch advancement, trigger dispatch — these are supervision concerns, not flow concerns. They stay in the daemon, wrapping the `lf` process spawn.

**Per-step sync moves to CLI.** The CLI's `commit_step_work()` already handles post-step commits. The daemon's `pre_step_sync` / `post_step_sync` in `helpers.rs` can be ported to the CLI's flow runner. The daemon doesn't need to sync between steps if it's not executing steps.

## Scope

**In scope:**
- `WaveExecutor::execute()` replacement with process supervision
- Environment variable injection (`LFD_RUN_ID`, `LFD_WAVE_ID`, `LFD_SESSION_ID`, `LF_RUN_ID`)
- CLI `--start-index` flag for mid-flow resume
- Journal observer updates to track `step_index` from events
- Step-level progress derived from journal events
- Integration test harness comparing CLI-executor vs legacy-executor outcomes
- Feature flag for dual-path transition
- Deletion of legacy execution path once parity is proven

**Out of scope:**
- Daemon-hosted shells/PTYs (item 02)
- Push-based journal ingestion (inotify/kqueue) — 1s poll is fine for v1
- Escalation signal type in CLI — can land as follow-on or alongside
- Changes to scheduling, queueing, or trigger logic
- Docker executor changes (Docker path wraps `lf` the same way)

## Done when

- `lfd` starts automated runs by spawning `lf <flow-or-step> -b` with appropriate args and env vars
- The daemon tracks step-level progress via journal events, not by executing steps itself
- Interactive steps pause the run and resume via `--start-index`
- `WaveExecutor::execute()` is a supervision loop (~100 lines), not a flow interpreter (~1200 lines)
- Integration tests prove parity: serialized waves, parallel waves, queued activations, ci-fix, cancellation, failure propagation, run-scoped overrides, xor routing, loop flows, fork/and execution
- The legacy execution path is deleted

Wave goals advanced: "lf can write structured lifecycle events into a shared runtime store" (already done), "lfd starts normal lf commands for automated runs" (this item), "duplicate execution semantics between lfd and lf: trending to 0" (this item converges them).
