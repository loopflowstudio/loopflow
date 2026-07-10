---
description: Promote a project into a resident child wave.
default_agent: codex
action_style: procedural
---
Promote the named project. The seed names the project slug and its parent wave.

## Contract

A project becomes a wave only when it needs its own live thread and cadence.
Memory inherits from the parent; chat does not. Promotion moves the pen: after
this flow, the parent receives authored reports rather than overhearing the
child's work.

## Work

1. Read `wave/<parent>/projects/<slug>.md`, the parent's `GOAL.md`, and its
   current memory. Refuse a missing source or an existing `wave/<slug>/GOAL.md`.
2. Move the project document to `wave/<slug>/GOAL.md`. Preserve its definition
   and KRs, then add valid GOAL frontmatter with a cadence suited to the bet.
   Use a weekly pursue cadence when the document gives no sharper signal.
3. Create an empty `wave/<slug>/MEMORY.md`. Never copy parent memory: the
   runtime inherits it through `parent_wave_id`.
4. Initialize the child PM project with `lf pm init --wave <slug>`. Read the
   parent's `project:<slug>` tasks with
   `lf pm show --wave <parent> --project <slug> --json` and migrate every task
   into the child's Linear project. If the current PM provider cannot remove
   the old project label safely, move the tasks and record that exact residual
   drift in `scratch/questions.md`.
5. Delete the source project file only after the destination is complete.
6. Add a child `Process` instruction requiring its first pass to report the
   newly owned definition and KRs in its own thread, then publish that concise
   report to the parent with `lf radio --parent`.

The command's mechanical postflight links the registry ancestry, launches the
child residency, waits for `.wave-endpoint`, and sends the bootstrap message
that starts this first pass. Do not launch a second wave process or impersonate
the child from the promotion flow. The child's resulting report is the
checkable proof that the new mind can speak across the boundary on purpose.
