# LOO-334 decisions and assumptions

2026-09-29. Draft design: [resolve-tasks-from-linear-and.md](resolve-tasks-from-linear-and.md).

## Accepted direction

Jack Heart selected per-Wave planning authority: Linear when connected, local
storage otherwise; official installed lf for workers by default; explicit
pinning retained. Linear is not a prerequisite for operating local Tasks.

## Product decisions still open

- **Wave existence:** Jack explicitly left this undecided. Candidate for review:
  connected Initiative / unconnected local store own existence; Git owns authored
  content and connection configuration. This kickoff does not implement Wave
  creation/deletion policy or silently ratify the candidate.
- **Discard mismatched planning:** proposed interpretation of Jack's direction
  replaces connected planning facts while preserving execution, authored work
  and historical provenance. Confirm this distinction before any destructive
  cleanup. Reads and the first implementation slice need no destructive choice.

## Safe design choices for review

- TaskSpace and TaskOps are conceptual boundaries in existing code, not services.
  Unavailable provider evidence remains unknown; cancellation is not completion.
- A local-only Task remains owned by its original store. Other known stores can
  locate it without copying it. Unknown arbitrary paths cannot be discovered
  without an address; report the search boundary explicitly.
- Official means the currently selected machine installation. Select bytes at
  each worker boundary while preserving the existing execution directory.
  Explicit runtime policy is recorded on TaskOps; inherited LF_BIN is not a pin.
- Do not consolidate provider accounts or quota readings in this Task. Display
  their owning execution location; credential sharing is a separate decision.
- Creating a Task must not require rotating a chapter to populate a local
  receipt. Resolve an existing provider Project or expose actual ambiguity;
  do not reconstruct chapter history from an In Progress label.

## Coordination blocker

The authorized ordinary Run request through official
`lf --as task:LOO-298 -b : ...` failed with `Task "LOO-298" is not registered`.
No Run launched and no reply established the latest execution-schema contract.
The supplied Wave memory is prior design evidence only. Retry coordination
before shared execution migrations; do not edit LOO-298's branch, copy stores,
repair auth, or claim agreement on the basis of this kickoff.

No conversational question or blocking Ask was needed to produce this reviewable
draft. The open product decisions do not prevent the first read-only resolution
slice; they remain explicit for design review rather than being inferred as
approved implementation scope.
