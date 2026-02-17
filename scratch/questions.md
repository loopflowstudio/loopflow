# Open Questions

- `queue_role` and `next_action` are projected from stack order + live/cache state; no canonical persisted queue role is stored. If downstream consumers need stable historical queue roles, we need an explicit event/audit log.
