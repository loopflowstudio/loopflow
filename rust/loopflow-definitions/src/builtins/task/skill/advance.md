---
description: Move the current Loopflow work forward from a review, an existing Task, or an approved design in an ordinary coding session.
action_style: procedural
---
Move the work under discussion to its next action, then report what started or what is waiting.

Read the conversation and the existing design. Continue the selected Task, or
this checkout's Task. Don't search Sessions or other Tasks for something to
run.

**A review is ready.** Save the design, the feedback as it was said and the
remaining work, and keep talking here. Feedback is not a command to close a
Session or launch a Graph; ambiguous feedback starts nothing.

**A Task continues.** Read `lf task status <issue> --json`. Leave a running
Graph alone. A stopped Graph is history: check what it finished and changed.
Run `lf task run <issue>` only when the request names the next work; a Task
parked at a breakpoint continues past it, and `--from <step>` starts
elsewhere. A finished Graph alone is no reason to run it again.

**A design has no Task.** Continue in its checkout: implement directly or run
the chosen Graph, such as `lf graph pursue`. No Task, Wave or Linear setup is
needed, and "keep coding" doesn't ask for one. Only when managed work is asked
for, use `launch-plan`.

Check `lf help <command>` before relying on a command you're unsure of, and
never upgrade the installation along the way. After launching, read the run
and `lf task status`, and report the step actually running or waiting and what
needs a person next.
