---
description: Judge and advance one Task's PR after implementation.
default_agent: codex
action_style: procedural
---
Judge the Task change against its directive and design.

Inspect the worktree, focused verification, diff, PR, CI, and review state.

- If the change is incomplete or verification fails, state the next concrete
  repair; the Task runner may start another full flow iteration.
- If the diff is reviewable and has no PR, use the repository's Loopflow PR
  operation to publish or refresh that one Task PR to `main`.
- If CI or review has an obvious fix, make the bounded repair in this Task
  worktree, verify it, and update the same PR.
- Treat an open or approved PR as submitted, never complete. Completion is an
  observed merge or explicit abandonment.
- Close the Linear Task through `lf pm task done` only after merge, preserving
  a pending writeback if Linear is unavailable.

Return concise evidence and the next external wait or repair. The Task runner
independently observes PR/worktree state to choose repeat, wait, block, or
complete; write no loop bit.
