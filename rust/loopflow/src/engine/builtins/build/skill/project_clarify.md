---
description: Make the project's KR set measurable in its own doc.
default_agent: codex
action_style: procedural
---
Clarify the project artifact: its KR set.

## Orientation

The project seed is in `<lf:message>`. A project is a measured bet inside one
wave; it owns KRs and closure criteria, not memory, cadence, or child projects.
Read the seed, the project doc if present, and the wave's GOAL.md and MEMORY.md.

Wave project docs live at `wave/<wave>/projects/<project>.md`. During an
isolated project loop, the working doc may be `scratch/<branch>.md`. In
either case, the project doc carries the definition and a `## KRs` section.
Concrete tasks live outside the KR set.

## Work

- If the doc's KR set is measurable (each KR states an observable condition
  you could check with a command or a look), leave it alone.
- Otherwise write it: 2–10 KRs, each one line, each checkable. Shape each
  KR as proof under duration — counted streaks on real work, unattended
  windows ("over one week... zero rescues"), never capability checkboxes
  that pass once on a demo. KRs should read
  as proof that the bet holds, not backlog bullets, implementation receipts,
  issue ids, or status notes. Milestone KRs retire when true; self-renewing KRs
  say what respawns them.
- If the seed is a task bundle or individual technical-debt cleanup, do not
  promote it into a project. Name the broader behavioral improvement or standing
  quality frontier, or record that it belongs under an existing project.
- If the seed can't support real KRs, record that as a blocker in
  `scratch/questions.md` and stop.

Do not decompose work in this phase unless the clarification is trivial and
directly unblocks the next phase.
