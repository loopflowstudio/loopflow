---
description: Decompose open KRs into running tasks.
default_agent: codex
action_style: procedural
---
Pursue the open KRs.

## Orientation

Read the KR set in `scratch/<branch>.md` and the wave's GOAL/MEMORY. The
project flowloop owns the KR set, not a product PR. KRs are proof-shaped end
states; tasks are the concrete work that advances them. There is no backlog in
the project doc: **the open `lf` runs with a task flow are the open tasks** —
check them with `lf runs`.

## Work

- For each open KR with no running task, dispatch one:
  `lf task "<one-PR-sized statement>" --wave <wave>`. The seed is the task's
  whole handoff — make it computable on its own.
- Use direct execs only for hot, now problems where dispatch would be slower
  than the fix.
- Discovered debt becomes a dispatched task under an existing KR unless it
  reveals a broader standing quality frontier. Do not turn individual cleanup
  into a project-shaped KR.

Do not check off a KR unless its observable condition is already true.
