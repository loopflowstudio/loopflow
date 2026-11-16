---
description: Move the current Loopflow work forward from a review, an existing Task, or an approved design in an ordinary coding session.
action_style: procedural
---
Move the work under discussion to its next action, then report what started or
what is waiting.

Read the conversation and the existing design. Continue the selected Task, or
the current checkout's Task. Don't search Sessions or other Tasks for something
to run.

## A review is ready

To finish a review, save the agreed design, feedback and remaining work, and
keep talking in the same conversation. Feedback is not a command to close a
Session or launch a Graph. Check what already ran before launching the next
work that was asked for; ambiguous feedback starts nothing.

## An existing Task continues

Read `lf task status <issue> --json`. Leave a running Graph alone. A stopped or
failed Graph is history: check what it finished and what it changed. Run
`lf task run <issue>` only when the request names the next work, such as
implementation after an accepted design. A Task waiting at an interactive step
continues after it; `--from <step>` starts elsewhere. Clear a blocker the way it
says first, don't repeat finished work, and a finished Graph alone is no reason
to run it again.

## An approved design has no Task

Continue in its checkout. Implement directly or run the chosen Graph, such as
`lf graph pursue`, from the accepted design; no Task, Wave or Linear setup is
needed. If implement → compress → refresh in this conversation was chosen,
do that.

Only when managed work is asked for, load `work` and use `launch-plan` to place
and prepare it. An approved design or "keep coding" doesn't ask for a Task.

## Check the handoff

When unsure a command exists, check installed CLI help before changing
anything. If Advance isn't available, say so and use Task or Session commands
only when they do the same thing. Don't claim a finished Graph will repeat on
its own, and never upgrade the installation along the way.

After launching, check the run and, if there is one, `lf task status`. Report
the step actually running or waiting, and what needs a person next. Follow the
chosen delivery path through its review and merge steps.
