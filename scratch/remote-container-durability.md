# Container Durability & Recovery

Picked from `roadmap/remote/01-sandboxed-agents.md` — Stage 01B follow-up.

## Problem

lfd's Docker executor tracks running containers in an in-memory `HashMap<String, String>` (`active` field — agent_id → container_id). When lfd restarts, that map is empty. Running containers become invisible: lfd can't stop them, can't stream their logs, and can't detect their exit. The `fail_orphaned_runs()` call on startup marks all in-flight runs as Failed, but the actual Docker containers keep running until they finish or until someone manually kills them.

This means:
- Agent containers run to completion with nobody watching. Their output is lost.
- If a wave has a recurring stimulus, lfd may spawn a new container for the same wave while the old one is still running.
- There's no path from "lfd crashed" back to "agent output resumes in Concerto."
- Orphaned containers accumulate across crashes — nothing removes them.

## Approach

Two additions to the startup path, both in the Docker executor:

### 1. Rehydrate — reconnect to surviving containers

On startup (after store init, before starting background loops):

1. Query the agents table for Running agents.
2. For each, check if a Docker container named `lfd-agent-{agent_id}` still exists and is running (via `docker inspect`).
3. If yes: populate the in-memory `active` map and spawn a log-streaming task. The existing `WaveExecutor::execute` loop for that wave run is gone (it died with the old process), so we need a lightweight "reattach" loop that:
   - Streams logs to OutputHub
   - Waits for container exit (`docker wait`)
   - On exit: syncs worktree back to host, records exit code, updates agent/run status, removes container
4. If no (container gone): mark the agent as Failed, mark the wave run as Failed with error "container lost during lfd restart."

This replaces the current `fail_orphaned_runs()` blanket for Docker mode. For local process mode, `fail_orphaned_runs()` stays as-is — you can't reattach to a forked process after the parent dies.

### 2. Cleanup — remove orphaned containers

After rehydration, enumerate all Docker containers with label `io.loopflow.managed=true`:

1. `docker ps -a --filter label=io.loopflow.managed=true`
2. For each container not in the `active` map (i.e., not rehydrated): stop and remove it.
3. Log each removal at `info` level.

This catches containers from previous lfd instances that are no longer tracked — crashed mid-run, leftover from a bug, etc. Only loopflow-labeled containers are affected.

### Container labels — enrich for rehydration

Currently containers get one label: `io.loopflow.managed=true`. Add:

| Label | Value | Purpose |
|-------|-------|---------|
| `io.loopflow.agent-id` | agent LfdId | Map container back to agent record |
| `io.loopflow.wave-id` | wave LfdId | Quick filtering by wave |
| `io.loopflow.wave-run-id` | wave run LfdId | Associate with run for log routing |

These labels make `docker inspect` self-sufficient — we can rehydrate without querying the store for container→agent mapping (the container name `lfd-agent-{id}` already encodes this, but labels are the canonical Docker way and survive renames).

### Database changes — persist container_id

Add `container_id TEXT` column to the `agents` table (migration 003). The `pid` column currently stores the process ID for local mode; for Docker mode, we store nothing useful there. `container_id` is the Docker container ID (64-char hex), written at container creation time.

This gives us a second path to find the container on restart: even if the naming convention changes, the persisted container ID is authoritative.

### Reattach loop — lightweight execution tail

The reattach loop is not a full `WaveExecutor::execute`. It's a focused function:

```rust
async fn reattach_agent(
    docker: Docker,
    store: SharedStore,
    output: OutputHub,
    agent: Agent,
    container_id: String,
    wave_run_id: LfdId,
) -> Result<i32>
```

It does exactly what the tail of `DockerExecutor::run` does after `start_container`:
1. Spawn `stream_logs` task
2. `docker.wait_container` for exit
3. Await logs task completion
4. Remove from `active` map
5. Sync worktree to host
6. Remove container

It does NOT re-prepare the workspace, re-run hygiene, or advance the flow. The wave run's step execution already happened — we're just collecting the result.

After `reattach_agent` returns, the flow cannot resume from where it left off (the step iterator state lived in the old process). The run completes or fails based on the agent's exit code, and the next stimulus tick picks it up fresh.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Persist full execution state (step index, plan, etc.) and resume mid-flow | Full resumability across restarts | Massive complexity — the executor's control flow is a loop with local state, closures, and branching. Serializing that is a rewrite. The payoff is small: most runs are single-step, and multi-step flows restart quickly. |
| Kill all containers on startup (current behavior) | Simple, predictable | Wastes work. A 30-minute agent run that's 29 minutes in gets killed for nothing. Agents are expensive (API costs, time). Reconnecting is strictly better. |
| Use Docker restart policies instead of lfd management | Docker handles restarts natively | Wrong abstraction — we need to manage the agent lifecycle (log routing, status tracking, worktree sync), not just keep a process alive. Restart policies also re-run the same command, which may not be idempotent. |
| Store container_id in the existing `pid` column | No schema change | Confusing dual-purpose column. `pid` is `INTEGER` in the schema; container IDs are 64-char hex strings. Storing a string in an integer column relies on SQLite's type affinity, but breaks on Postgres. A new column is cleaner. |

## Key decisions

**Reattach, don't resume.** We reconnect to the running container and collect its result, but we don't try to resume the flow from the interrupted step. The flow restarts from scratch on the next stimulus. This is the right tradeoff: reattach handles the common case (single-step run in progress) without the complexity of serializing executor state.

**Container labels over container names.** Names encode the agent ID by convention (`lfd-agent-{id}`), but labels are the Docker-native way to query and filter. We use both: names for human readability, labels for programmatic lookup.

**New column, not overloaded pid.** The `pid` column stays as-is for local mode. `container_id` is a new nullable TEXT column. Clean separation, no type gymnastics.

**Rehydrate before cleanup.** Order matters: first identify which containers are ours and still useful, then kill the rest. If we cleaned up first, we'd kill containers we could have reconnected to.

**Log gap is acceptable.** Lines emitted between lfd crash and restart are lost (the OutputHub wasn't listening). The alternative — persisting container logs to a Docker log driver and replaying — adds complexity for marginal value. Agents produce structured output; a few lost lines during a crash don't break the flow.

Follows the remote wave principle: "lfd orchestrates containers" — durability keeps the orchestrator in control across restarts. Follows the sandbox principle: only loopflow-labeled containers are touched.

## Scope

- In scope:
  - Persist `container_id` in agents table (schema migration)
  - Write container_id at creation time in DockerExecutor::run
  - Enrich container labels with agent-id, wave-id, wave-run-id
  - Startup rehydration: inspect surviving containers, populate active map, spawn reattach loops
  - Startup cleanup: remove orphaned loopflow-labeled containers
  - Tests: rehydration with mock Docker, cleanup of orphans, label verification

- Out of scope:
  - Full flow resumption (step iteration, plan advancement) after restart
  - Worktree cleanup for dead containers (handled separately in cleanup_wave)
  - Docker log driver configuration
  - Volume cleanup (volumes persist by design; cleanup is a separate concern)

## Done when

- `cargo test` passes with new rehydration and cleanup tests
- lfd can restart and reconnect to a running agent container (observable: logs resume in Concerto after daemon restart)
- Orphaned containers are cleaned up on startup (observable: `docker ps` shows no stale `lfd-agent-*` containers after restart)
- Only `io.loopflow.managed` containers are affected by cleanup
- `container_id` is persisted in the agents table and survives daemon restart
- Stop/delete operations work for rehydrated containers (the active map is populated, so terminate() finds them)
