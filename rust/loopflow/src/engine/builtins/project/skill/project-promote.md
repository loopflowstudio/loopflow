---
description: Promote a project into a resident child wave.
action_style: procedural
---
Promote the named project. The seed names the project slug and its parent wave.

## Contract

A project becomes a wave only when it needs its own live thread and cadence.
Memory inherits from the parent; chat does not. Promotion moves the pen: after
this flow, the parent receives authored reports rather than overhearing the
child's work.

## Work

1. Read the source with `lf pm show --wave <parent> --project <slug> --json --no-sync`,
   plus the parent's `GOAL.md` and current memory. Refuse a missing Project or
   an existing `wave/<slug>/GOAL.md`.
2. Create `wave/<slug>/GOAL.md`. Preserve the Project definition as the child
   objective, then add valid GOAL frontmatter with a cadence suited to the bet.
   Use a weekly pursue cadence when the Project gives no sharper signal.
3. Create an empty `wave/<slug>/MEMORY.md`. Never copy parent memory: the
   runtime inherits it through `parent_wave_id`.
4. Initialize the child's Linear Initiative with `lf pm init --wave <slug>`;
   it reuses the repository Team. Recreate the measured bet there with
   `lf pm project create`, and move every source task with
   `lf pm task move --id <id> --wave <slug> --project <slug>`. Loopflow already
   prepared durable ancestry before this flow, so the first Project title uses
   the full Wave path. Only after the child snapshot is complete, remove the
   duplicate parent bet with
   `lf pm project archive --wave <parent> --project <slug>`.
5. Add a child `Process` instruction requiring its first pass to report the
   newly owned definition and KRs in its own thread. Parent/child continuity
   is already represented by the typed Wave and Project relationship.

The command's mechanical postflight links the registry ancestry, launches the
child residency, waits for `.wave-endpoint`, and sends the bootstrap message
that starts this first pass. Do not launch a second wave process or impersonate
the child from the promotion flow. The child's resulting report is the
checkable proof that the new Wave can start from the promoted definition.
