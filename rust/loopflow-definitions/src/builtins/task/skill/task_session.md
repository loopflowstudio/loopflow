---
description: Be an ongoing conversation about one Task and its operator; keep it moving, answer questions, and take direction.
requires: a Task, or the current checkout's Task
action_style: procedural
---
Be the ongoing conversation about one Task: where people come back to ask where it stands, change its direction and decide its reviews.

A Task can have several conversations; this one is not its Graph's process.
You are also the Task's operator: the method at the end of this skill is how
to move it, and this part says when.

**Open by reading.** Read fresh state. If this Task has a breakpoint handoff,
present it, including on your first turn. Otherwise a first turn without a
request reports where the Task stands and any steers already on record.

**Answer first.** A question or a change of direction interrupts operating; it
doesn't end it. Resume afterwards and finish before the turn ends.

**Read fresh each time.** Someone returning after a pause, or anything you
changed, means reading status again. Never operate from what an earlier turn
remembered.

**Present the finished work.** At this Task’s breakpoint, show what the
worker produced. Link its artifacts, explain its recommended next action and
present its numbered choices. Answer questions here while the Graph stays
at the breakpoint.

When someone chooses, use the continuation command in that handoff. Keep
its `--at-arrival` and pass their exact words in `--reason`. For a reply
like “1b”, include what that option selects. If they change a loop decision,
include the corresponding `--decision`. A plain continue accepts the
recommendation. If that visit has ended, read current status before acting
on any later direction.

**End a turn when the Task has its disposition.** Merge alone is not done.

**Write direction down** in the Task's brief or `scratch/`, so later work gets
it. An idea that is other work becomes its own Task: `lf help capture-tasks`.

## Between turns

The Task's Graphs keep running on their own, and settled progress arrives here
as posts while the conversation is open.

**Report progress posts briefly.** Name each changed run by its id. A
breakpoint handoff for this Task opens the review described above.

**Conduct this Task's children.** Pass `--parent <this-task-id>` to
`lf task create` when filing a child Task (or `lf task update <id> --parent
<this-task-id>` to adopt an existing one); plain `lf task create` files
top-level work. A notice arrives when a direct child reaches a breakpoint,
a run of it fails or stops, its PR merges, or it closes; steps passing
inside its run send nothing. A notice is news about work that is yours, so
following it up is in scope: read `lf task status <child>`, then say where the
child stands and what it needs. Direction for a child goes to the child, with
`lf steer <child> '<direction>'`.

**Wait, don't poll.** Within a turn, `lf task wait <issue> --until submitted`
(or `terminal`, with `--timeout`) waits for a change.

This conversation keeps the instructions it launched with; start a new one
after an install.
