---
description: Move the current Loopflow work forward from a review, an existing Task, or an approved design in an ordinary coding session.
action_style: procedural
---
Advance the work the User is discussing. Resolve the context, carry out the next
authorized action, and report what actually started or what is waiting.

Read the current conversation and scratch design, then run `lf session list --json`. Use explicit Task identity or the current checkout's tracked Task;
never select a different Task merely because it is the only waiting session.

## A review is ready

When the User asks to finish a review, save the agreed design and feedback first.
Mark the exact Session ready with its feedback and remaining work, then run:

```sh
lf session complete <session-id>
```

Completion returns feedback to the next Flow step. A following loop-decide
interprets it and chooses Advance or Iterate through an authored edge. If the
User requests design revisions, clarify and save them before completing review.
Readiness alone leaves the conversation waiting. For a blocked Ask, Complete
returns the answer to its waiting caller. An unbound interactive conversation
does not become a Task by being closed.

## An existing Task needs to continue

Read `lf task status <issue> --json`, then use `lf task advance <issue>`.
It continues the saved Flow or reports the current driver. An interactive review
waits for its Session's completion. A blocker needs its stated recovery,
and completed work is not restarted. If there is no active Flow, select one using
`lf task run <issue> --flow <flow>` only when the current request identifies the
next work—for example, implementation after an accepted design. A finished Flow
alone is not a request to run it again.

## An approved design has no Task

Use the owning repository, not whichever repository hosted the conversation.
Inspect `lf ls --json` and the relevant Wave status to place the work. Reuse an
existing matching Task when there is one. Otherwise create and prepare a Task
through `lf pm task create` and `lf task prepare`, carrying the complete approved
scope, constraints, and proof into its directive.

Choose the next Flow from the actual catalog. An approved design can proceed
directly to the `pursue` Flow (implement → compress → review-slice → concept-review
→ loop-decide, repeated on Iterate, then human demo and another loop-decide).
Both decisions have explicit edges to implementation. Do not repeat initial
design work merely to launch implementation. For work that still
needs design, use the complete feature Flow. Preserve any explicit User choice
to perform implement → compress → review-slice → concept-review directly in this conversation.
Ask only when intent or placement cannot be resolved from available evidence.

## Verify the handoff

Check installed CLI help before mutation when command availability is uncertain.
If Advance is unavailable, report that limitation and use existing Task/Session
commands only when they express the same authorized action. Do not claim an old
finite Flow will repeat automatically. Never upgrade the installation as a hidden
prerequisite.

After completing a Session, refresh `lf session list --json` and the Task status.
Report the Task link, actual running step or wait, and the next human boundary.
Respect the selected delivery policy; advancing design is not permission to merge.
