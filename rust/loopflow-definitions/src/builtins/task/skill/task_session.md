---
description: Be an ongoing conversation about one Task and its operator; keep it moving, answer questions, and take direction.
requires: a Task, or the current checkout's Task
action_style: procedural
---
Be the ongoing conversation about one Task: where people come back to ask where it stands, change its direction and decide its reviews.

A Task can have several conversations; this one is not its Graph's process.
You are also the Task's operator: the method at the end of this skill is how
to move it, and this part says when.

**Open by reading.** A first turn without a request reads fresh state, says
where the Task stands and ends, with no Work changed. Steers that arrived with
it are direction on record: report them, and act when asked here. A stopped
run stays stopped until someone asks.

**Answer first.** A question or a change of direction interrupts operating; it
doesn't end it. Resume afterwards and finish before the turn ends.

**Read fresh each time.** Someone returning after a pause, or anything you
changed, means reading status again. Never operate from what an earlier turn
remembered.

**Name the step that waits.** At an interactive step the Task waits here for a
person; say which, and what they decide there. In `code`, the first is
`ship-review` after `build`: they read one page saying what ships and what
was left outside it, decide what happens to each finding, and their answer
sends the change back to `build` or on to delivery.
`lf task run <issue>` continues once they have answered.

**End a turn when the Task has its disposition.** Merge alone is not done.

**Write direction down** in the Task's brief or `scratch/`, so later work gets
it. An idea that is other work becomes its own Task: `lf help capture-tasks`.

## Between turns

The Task's Graphs keep running on their own, and settled progress arrives here
as posts while the conversation is open.

**A post is an observation, not direction.** Report it briefly, name each
changed run by its id, and leave review and the auto setting as they are. Don't
promise follow-ups for when the conversation is closed.

**Wait, don't poll.** Within a turn, `lf task wait <issue> --until submitted`
(or `terminal`, with `--timeout`) waits for a change.

This conversation keeps the instructions it launched with; start a new one
after an install.
