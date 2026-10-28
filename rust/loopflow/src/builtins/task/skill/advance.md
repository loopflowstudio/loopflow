---
description: Move the current Loopflow work forward from a review, an existing Task, or an approved design in an ordinary coding session.
action_style: procedural
---
Advance the work the User is discussing. Resolve the context, carry out the next
action, and report what actually started or what is waiting.

Read the current conversation and existing design. Continue the explicitly
selected Task or the current checkout's existing Task when one is supplied.
Do not inventory Sessions or search other Tasks to find something to operate.

## A review is ready

When the User asks to finish a review, save the agreed design, feedback and
remaining work. Continue the discussion in the same conversation. Feedback is
not a command to close a Session or launch a Graph. Inspect work and effect
history before launching explicitly selected next work; ambiguous feedback
starts nothing.

## An existing Task needs to continue

Read `lf task status <issue> --json`. Leave a Graph with a live process running.
A stopped or failed Graph is history: inspect what it finished and the effects it
recorded; historical review boundaries remain evidence. Launch fresh work with
`lf task run <issue>` only when the current request identifies the
next work—for example, implementation after an accepted design. A Task waiting
at an interactive step continues after it; `--from <step>` starts elsewhere.
A blocker needs
its stated recovery first, completed work is not repeated, and a finished Graph
alone is not a request to run it again.

## An approved design has no Task

Continue in the owning checkout. Implement directly or run the selected Graph,
such as `lf graph pursue`, using the accepted design. No Task, Wave, Linear setup,
or Task graph is required. Preserve an explicit choice to perform
implement → compress → refresh in this conversation.

When the user explicitly requests managed work, load `work` and use
`launch-plan` to place and prepare it. An approved design or request to continue
coding alone does not request Task creation.

## Verify the handoff

Check installed CLI help before mutation when command availability is uncertain.
If Advance is unavailable, report that limitation and use existing Task/Session
commands only when they express the same action. Do not claim an old
finite Graph will repeat automatically. Never upgrade the installation as a hidden
prerequisite.

After launching, inspect the selected execution and, when associated, its Task
status. Report the actual running step or wait and what next needs the user.
Follow the selected delivery policy through its review and merge steps.
