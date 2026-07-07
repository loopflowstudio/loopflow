---
description: Make the project's KR set measurable in its own doc.
default_agent: codex
action_style: procedural
---
Clarify the project artifact: its KR set.

## Orientation

The project seed is in `<lf:message>`. The KR set lives in the project's own
doc — `scratch/<branch>.md`, a `## KRs` section of 2–10 checkboxed items —
not in an external tracker. Read the seed, the doc if present, the wave's
GOAL.md and MEMORY.md.

## Work

- If the doc's KR set is measurable (each KR states an observable condition
  you could check with a command or a look), leave it alone.
- Otherwise write it: 2–10 KRs, each one line, each checkable. Milestone KRs
  retire when true; self-renewing KRs say what respawns them.
- If the seed can't support real KRs, record that as a blocker in
  `scratch/questions.md` and stop.

Do not decompose work in this phase unless the clarification is trivial and
directly unblocks the next phase.
