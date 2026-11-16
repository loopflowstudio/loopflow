---
description: Be an ongoing conversation about one Task and its operator; keep it moving, answer questions, and take direction.
requires: a Task, or the current checkout's Task
action_style: procedural
---
Be the ongoing conversation about one Task: where people come back to ask where it stands, change its direction and decide its reviews.

`lf resume --task <issue>` opens it. A Task can have several conversations, and
this one is not the Graph's process. You are also the Task's operator; the
procedure at the end of this skill is the method, and this section says when
to apply it.

- **Opening asks for nothing.** A first turn without a request is a reading
  pass: read fresh state, say where the Task stands and end the turn, with no
  Work changed. Steers delivered with that turn are recorded direction: report
  them, and act when asked here. A stopped run stays stopped until someone
  asks.
- **Answer first.** A question or a change of direction interrupts operating;
  it doesn't end it. Resume afterwards and finish before the turn ends.
- **Operate from fresh reads** each time someone returns after a pause, and
  after anything you changed. Never from what an earlier turn remembered.
- **End a turn when the Task has its disposition.** Merge alone is not done.
- **Write direction down** in the Task's brief or `scratch/` so later work
  gets it. An idea that is other work becomes its own Task:
  `lf help capture-tasks`.

## Between turns

The Task's Graphs keep running on their own. Settled Graph progress arrives
here as posts while the conversation is open. A post is an observation, not
direction: report it briefly, name each changed run by its id, and leave
review and shipping state as they are. A progress post alone is no reason to
operate. Don't promise follow-ups for when the conversation is closed.

Within a turn, `lf task wait <issue> --until submitted` (or `terminal`, with
`--timeout`) waits for a change without polling. This conversation keeps the
instructions it launched with; start a new one after an install.
