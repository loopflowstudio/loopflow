---
description: Decompose project KRs into concrete work.
default_agent: codex
action_style: procedural
---
Pursue the open KRs.

## Orientation

Read the KR set in `<lf:message>` and the wave's GOAL/MEMORY. The project
flowloop owns the KR set, not a product PR.

## Work

- For each open KR, file or refine the smallest Linear task that would move it.
- Dispatch executable tasks with `lf task <linear-item-id> --wave <wave>` when
  the task is clear enough to run.
- Use direct execs only for hot, now problems where dispatch would be slower
  than the fix.
- File discovered debt as Linear tasks instead of hiding it in scratch notes.

Do not mark a KR complete unless the described condition is already true.
