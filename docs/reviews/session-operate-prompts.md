# Session and operate prompts: scenario walk-through (LOO-383)

Dated 2026-10-05. These are **simulations**: each row reads the assembled
`wave/session` prompt (session text plus the composed `wave/operate` procedure)
and states what its instructions require. No model was run against them. They
show that the prompts agree with each other, not that an agent follows them.
The runtime evidence is the post-install demo in "Not yet shown" below.

Jack Heart's decisions on 2026-10-05 set the contract: started Tasks keep
moving, unstarted backlog is not started, a defined Flow proceeds, and a Task
whose Flow ended before landing waits on Jack.

| Scenario | What the operator does | Boundary kept | Governing text |
| --- | --- | --- | --- |
| Idle runnable work: started Task, Flow steps left, no live driver | `lf --task <issue> flow start`, then rereads `lf task status`; reports **acted** | No second driver; no Flow chosen for the Task | "A defined Flow proceeds" |
| Live worker | Reports **moving** with the observed worker; leaves it alone | No interrupt, restart or duplicate launch | Disposition table |
| Recoverable failure | Reads status, the failed step's log and `lf top`/`lf ps --json`; repairs through supported controls; retries with `--reason` | An unchanged failure is reported with its evidence, not retried again | "Recover before reporting a blocker" |
| Unresolved liveness | Reports **unknown**, naming the missing read | Unknown is not idle; nothing is started | Disposition table |
| Pending user review | Reports **waiting on a person** with the Session to open | Completes that Session only when the person decides it with the operator; never approves for them | "Reviews belong to their Sessions" |
| Published PR, delivery unfinished | Reports publication, review, landing, completion and remaining scope separately; if the Flow finished unlanded, **waiting on a person** naming the PR | No landing chosen for the person; no other Flow selected | "A Flow ends where it is authored to end" |
| No useful action | Says so briefly | Valid only when every started Task already holds a disposition | "No action is a valid result only when…" |
| Returning user | Runs the pass again from fresh reads before reporting | Nothing from an earlier turn is reused as current state | Session "Operate, continuously" |
| Question or capture mid-operation | Answers or captures first, then resumes and finishes the pass in the same turn | Captured Tasks are filed, not started | Session "Answer the user first"; `capture-tasks` |

The same rows hold for `repo/session` (which operates each Wave with started
work instead of sending the user to that Wave's conversation) and for
`task/session` at the scope of one Task.

## What changed for the observed failure

`lf wave status` gives `next_move.owner` as `wave` for work the Wave operator
must act on. The old report contract copied that owner into the reply, and the
old opening allowed "one or two useful moves" with no requirement to cover the
rest. The Wave conversation therefore reported its own queue as someone else's.
`wave/operate` now says that owner is the reader, and "ready", "needs
reconciliation" and "the Wave owns this" are not dispositions.

## Continuation limits

A conversation acts only during a turn; nothing in Loopflow schedules its next
one. Between turns a Task's own worker keeps running. The minute check
(`lf task automation`) and the CI watcher (`lf ci watch --status`) help only
where installed. On Jack's Home on 2026-10-05 the minute check was disabled and
Product's declared `wave/operate` cron was not installed; this change installs
neither.

## Rollout

The prompts ship inside the `lf` binary. Nothing changes until a release is
installed; install refreshes the exported skills. A conversation opened before
the install keeps its old text until `lf session replace <id>`.

## Not yet shown

- An installed Wave conversation ending its first turn with every started
  Product Task in a named disposition and no "another role owns it" line.
- The same conversation answering a status question and filing an idea, then
  resuming operation.
- `task/session` used on a real Task. A primary Task conversation
  (`session ensure` for a Task) is LOO-364 work and is not part of this change.
