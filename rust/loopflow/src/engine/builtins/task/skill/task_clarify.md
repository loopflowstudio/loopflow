---
description: Turn one Linear Task directive into a computable change design.
action_style: procedural
---
Clarify the exact Task owned by the current Run.

Read the Task seed, current durable direction, Project definition/KRs, repository
instructions, current worktree, and any existing design note in `scratch/`.

- Honor every Steer included in the seed. The boundary Basis is fixed; do not
  invent an acknowledgement mutation or treat provider delivery as application.
- Keep the design to this Task's one worktree and ordered serial PRs.
  Do not select backlog work, start another Task, or create a second
  worktree. The Task may require several PRs.
- Write or tighten the single Task design note only when the change is not yet
  computable. Preserve a clear existing design.
- Resolve reversible ambiguity with the simpler path. When a choice changes
  scope, behavior, or authority, run `lf ask "<exact question>"`; continue the
  same Turn after the parent Ask settles.
- Do not implement beyond a trivial probe that makes the design computable.

## Computable design contract

Before leaving clarify, make the design note state:

- **User-visible outcome** — whose behavior changes and what they can observe
  when the Task holds.
- **End-to-end proof** — one concrete scenario that crosses the source of truth
  and every affected consumer, plus the command, test, or observation that
  proves the outcome.
- **Source of truth** — the authoritative persisted record, model, or API and
  which views are derived from it.
- **Affected surfaces and consumers** — every CLI, wire DTO, app, automation,
  or downstream reader that must change or remain compatible.
- **Absent and error states** — what missing evidence, empty state, invalid
  input, or failed dependency means at each affected boundary.
- **Operational boundary** — when relevant, the latency, subprocess, network,
  scale, or recovery budget the implementation must hold.
- **Exclusions** — adjacent behavior deliberately left outside this Task.

Files changed, migrations applied, tests added, and a PR opened are
implementation receipts. They may support the proof, but they are not the
finish line; the design must end in an observable condition.

Leave the pursue phase a concrete build and verification target. The Task
runner advances the flow; write no loop bit.
