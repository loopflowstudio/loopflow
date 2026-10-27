# LOO-615 open questions and assumptions

- **Rule 2 (keep a finished child while its parent runs)** goes beyond the
  wording of LOO-602 decision 5. Assumed it is required: without it, Graph steps
  and `task run` attempts that finish before their parent's registration poll
  lose their results. Reversible; it changes only what is kept.
- **Desktop Task-content reads move outside the checkout** (`RegistryQuery.swift`).
  Assumed this is in scope as the minimum consumer cutover. Once recorded,
  their rows would otherwise count as Task execution and could make `lf land`
  refuse.
- **Harness callbacks (`__context-block`, `__pi-event`, `__tool-lifeline`) are
  not agent-issued.** Assumed, so hook rows never become permanent agent history.
- **Agent-issued rows are still kept forever** (~1k/day, ~150 MB/year). This
  follows decision 5 as accepted. If that growth matters, it is a separate
  decision.
- **Hooks are admitted, not exit-observed** (implement, 2026-10-26). General
  admission runs before `execute_command`, so `__context-block`, `__pi-event`
  and `__tool-lifeline` write a row while running and delete it at exit: two
  `store_revisions.processes` bumps per hook. Kept that over a hook denylist,
  per the design's rejected alternative; gate's Work-watch frame check decides
  whether dedup is needed.
- **Exit-only observation also writes under an inherited `LF_PROCESS_ID`.**
  Otherwise a Graph step rejected at argument parsing leaves its running Graph
  no step to record. A child of an already-finished parent is deleted in the
  same transaction.
- **Other Desktop Task-scoped reads** (audited at kickoff). Comments, history
  and context run from the repository origin. The Debug Graph-process read
  now runs with `nil` cwd like the rest.
- **A forgotten Process answers `Unknown` liveness.** Kept (kickoff,
  2026-10-26): an absent row is also an unregistered child, and only running
  parents hold such ids in memory, which rule 2 covers.
