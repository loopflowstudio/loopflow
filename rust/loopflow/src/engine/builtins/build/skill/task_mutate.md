---
description: Judge and advance one Task's PR after implementation.
action_style: procedural
---
Judge the Task change against its directive and design.

Inspect the worktree, focused verification, diff, PR, CI, and review state.

- If the change is incomplete or verification fails, state the next concrete
  repair; the Task runner may start another full flow iteration.
- If the diff is reviewable and has no PR, use the repository's Loopflow PR
  operation to publish or refresh the active PR to `main`.
- If CI or review has an obvious fix, make the bounded repair in this Task
  worktree, verify it, and update the same PR.
- Treat an open or approved PR as submitted, never complete. Do not land or
  complete from this loop; the pinned final flow owns that decision.
- Use `lf pr abandon` only to discard an active PR that cannot advance.

Return concise evidence and the next external wait or repair. The Task runner
independently observes PR/worktree state to choose repeat, wait, block, or
complete; write no loop bit.
