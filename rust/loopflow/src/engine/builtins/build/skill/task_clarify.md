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
- Keep the design to this Task's one worktree and one PR. Do not select backlog
  work, start another Task Session, or create a second worktree.
- Write or tighten the single Task design note only when the change is not yet
  computable. Preserve a clear existing design.
- Resolve reversible ambiguity with the simpler path. Request a durable
  supervisor decision when the choice changes scope, behavior, or authority.
- Do not implement beyond a trivial probe that makes the design computable.

Leave the pursue phase a concrete build and verification target. The Task
runner advances the flow; write no loop bit.
