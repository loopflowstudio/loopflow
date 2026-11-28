---
description: Operate one Wave; keep your started Tasks moving, leave other people's work alone, and judge the existing work.
action_style: procedural
---
Operate one Wave once: read, judge, act, check, and exit.

A started Task you own with no live Graph has nobody else to take its next
step, so that step is yours. The pass is done when every started Task you own
has a disposition and every other Task is listed. Work the Wave's existing
Tasks; its plan, new Tasks and backlog are not this skill's to change.
Another conversation or scheduled pass may be operating this Wave too: work it
is driving is moving, so leave it alone.

## Read the Wave

Operate the Wave the invocation names or the conversation is bound to; if it is
missing or ambiguous, say so and stop. Read its goal and memory, then
`lf wave status <wave> --json` and `lf wave show <wave> --json`, then
`lf task status <issue> --json` for the Tasks that need a closer look. Follow a
summary to its source only where a claim, conflict or missing fact matters.

A failed read, a stale plan or unknown liveness never means an empty backlog,
a healthy Wave or a finished outcome. Keep going on what you can read; don't
stop to fix auth.

## Whose Task is it

Ownership is `runtime.owner` in `lf wave show --json`, compared with
`lf config user`:

- **Yours:** the person is you, or the owner names no person. Each unfinished
  one gets exactly one disposition. If its Machine is not this one
  (`lf machine self`), operate it there with
  `lf --machine <machine> task-operate <issue>` or report it unknown; never
  run, repair or complete it from here.
- **Theirs:** list it with its person and Machine as "theirs · left alone".
  Don't run, repair, complete, move, interrupt or comment on it, and don't
  judge its liveness; their processes aren't visible here.
- **Unowned:** backlog. List it; starting it is a person's selection.

An unreadable owner makes the Task unknown: report it and leave it.

| Disposition | When, and what to do |
| --- | --- |
| paused | Report the held step and leave the Task quiet, including while a command finishes. |
| moving | A live Graph run. Leave it alone. An open conversation is not a Graph run. |
| acted | This pass continued, repaired or delivered it with an `lf` command, then reread status and saw the change. |
| available | A review, demo, choice or merge click is ready, with its conversation link or PR. Never complete or approve it for them. |
| dependency or capacity | Waiting on a named Task, PR, running check, account or limit. |
| unassigned | Stopped by instruction, or started and owned by nobody. |
| unknown | Name the missing read or liveness check. Unknown is not idle, and never a reason to start a second Graph. |

"Ready" and "needs reconciliation" are not dispositions. Take the first row
that fits:

- **Check for a live Graph on each remaining Task.** In `lf task status <issue> --json`, a
  position with `running` true is a live run, read from its own pid. A live
  run makes the Task moving, whoever launched it.
- **Repair before reporting a blocker.** Read the failed step's log and what
  it changed, fix the cause if you can, and relaunch what remains with
  `lf task run <issue-id> --reason "<what changed>"`. A stopped Graph is
  history and can't be resumed. Report an unchanged failure; don't repeat it.
- **A Graph ends where it was written to.** A finished Graph or a published PR
  doesn't complete the Task. A Task at a breakpoint is available for review in
  its conversation: never continue past the review, pick a new direction or arm
  a merge they didn't approve.
- **Finish merged work.** Let a live Graph finish. Otherwise, when the
  outcome holds, run `lf task complete <issue>` and reread status. Retry
  completion alone, never by re-arming a merged PR.

Tasks progress independently of this pass; their Graphs do the work, so don't
perform a Graph's steps here. `lf task run <issue-id>` continues a Task you
own to its next breakpoint, and `lf task status` follows it. `lf task
auto on` follows the workers’ recommendations through breakpoints and may
merge the PR; `lf -b task run` does this for one run, and a run with no
breakpoint left ahead may merge it too. Use them only when asked to ship or land. Start
remaining work on your Tasks without asking. Don't approve reviews, cancel
Tasks or change direction.

Unstarted Tasks that are due or overdue are listed apart from other backlog,
for selection. A due date is a day, not an alarm: time passing never runs or
completes a Task.

## Judge the Wave's work

Ask these together and dig into the tension that matters. They are not five
required reports.

| Function | Question |
| --- | --- |
| S1 · Delivery | What useful outcome did a Task deliver, for the people it was for? |
| S2 · Coordination | Where do Tasks collide or depend on each other? Name the interface or ordering. |
| S3 · Present capacity | Is effort advancing outcomes? Tell active work, review waits, repeated failures and idle capacity apart. |
| S4 · Adaptation | What changed in user needs, code or the outside world that could undo a Task's direction? |
| S5 · Identity | Does what a Task found challenge the Wave's objective or scope? Name the finding and the decision needed. |

Follow the signal to its source: path overlap and repeated conflicts for
coordination; retries, CI waits and real outcomes for capacity; dated
dependency or API changes for adaptation. Quiet chat, a finished agent turn or
a green PR doesn't show an outcome. Don't change policy just to have something
to do.

## Finish

Reply with each started Task once, under these headings, skipping empty ones:

1. **Available** — each review, demo, choice or merge to take up, with its link.
2. **Moving** — live work left alone, and what this pass did with its result.
3. **Stuck** — dependency or capacity, unassigned or unknown, with the cause.
4. **Others' work** — each Task another person owns, with person and Machine.

Then due follow-ups, other backlog, and any judgment, proposals and gaps. If
nothing needed action, say why in a line.

Render each row as
`[identifier · Task title](Work URL) — status; next action/owner · [PR #n](PR URL)`.
`lf open --task <issue> --link --format markdown` prints the label and Work
URL; `reference.issue_url` is the provider issue when that is the point. Take
the identifier from `task.identifier`, the title from `task.name` and the next
owner from `next_move.owner`; give the disposition as status, not
`runtime.status` alone. A Task can have a checkout without a PR. Never guess a
URL, branch or `reference.workspace.slug` from a title; without a link, keep
the title and status.
