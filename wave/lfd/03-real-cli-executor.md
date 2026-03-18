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

## Open questions

- What should the stable machine-readable invocation boundary be: flags, env vars, temp config, or some combination?
- Which pieces of today's executor need to survive as supervision helpers instead of disappearing entirely?
- How do we stage the migration without breaking existing automation paths?

## Done when

- Automated runs launched by `lfd` execute normal `lf <flow-or-step>` commands
- The daemon still owns scheduling, queueing, cancellation, and persistence
- Flow semantics no longer need to be implemented twice
- Reactive runs like CI-fix and repo-triggered activations use the same command model
- Tests prove parity between daemon-started runs and the standalone CLI path
