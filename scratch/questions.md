# Open Questions / Assumptions

- Assumed `POST /hooks/git` should reject non-absolute `repo` paths with `400` and canonicalize accepted paths before emitting events.
- Assumed `LFD_DB_PATH` should now be relative to `~/.lf` (absolute overrides rejected) to enforce the new root-boundary invariant.
