# Open Questions

- Queue reconciliation invoked from run completion currently uses `GitHubConfig::default()` inside `WaveExecutor` (no token threading yet), so it relies on locally upserted live PR state and background poll/webhook refresh for authoritative GitHub state.
- `queue_role` and `next_action` are projected from stack order + live/cache state; no canonical persisted queue role is stored. If downstream consumers need stable historical queue roles, we need an explicit event/audit log.
