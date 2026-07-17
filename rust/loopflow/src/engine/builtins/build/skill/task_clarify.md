---
description: Turn one Linear Task directive into a computable change design.
default_agent: codex
action_style: procedural
---
Clarify the exact Task named by `LF_TASK_SESSION_ID`.

Read the Task seed, current directive, Project definition/KRs, repository
instructions, current worktree, and any existing design note in `scratch/`.

- Acknowledge the current directive with the exact command in the session seed
  before editing.
- Keep the design to this Task's one worktree and ordered serial PRs.
  Do not select backlog work, start another Task Session, or create a second
  worktree. The Task may require several PRs.
- Treat required Kickoff, Iterate, and Gate checkpoints uniformly: each is a
  durable provider-backed InteractionReview conversation. The GitHub UI and a
  merge click are never managed Task lifecycle authority.
- Write or tighten the single Task design note only when the change is not yet
  computable. Preserve a clear existing design.
- Resolve reversible ambiguity with the simpler path. Request a durable
  supervisor decision when the choice changes scope, behavior, or authority.
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
