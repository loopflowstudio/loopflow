---
description: Implement one pass of work toward the task PR.
action_style: procedural
---
Work the task PR.

## Orientation

Read the task seed in `<lf:message>`, then `scratch/<branch>.md` and
`scratch/questions.md` if present. If the seed names a filed task, read that
record. Inspect related or in-flight work when it can reveal conflicts,
dependencies, or reusable context; do not turn task execution into backlog
selection. Follow the repo style guide.

## Work

- Honor every Steer included in the seed and summarize how it changes the
  execution plan. The boundary Basis is fixed; provider acceptance alone is
  not application. Do not create a separate acknowledgement mutation for inputs
  already present in the seed.
- Own this bounded attempt in the supplied worktree. Operational Loopflow children such
   as `lf commit`, `lf pr land`, `lf rebase`, and direct skill or flow calls are
  part of that attempt and remain available. Integration is accepted through
  the Task Basis and worktree mutation boundary, not process seniority. Do not boot a server, create a
  second Task, or delegate the task seed. If scoped PM reads fail,
  note the failure and continue from the seed rather than repairing auth.
- Delegate only bounded, independent checks through the execution tools already
  available to this process, and keep responsibility for integrating the result.
- Implement the smallest coherent slice described by the design doc. Check it
  against both its focused proof and the complete target architecture. Update
  only `This slice` and the slice ledger; never replace the north star with a
  local implementation plan.
- Before adding a type, store, writer, or launch path, identify the existing
  concept that should own the behavior and what becomes obsolete. A v2,
  Legacy/New split, adapter, fallback, dual write, or parallel authority is
  blocking unless the reviewed design explicitly justifies it and names its
  deletion point.
- Use a relevant Project metric when one already covers the outcome. While
  building feature work, notice signals that could help the Project steer and
  propose the useful ones back to it: name the outcome, the candidate measure,
  why it would change a decision, and the cheapest credible producer. When the
  first useful signal belongs naturally in this coherent change, ship it;
  otherwise leave the proposal for Project sponsorship. A substantial new UI
  performance path is a strong reason to look. Metric proposals are discoveries,
  not a completion quota.
- Add or update tests for user-visible behavior.
- Run the narrowest verification that covers the touched code.
- When progress requires another Work's perspective, launch an ordinary
  `lf --as project:<id> : "<prompt>"` Run. When it requires human authority and
  no human is present, run `lf ask "<exact request>"` and wait for the human to
  complete that conversation. Do not invent a provider-specific decision command.
- Use `lf pr publish` when the branch has a reviewable PR-shaped change; it
  pushes and creates or refreshes the PR without opening a browser. Reach for
  `lf pr open` only when a human explicitly asked to see the PR for review.
- Do not land or complete the Task from this loop. The pinned final flow owns
  its gate, learning record, and landing disposition.
- File a concrete follow-up with `lf pm task create` when new work belongs later
  under a known project. Filing does not authorize launching it in this task.
- Report consequential progress through the Task; its linked events
  keep the owning Wave informed without copying raw tool chatter.

If implementation produces a counterexample to the design's authority model,
deletion path, or full-system proof, stop dependent work and return to design
review. Do not note the contradiction and continue building on it.

Stay scoped to the task. Put unresolved ambiguity in `scratch/questions.md`.
