---
description: Move the current Loopflow work forward from a review, an existing Task, or an approved design in an ordinary coding session.
action_style: procedural
---
Advance the work the User is discussing. Resolve the context, carry out the next
action, and report what actually started or what is waiting.

Read the current conversation and scratch design, then run `lf session list --json`. Use explicit Task identity or the current checkout's tracked Task;
never select a different Task merely because it is the only waiting session.

## A review is ready

When the User asks to finish a review, save the agreed design, feedback and
remaining work. Continue the discussion in the same conversation. Feedback is
not a command to close a Session or launch a Flow. Inspect work and effect
history before launching explicitly selected next work; ambiguous feedback
starts nothing.

## An existing Task needs to continue

Read `lf task status <issue> --json`. Leave a Flow with a live driver running.
A stopped or failed Flow is history: inspect what it finished and the effects it
recorded; historical review boundaries remain evidence. Launch fresh work with
`lf task run <issue> <flow>` only when the current request identifies the
next work—for example, implementation after an accepted design. A blocker needs
its stated recovery first, completed work is not repeated, and a finished Flow
alone is not a request to run it again.

## An approved design has no Task

Use the owning repository, not whichever repository hosted the conversation.
Inspect `lf wave list --json` and the relevant Wave status to place the work. Reuse an
existing matching Task when there is one. Otherwise create and prepare a Task
through `lf task create` and `lf checkout`, carrying the complete approved
scope, constraints, and proof into its directive.

Choose the next Flow from the actual catalog. An approved design can proceed
directly to the `pursue` Flow (implement → compress → refresh
→ loop-or-next, repeated on Iterate, then pr-publish): `lf task run <issue> code`
takes up the `code` workflow, whose first edge is that Flow. Its PR review
happens in the Task conversation, not as a Flow step. Do not repeat initial
design work merely to launch implementation. For work that still
needs design, take up the `feature` workflow, whose first edge drafts the
design. A Task on a workflow takes only the edges leaving its current stage;
`lf task run <issue>` names them. Preserve any explicit User choice
to perform implement → compress → refresh directly in this conversation.
Ask only when intent or placement cannot be resolved from available evidence.

## Verify the handoff

Check installed CLI help before mutation when command availability is uncertain.
If Advance is unavailable, report that limitation and use existing Task/Session
commands only when they express the same action. Do not claim an old
finite Flow will repeat automatically. Never upgrade the installation as a hidden
prerequisite.

After launching, refresh `lf session list --json` and the Task status.
Report the Task link, actual running step or wait, and what next needs the User.
Follow the selected delivery policy through its review and merge steps.
