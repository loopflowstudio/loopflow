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
Linear; running hands live in `lf runs`; merged PRs are closure evidence.
Resolve the exact wave from the prompt or GOAL path; never guess it. If the PM
reader fails, report that once and continue from the KR set instead of repairing
PM or auth.

The project may read and file its own tasks:

```bash
lf pm show --wave <exact-wave> --project <project>
lf pm task create --project <project> --title "..." --notes "..."
```

## Work

- Read the filed backlog before creating work. File a concrete task when the
  KR needs it; no rule requires every filed task to start immediately.
- Work the next concrete step inline by default. Resolve the sole task blocking
  a KR in this process when it fits the current branch and pass.
- Create a task loop only when the task is a strict subset with its own PR or
  multi-pass/recheck lifecycle, or when independent parallel work materially
  helps. Use `lf --wave <exact-wave> loop task "<one-PR-sized statement>"`;
  add `--detach` only when that exact wave already has a live server and the
  project has another useful move while the task runs. If the result gates the
  project, keep the loop foreground.
- Never start a project or wave from a project loop, and never delegate the
  remaining project as one task. A stopped server is a reason to work inline,
  not to boot orchestration infrastructure.
- A detached task must report, publish live learnings, and leave a PR; otherwise
  its private transcript makes it invisible.
- Discovered debt becomes a task under an existing KR unless it
  reveals a broader standing quality frontier. Do not turn individual cleanup
  into a project-shaped KR.

Do not check off a KR unless its observable condition is already true.
