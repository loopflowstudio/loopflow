# 02: Real CLI Executor

**Finish line:** when `lfd` decides a run should start, it supervises a normal `lf <flow-or-step>` process in the correct worktree and environment instead of executing flows through a second bespoke daemon executor.

## Context

Today the daemon still carries its own deep execution path. That creates semantic drift: the CLI and the daemon each have to know how flows run, how overrides resolve, how interactive waits behave, and how results map back into run state. The longer both paths coexist, the more bugs land in the seam between them.

`lfd` should stay responsible for scheduling, queueing, worktree choice, and process supervision. It should stop being the place where loopflow execution semantics are reimplemented.

## What to build

1. **Run startup via `lf`.** When `lfd` starts an automated run, spawn the real CLI with the run's chosen flow/step, repo, worktree, env, and daemon-aware metadata.

2. **Headless supervision.** Preserve current daemon responsibilities:
   - process lifecycle
   - stdout/stderr capture
   - cancellation / stop
   - exit status reconciliation
   - trigger / queue integration

3. **Run override parity.** Ensure run-scoped overrides like `flow`, `area`, and `direction` map cleanly onto the spawned CLI command or its environment. The run snapshot remains authoritative for that execution.

4. **Executor convergence.** Remove or shrink duplicate in-daemon execution logic as parity lands. Keep one implementation of flow semantics.

5. **Regression coverage.** Add tests for:
   - serialized vs parallel waves
   - queued activations
   - CI-fix / reactive runs
   - cancellation
   - failure propagation
   - run-scoped flow overrides

## Design guidance from tmux study

### Server as supervisor, not executor

tmux's server owns the PTY and child process lifecycle but never interprets what the child does. It supervises: start, signal, reap, track exit status. The shell semantics live in the shell. This is exactly the separation `lfd` should achieve: supervisor for `lf` processes, not a second flow interpreter.

What tmux's server does that `lfd` should copy:
- **Owns process lifecycle.** Fork, exec, waitpid. Knows the PID. Can signal (SIGTERM, SIGKILL).
- **Captures exit status.** Maps it to session/run state.
- **Environment injection.** tmux sets `TMUX`, `TMUX_PANE`, and session env vars before exec. `lfd` should set daemon-awareness env vars the same way — before the process starts, not via side channel.
- **Does not interpret output.** tmux routes bytes from PTY to grid/client. It never parses what the shell is doing. `lfd` should route structured events from `lf`, not parse terminal output.

### Process supervision details

tmux handles zombie reaping, signal forwarding, and process group management internally. Key details for `lfd`:
- tmux sends `SIGHUP` to the process group on session kill (configurable via `remain-on-exit`)
- exit status flows through `waitpid` → session state update → client notification → potential session destruction
- `remain-on-exit` lets dead panes stay visible until explicitly killed — worth considering for failed agent runs where the conductor wants to inspect terminal state before cleanup

### Environment as the invocation boundary

tmux's handshake sends environment variables from client to server (`MSG_IDENTIFY_ENVIRON`). For `lfd` spawning `lf`:
- Set `LFD_RUN_ID`, `LFD_WAVE_ID`, `LFD_SESSION_ID`, `LFD_STORE_URL` (or socket path) as env vars
- These are the detection contract from 02 — `lf` checks for them on startup
- Don't use flags for daemon-aware behavior — env vars compose better with shell wrappers, `exec`, and process supervision

## Open questions

- What should the stable machine-readable invocation boundary be: flags, env vars, temp config, or some combination? (Guidance: env vars. tmux uses this pattern. Flags force CLI parsing; env vars compose cleanly.)
- Which pieces of today's executor need to survive as supervision helpers instead of disappearing entirely? (Guidance: process lifecycle, exit-status reconciliation, cancellation signaling. Everything that tmux's server does. Not flow expansion, step resolution, or prompt assembly.)
- How do we stage the migration without breaking existing automation paths? (Guidance: dual-path with feature flag. Run the real CLI path alongside the bespoke executor, compare outcomes, swap default once parity is proven. tmux migrated control mode incrementally — early versions had fewer notifications, iTerm2 adapted.)

## Done when

- Automated runs launched by `lfd` execute normal `lf <flow-or-step>` commands
- The daemon still owns scheduling, queueing, cancellation, and persistence
- Flow semantics no longer need to be implemented twice
- Reactive runs like CI-fix and repo-triggered activations use the same command model
- Tests prove parity between daemon-started runs and the standalone CLI path
