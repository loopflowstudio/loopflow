---
requires: scratch/collapse.md (running in ../loopflow.collapse), the essence/incident audit (2026-07-05)
produces: the wave-server realignment — channels, the pure listener, the extracted mind
---
# Realign: the wave server becomes what we now know it is

This branch takes the realignment implied by the walk: the channel is the
essence (hear / check / fold / tell + two pens); the mind is a resident.
Grammar (lf step/lf flow) and the dispatch-verb question stay on the
language roadmap item — NOT this branch.

## Wave A — wire realignment (one dispatch)
- Unified `/events` stream: turn + state + memory events on one SSE (hard
  cut of /conversation/stream's turn-only contract; Concerto client updates).
- `lf sub [NAME] [--json]` — subscription as a verb: follow a wave's events
  until killed; reconnect/backoff re-resolving the endpoint; LOOPFLOW.md
  Speak section gains the teaching (now that the command exists).
- `/health` splits channel-liveness from resident-status (dormant channels
  have no mind).
- The door forwards resident-directed ops opaquely (steer/interrupt
  semantics leave the ear).
- `WorkerDispatched/WorkerFinished` → `RunObserved/RunCompleted`.
- `ServerStarted` journal event (decision made: the janitor already leaks
  process lifecycle into the record; make it honest — restarts become
  forensically visible; vetoable).

## Wave B — channels
Per-channel journals, name-addressed (`goals`, `goals.<runid>`); the server
serves a channel FAMILY (parent listener holds children's pens, folds
upward); `lf chat`/`lf sub`/ambient context address by name/prefix;
`lf q worker run` mints the work line's channel; consumption markers stay
journal-local. Wave = identity; channel = stream; promotion is vocabulary.

## Wave C — the extraction (phase 2, this branch's final act)
mind.rs + state.rs + the TurnSink/interrupt paths leave the server. The
turn-delta door becomes wire (full DTO discipline + fixtures); the mind is
an lf process whose input is its subscription (lf sub's second customer)
and whose home is <repo>.<wave> (worktree bootstrap moves to the resident);
auto-revive = process respawn; registry rows split server/mind. Server =
hear / check / fold / tell, ~4k lines, vendor-free.

Then: compress, 8-angle review, fix wave — same pipeline as the wave-agent
branch. Cron-into-the-mind is collapse wave 3's item (ordering guard:
built before the poller dies).
