# Container Durability & Recovery

Picked from `roadmap/remote/01-sandboxed-agents.md` — Stage 01B follow-up.

## What

Persist container metadata in run state so lfd can rehydrate running containers across daemon restart. Aggressive startup cleanup for loopflow-labeled containers that were orphaned.

## Scope

Two concerns:

### Durability
- Persist container metadata (container ID, volume, worktree path, wave ID) in run state
- On daemon restart, rehydrate container handles from persisted state
- Reconnect log streams for containers that survived the restart
- Detect containers that died while the daemon was down

### Recovery
- On startup, enumerate all loopflow-labeled Docker containers
- Stop and remove orphaned containers (labeled as loopflow but not in persisted state)
- Clean up dangling worktrees from dead containers
- Only touch loopflow-labeled containers — never interfere with user containers

## Done when

- lfd can restart and reconnect to running agent containers
- Log streaming resumes after daemon restart
- Orphaned containers are cleaned up on startup
- Only loopflow-labeled containers are affected by cleanup
- Container state survives daemon restart for stop/delete/recovery paths
