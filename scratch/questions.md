# Open questions / follow-ups

- Docker mode currently mounts the active worktree path as a bind mount at `/workspace` instead of using per-repo Docker volumes for clone/worktree lifecycle. This keeps the first draft shippable but does not yet enforce the full repo-in-volume model from the design.
- Active Docker container IDs are tracked in-memory by `DockerExecutor` for cancellation/recovery. They are not yet persisted in the store, so daemon restarts can lose direct terminate handles for already-running containers.
- Credential mounts in `lfd.yaml` are parsed as explicit `host_path:container_path` strings. We should confirm whether we want first-class structured mount config before making this user-facing.
- Docker execution behavior was validated by unit tests and compilation, but not by an end-to-end runtime test against a live Docker daemon in this run.
