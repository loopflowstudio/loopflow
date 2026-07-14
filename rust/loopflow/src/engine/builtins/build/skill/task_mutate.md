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
  operation to publish or refresh the active delivery PR to `main`.
- If CI or review has an obvious fix, make the bounded repair in this Task
  worktree, verify it, and update the same PR.
- Treat an open or approved PR as submitted, never complete. A merge settles
  that delivery; it does not inherently complete the Task.
- Use bare `lf pr land --next <slug>` when another delivery follows. Use
  `lf pr land -c` only when this merge proves the whole Task complete. Use
  `lf pr abandon` to discard the active delivery without abandoning the Task.
- Use `lf task complete <issue> --summary "..."` only for a clean Task that
  honestly needs no PR. Loopflow owns Linear completion and pending writeback.

Return concise evidence and the next external wait or repair. The Task runner
independently observes PR/worktree state to choose repeat, wait, block, or
complete; write no loop bit.
