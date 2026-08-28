---
description: Turn one Linear Task directive into a computable change design.
action_style: procedural
---
Clarify the exact Task named in the seed. The Run records one attempt; the Task
Basis, not Run attribution, governs planning changes.

Read the Task seed, current durable direction, Project definition/KRs, repository
instructions, current worktree, and any existing design note in `scratch/`.

Use the Project's KRs and any metrics named in the direction to understand the
outcome. Do not force the design to invent a metric before the work reveals the
useful signal; feature implementation may expose a better proposal.

- Honor every Steer included in the seed. Do not invent an acknowledgement
  mutation for inputs already present in the seed.
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
- **Current and target architecture** — the concepts, authorities, persisted
  records, writers, and launch paths before and after the change; state what is
  reshaped and what becomes obsolete.
- **Forbidden outcomes** — duplicate representations, Legacy/New splits,
  adapters, fallbacks, dual writes, or locally passing states that would still
  violate the intended architecture.
- **Internal slices** — for an indivisible change, keep the complete end state
  intact, mark one `This slice`, and append evidence to a slice ledger rather
  than replacing the design with a narrower plan.

Files changed, migrations applied, tests added, and a PR opened are
implementation receipts. They may support the proof, but they are not the
finish line; the design must end in an observable condition.

Leave the pursue phase a concrete build and verification target. The Task
runner advances the flow; write no loop bit.
