# How plan sync across Machines got here (read 2026-11-18)

- LOO-406 (closed): the store is the home of the plan, Linear optional.
  Its brief: "Git plan synchronization, offline replay, multi-person plan
  authority changes ... are separate follow-ups". "No shared resident process."
- #1491 (2026-10-12, LOO-412 transport): "Share saved Work across Machines
  through Git or Linear". Every Machine journals WorkEdits with field
  causality and exchanges them over a planning ref; "machines acquire and
  deliver independently". Peer-to-peer, multi-writer, no server.
- Infrastructure Wave memory, "Plan ownership" (2026-11-03): "Sessions,
  Processes, checkouts, reservations and execution remain machine-local."
- LOO-552 (closed, 2026-10-21): Jack Heart: "lets try the idea of having a
  default global ref." Shared `refs/loopflow/planning/shared/main`.
- LOO-523: Tasks owned as (person, Machine) with an append-only ownership log.
- LOO-868 / #1930 (2026-11-14), LOO-899: patches to the peer import's
  write lock and history gaps.
- LOO-861 (open, 2026-11-12): one always-on Machine is the hub everyone
  dials. Its brief: "The hub relays and keeps presence ... It executes nothing
  for others and reads no Work of the Machine it runs on." "the hub stores
  presence, not other Machines' readings." Open: what the phone shows for a
  sleeping Machine's Tasks.
- #1956 (2026-11-18, LOO-926): a Task's writes run on the Machine that runs
  it, through that Machine's server.
- LOO-925 / #1957 (2026-11-18): Linear/Asana through `tracker_actions`;
  Git left as a peer journal. Jack Heart: "3a for now, possibly a follow up
  task?"
- LOO-938 / #1971 (open): cost fixes to the peer journal. Jack Heart,
  2026-11-18: "the plan syncing was written in a pre-machine world"; "I think
  i decided towards ahving one plan owner machine server now"; "i would
  prefer to just turn this task into the proper design rather than continue
  to churn".

No written record found of the plan-owner decision (searched git history,
Wave memory, Task briefs, Claude transcripts). LOO-861's hub, as written,
is relay-only and holds no Work.
