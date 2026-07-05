---
requires: components.md (the charter), the M0-M4 roadmap arc
produces: the post-M2 architecture — one picture, checked against the doctrine
---
# The world after M2

Substrate: the filesystem is the database. Per-channel journals (origin +
worktrees), GOAL.md/MEMORY.md as identity/knowledge, endpoint+token files,
git/gh as PR truth, keychain tokens. NO database engines anywhere.

Processes: LISTENER (lf wave; the only pens; hear/check/fold/tell) —
RESIDENT (lf wave --mind-only; owns the harness crate; spawned by, wired to,
its listener) — WORKERS (lf … --dispatch z; tmux; speak lf chat, hear
ambient context, may lf sub) — GATEKEEPER (lfd serve, optional: in-memory
scan-built index, read routes + /ws push, webhooks → exec lf --from,
remote exec-lf door under client identity) — VIEWERS (Concerto local via
files+SSE; loopflow python lib; remote via the gate).

Checks: route-around (gate removable, local flows intact); one pen per
journal; only non-file state = vendor threads + the gate's rebuildable RAM.

(Rendered diagram in the session log, 2026-07-05; this doc is the durable
summary. M1 = the shape converges on components.md; M2 = this substrate.)
