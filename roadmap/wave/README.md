# Wave Roadmap

This folder tracks wave-specific features: the stateful, daemon-backed orchestration layer.

## Relationship to Rust roadmap

Wave features depend on the Rust port:
- Stage 2 (lf-core) provides the execution engine
- Stage 3 (daemon service) provides the control plane
- Stage 4 (lf-client) adds git operations to lf-core

The `lf ops` refactor is part of Stage 4 - see `roadmap/rust/04-lf-client.md`.

Wave-specific work (lfd commands that add state on top of lf-core) follows after.

## Files

- `ops-architecture.md` — design reference for lf ops vs lfd relationship
