# Review: Restart Current Step

## What was implemented

Added `POST /v0/waves/{wave_id}/restart-step` endpoint that kills the running agent for the current step and relaunches it without creating a new WaveRun or resetting step_index. The flow continues normally after the restarted step completes.

Full stack: Rust handler + route registration + DTO, Swift service protocol + HTTP implementation + RepoState action + UI button on the active flow progress pill.

## Key choices

**Reuse existing WaveRun** — The run stays alive with status `Running` and unchanged `step_index`. This is the core design decision: restart-step is a re-execution of the current step, not a new run. Avoids creating orphaned or duplicate runs.

**Extracted shared helpers** — `mark_active_agents_failed`, `respawn_run_task`, and `postWaveCommand` were factored out during implementation, reducing duplication across `stop_wave_handler`, `continue_wave_handler`, and the new `restart_step_handler`. The stop handler's agent-ending logic moved from inline to `mark_active_agents_failed` — this is a minor behavioral consolidation (same store call, just moved).

**No confirmation dialog** — Restart is non-destructive (the wave stays running), so no confirmation is needed. The icon appears inline in the active pill to keep the interaction fast.

**Precondition: Running only** — Returns 412 if the run isn't in `Running` status. Restart doesn't make sense for Waiting (use continue), Failed (use retry), or Completed.

## How it fits together

```
FlowProgressPills (active pill) → onRestartStep callback
  → WaveDetailPanel.restartStep() → RepoState.restartStep()
    → LocalWaveService.postWaveCommand("restart-step")
      → POST /v0/waves/{id}/restart-step
        → terminate_active_agents + mark_active_agents_failed + respawn_run_task
```

The handler kills agents, marks them failed in the store, re-acquires a scheduler slot, and spawns the run task with the same WaveRun (same step_index). The executor picks up where it left off.

## Risks and bottlenecks

**Scheduler slot acquisition** — If all slots are taken (unlikely for a restart since we just freed one), the endpoint returns 503. The freed slot from killing the agent should be available immediately, but there's a theoretical race if another wave grabs it first.

**No agent cleanup verification** — `terminate_active_agents` sends kill signals but doesn't wait for confirmation. The respawned step could overlap briefly with a dying agent. In practice, `kill_process` is synchronous (SIGKILL), so this is unlikely.

## What's not included

- **Restart a specific step by index** — Only restarts the current step. Restarting arbitrary past steps would require resetting step_index and is a different feature.
- **Python API / CLI** — No `lfq` or Python client changes. The HTTP endpoint is available but not yet exposed in the Python layer.
- **Tests for the handler** — No new Rust integration tests for the endpoint itself (consistent with other handlers in this file, which also lack dedicated HTTP-level tests). All existing tests pass.
