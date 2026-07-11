---
description: Advance open KRs inline first, filing or looping tasks when needed.
default_agent: codex
action_style: procedural
---
Pursue the open KRs.

## Orientation

Read the KR set in `scratch/<branch>.md` and the wave's GOAL/MEMORY. The
project loop owns the KR set, not a product PR. KRs are proof-shaped end
states; tasks are the concrete work that advances them. Filed tasks live in
Linear; running work lives in Task Sessions; merged PRs are closure evidence.
Resolve the exact wave from the prompt or GOAL path; never guess it. If the PM
reader fails, report that once and continue from the KR set instead of repairing
PM or auth.

The project may read and file its own tasks:

```bash
lf pm show --wave <exact-wave> --project <project> --no-sync
lf pm task create --project <project> --title "..." --notes "..."
```

## Work

- Read the filed backlog before creating work. File a concrete task when the
  KR needs it; no rule requires every filed task to start immediately.
- Every file-writing task must already have a Linear identity. Start it with
  `lf task run <issue-id>` and supervise the same Task Session through review
  and merge.
- Use `lf task follow-up`, `steer`, `interrupt`, `wait`, and `resume`. Do not
  create another worktree or session for review feedback or CI repair.
- Never start another Project or Wave from Project pursuit, and never collapse
  the remaining Project into one anonymous task.
- Discovered debt becomes a task under an existing KR unless it
  reveals a broader standing quality frontier. Do not turn individual cleanup
  into a project-shaped KR.

Do not check off a KR unless its observable condition is already true.
