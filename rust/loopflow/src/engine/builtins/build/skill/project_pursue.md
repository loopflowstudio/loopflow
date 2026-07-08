---
description: Decompose open KRs into running tasks.
default_agent: codex
action_style: procedural
---
Pursue the open KRs.

## Orientation

Read the KR set in `scratch/<branch>.md` and the wave's GOAL/MEMORY. The
project flowloop owns the KR set, not a product PR. There is no backlog:
**the open `lf` runs with a task flow are the open tasks** — check them with
`lf runs`.

## Work

- For each open KR with no running task, dispatch one:
  `lf task "<one-PR-sized statement>" --wave <wave>`. The seed is the task's
  whole handoff — make it computable on its own.
- Use direct execs only for hot, now problems where dispatch would be slower
  than the fix.
- Discovered debt becomes either a dispatched task or a new KR in the doc —
  never a silent scratch note.

Do not check off a KR unless its observable condition is already true.
