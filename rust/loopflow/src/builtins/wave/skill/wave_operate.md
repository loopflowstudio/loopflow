---
description: Operate one Wave; keep your started Tasks moving, leave other people's work alone, and judge the existing work.
action_style: procedural
---
Operate one Wave once: read, judge, act, check, and exit. A started Task you own
with no live Graph has nobody else to take its next step, so that step is
yours. The pass is done when every started Task you own has a disposition and
every other Task is listed. Work the Wave's existing Tasks; the plan, key
results, metric targets, new Tasks and backlog are not this
skill's to change. Another conversation or scheduled pass may be operating this
Wave too; work it is driving is moving, so leave it alone.

Edit `.lf/wave/` only when the running Graph says to; otherwise put proposed
Wave changes in `scratch/`.

## Read the Wave

Operate the Wave the invocation names or the conversation is bound to; never
guess from a GOAL file. If the Wave is missing or ambiguous, say so and stop.
Read its goal, memory, and the current Project's Tasks with their outcomes and
open concerns.

Start with `lf wave status <exact-wave> --json` and
`lf wave show <exact-wave> --json`, then `lf task status <issue> --json` for the
Tasks that need a closer look. Follow a summary to its source only where a
claim, conflict or missing fact matters; don't read every transcript.

Keep links, paths and dates with each finding that drives an action, and keep
observations, guesses and proposals apart. Rereading a cache doesn't refresh
it. A failed read, stale plan, missing outcome or unknown liveness never means
an empty backlog, a healthy Wave or a finished outcome. Keep going on what you
can read; don't stop to fix auth or wait indefinitely.

## Keep started work moving

Ownership is `runtime.owner` in `lf wave show --json`, whether or not a Graph
is running. Compare its `person` with `lf config user`:

- **Yours:** the person is you, or the owner names no person. Give every
  unfinished Task you own exactly one disposition: the first row below that
  fits. If its `runtime.owner.machine` is not this Machine (`lf id`), its
  liveness is unknown here: operate it on its Machine with
  `lf --machine <machine> task-operate <issue>` and file the disposition it
  returns, or report it unknown and name the Machine. Never run, repair or
  complete it from this Machine.
- **Theirs:** another person owns it. List it with that person and Machine as
  “theirs · left alone”. Don't run, repair, complete, claim, move, interrupt or
  comment on it, and don't judge its liveness: their Processes aren't visible
  here, so idle-looking means nothing.
- **Unowned:** backlog. List it and leave it; starting it is a selection for
  the person.

An unreadable owner or unset `lf config user` makes ownership unknown: report
the Task unknown and leave it. Tasks under `unavailable_tasks` are unknown
until read.

| Disposition | When, and what to do |
| --- | --- |
| moving | A live Graph process is running. Leave it alone. A connected conversation or review Session is not a Graph process. |
| acted | This pass continued, repaired or delivered it with an `lf` command, then reread status and saw the change. |
| available | A review, demo, concrete choice or merge click is ready, with its conversation link or PR. Never complete or approve it for them. |
| dependency or capacity | Waiting on a named Task, PR, running check, account or limit. |
| unassigned | Stopped by instruction, or a started Task nobody owns (`lf task unassign <issue>`); nothing moves it until someone claims it. |
| unknown | Name the read or liveness check that is missing. Unknown is not idle and never a reason to start a second Graph. |

“Ready”, “needs reconciliation” and “the Wave owns this” are not dispositions.
Judge ownership first. When status gives `next_move.owner` as `wave` on a Task
you own, settling its disposition is your job, not a handoff; another person's
Task stays “theirs · left alone” whatever `next_move` says. Treat its
recommended action as a suggestion to check against the rules below. No action
is a valid result only when every started Task you own already has a
disposition.

- **Look for a live Graph first.** In `lf task status <issue> --json`, check
  every Graph in `execution.work.graphs` and every unfinished Process in
  `execution.work.processes` against `lf ps --json`. A live process makes the
  Task moving, whoever launched it.
- **Repair before reporting a blocker.** Read the failed step's log and look at
  what it changed. Fix the cause if you can, then relaunch the remaining work
  with `lf task run <issue> <graph> --reason "<what changed>"`. A stopped Graph
  is history and can't be resumed. Retry only when something changed; report
  an unchanged failure instead of repeating it.
- **A Graph ends where it was written to.** A finished Graph or published PR
  doesn't complete the Task. Name what remains and the PR or Task conversation
  waiting on a decision. A Task at an interactive step (`work.graph` in status)
  is available to the person there. Start a stopped or failed run again with
  `lf task run <issue>`; never continue past their step, pick a new direction
  or arm an unapproved merge.
- **Retry completion alone.** A recorded completion request at `end` retries
  with `lf task complete <issue>`, never `task run` or gate. Completion, graph
  position and Process liveness are independent. A newer reopening wins; `end`
  alone can't re-complete it.
- **Finish merged work.** Merging doesn't complete a Task. Check its Graphs and
  unfinished Processes and let a live ship Graph finish. Otherwise, when the
  accepted outcome holds, run `lf task complete <issue>` and reread status.
  Don't replay gate or re-arm a merged PR.
- **Reviews stay in the Task conversation.** Point to the Session from
  `lf session list --available --json` and discuss feedback there. Closing a
  Session approves nothing. In a headless run, a judgment call leaves the Task
  available for the person.

Start remaining work on your Tasks without asking. Don't approve reviews, arm a
merge, cancel or abandon Tasks, or change direction.

## Due follow-ups

Read due dates and briefs in the plan. List unstarted Tasks that are due or
overdue apart from other backlog, for selection; they are unowned backlog, so
don't start one. Time passing never runs or completes a Task. A started Task you
own whose date has arrived takes its ordinary disposition; before that date it
is a dependency naming the date. Honor any timezone, release or environment
prerequisite in the brief; a due date is a day, not an alarm. A missing date
stays unknown. This pass installs no schedule, and an open conversation doesn't
schedule its next turn.

## Judge the Wave's existing work

Ask these together over the Wave's Tasks and dig into the tension that matters;
they are not five required reports or five agents. Put findings and proposals
in the reply.

| Function | Question |
| --- | --- |
| S1 · Delivery | What useful outcome did a Task deliver, compared with who it was for? |
| S2 · Coordination | Where do Tasks collide or depend on each other? Name the interface or ordering without serializing unrelated work. |
| S3 · Present capacity | Is effort advancing outcomes? Tell apart active work, review waits, repeated failures and unused capacity. |
| S4 · Adaptation | What changed in user needs, code or the outside world that could undo a Task's direction? |
| S5 · Identity | Does what a Task found challenge the Wave's objective or scope? Name the finding, the premise and the decision needed. |

Follow the signal to its source: PR and path overlap and repeated conflicts for
coordination; retries, CI waits, resource use and real outcomes for capacity;
dated dependency, advisory or API changes for adaptation. Quiet chat, a finished
agent turn or a green PR doesn't show an outcome. Don't change policy just to
have something to do.

## Tasks run themselves

Tasks progress independently of Wave and repository passes; neither pass gates
them. A Task's Graphs do its work, so don't perform a Graph's steps here. Keep
Graph history, review conversations, worktrees and ownership intact.

- Continue a Task you own with `lf task run <issue-id>`; it runs the Task's
  graph to the next interactive step. Each run is a fresh Graph process and
  returns once it is running; `lf task status <issue-id>` follows it. Leave a
  live one running instead of duplicating it. `lf -b task run` and `lf ship`
  approve Auto-Ship, which may merge the PR; use them only when asked to ship
  or land.
- Use `lf` Task commands, not raw stores, signals to processes you don't own,
  or a competing worker on this Machine. A missing Machine is a blocker to
  report.

Tell apart writes that landed, writes that failed and writes you couldn't read
back; settle the uncertain ones before retrying. Start no long-running process.

## Finish

Reply with each started Task once, under these headings in this order, skipping
empty ones:

1. **Available** — each review, demo, choice or merge ready to take up, with its link.
2. **Moving** — live work left alone, and what this pass did with its result.
3. **Stuck** — dependency or capacity, unassigned, or unknown, with the cause
   or the missing read.
4. **Others' work** — each Task another person owns, with that person and
   Machine, “left alone”.

Then list due unstarted follow-ups for selection, other backlog, and any
judgment, proposals, open decisions and gaps. Keep each row to its disposition
and the fact that decides it. If nothing needed action, say why in a line.
Record only what can't be worked out again later — changed decisions with their
status, what a run showed, open concerns — where they already live. Report a post to a channel only after it went through; this skill
doesn't send the reply anywhere.

For lists of Tasks, use the status and show reads above and render each row as
`[identifier · Task title](Work URL) — status; next action/owner · [PR #n](PR URL)`.
`lf open --task <issue> --link --format markdown` prints that label with its
Work URL, which opens the Task's conversation; the provider issue
(`reference.issue_url`) is a separate link when the issue itself is the point.
Where links may not activate, add the `lf open '<url>'` it prints. In prose,
shorten later references when unambiguous.

Take the identifier from `task.identifier` and the title from `task.name`. Give
the disposition as status, not `runtime.status` alone, and the next owner from
`next_move.owner`, read as described under ownership. Link the Task's `pr` by
its recorded number and URL, or omit it. State a next action only when current
status supports it; leave unknowns unknown. Include `reference.workspace.slug`
only when opening that workspace is the job.
A Task can have a checkout without a PR. Never guess a URL, branch or slug from a title or identifier; without a
link, keep the title and status.
