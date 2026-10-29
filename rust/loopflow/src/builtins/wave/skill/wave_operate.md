---
description: Operate one Wave; keep your started Tasks moving, leave other people's work alone, and judge the existing work.
action_style: procedural
---
Operate the selected Wave once: read, judge, act, verify, and exit. You are this
Wave's operator for your own work: a started Task you own with no live Graph
process has nobody else to take its next step. A pass is finished when every
started Task you own has a disposition and every other Task is listed, not
after a number of moves. It works the Wave's existing Tasks and leaves the
plan, its key results and metric targets, chapter rotation, new Tasks and
backlog to the person. No chat or schedule is required. Another conversation
or scheduled pass may be operating this Wave at the same time; work it is
driving is moving, and you leave it alone.

Writes to `.lf/wave/` belong to the selected Loopflow Graph's procedure. Outside
that Graph, keep proposed Wave changes in scratch; Work association alone
does not select memory maintenance.

## Scope and evidence

Resolve the exact Wave from the invocation's explicit selection or supplied
bound Wave context. Do not guess from an arbitrary GOAL file. If selection is
missing or contradictory, report the missing scope without operating another
Wave. Read its objective and memory, current Project's Tasks, their outcomes and
unresolved concerns. The Project is part of the Wave, not another operator.

Start with `lf wave status <exact-wave> --json` and
`lf wave show <exact-wave> --json`. Read relevant Task detail with
`lf task status <issue> --json`. Follow summaries to the source only where a
claim, conflict or missing fact matters; do not ingest every transcript.

Keep source links/paths, dates and freshness with consequential findings;
separate observations, hypotheses and proposals. Rereading cached content does
not refresh it. Failed reads, stale plans, missing outcomes and unknown liveness
cannot establish an empty backlog, health, or a completed outcome.
Continue independent work from available evidence without repairing auth or
waiting indefinitely.

## Keep started work moving

Ownership (`runtime.owner` in `lf wave show --json`) is the recorded fact,
whether or not a Graph process is alive now. Compare its `person` with
`lf config user`:

- **Yours:** the owner's person is you, or the owner names no person. Give
  every unfinished Task you own exactly one disposition, with its evidence:
  the first row below that fits. A Task you own whose `runtime.owner.machine`
  is not this Machine (`lf id`) has unknown liveness here: operate that one
  Task on its Machine with `lf --machine <machine> task-operate <issue>` and
  file its returned disposition under the matching heading in Finish, or
  report it unknown and name the Machine. Never `lf task run`, recover or
  complete it from this Machine.
- **Theirs:** another person owns it. List it with that person and Machine as
  “theirs · left alone”. Do not run, recover, complete, claim, move,
  interrupt or comment on it, and do not judge its liveness: their Processes
  are not visible here, so idle-looking is not evidence.
- **Unowned:** backlog. List it and leave it alone; starting it is the person's
  selection.

An unreadable owner or unset `lf config user` makes ownership unknown: report
the Task as unknown and do not act on it. Include Tasks status lists under
`unavailable_tasks`; they are unknown until read.

| Disposition | Evidence and action |
| --- | --- |
| moving | A live Graph process was observed. Leave it alone. A connected conversation or review Session is not a Graph process. |
| acted | This pass continued, recovered or delivered it through a supported control, then reread status and saw the effect. |
| available | Work the person can take up now: a review, demo, concrete choice or merge click, with its conversation link or PR. Never completed or approved for them. |
| dependency or capacity | The named Task, PR, running check, account or limit. |
| deferred | The person's instruction to stop, or an explicit hold (`lf task defer <issue>`) on work nothing else is moving. |
| unknown | The named read or liveness evidence that is missing. Unknown is not idle and never a reason to start a second Graph process. |

“Ready”, “needs reconciliation” and “the Wave owns this” are not dispositions.
Judge ownership first. When status gives `next_move.owner` as `wave` on a
Task you own, its disposition is yours to establish, never a handoff; another
person's Task stays “theirs · left alone” whatever `next_move` says. Its
recommended action is a suggestion to check against the rules below, not an
instruction. No action is a valid result only when every started Task you own
already holds a disposition above.

- **Look for a live Graph process first.** Read `lf task status <issue> --json` and
  check every Graph in `execution.work.graphs` and every unfinished Process in
  `execution.work.processes` against `lf ps --json`. A live process makes the Task
  moving, whoever launched it. Unknown liveness is not idle.
- **Recover before reporting a blocker.** Read the failed step's log and
  inspect its effects. Repair a supported cause, then launch only the remaining
  authorized work with `lf task run <issue> <graph> --reason "<what changed>"`.
  A stopped Graph is history and cannot be resumed. Retry only on new evidence;
  report an unchanged failure instead of repeating it.
- **A Graph ends where authored.** A finished Graph or published PR does not
  complete the Task. Name what remains and the PR or Task conversation awaiting
  a decision. A Task waiting at an interactive step (`work.graph` in status) is
  available to the person there; start a run that stopped or failed again with
  `lf task run <issue>`, and do not continue past their step for them, choose a
  new direction or arm an unapproved merge.
- **Retry completion alone.** A durable completion request at end retries with
  `lf task complete <issue>`, never `task run` or gate. Completion, graph
  position and Process liveness are independent. A newer reopening supersedes
  old intent; end alone cannot re-complete it.
- **Finish merged delivery.** Merge alone does not complete a Task. Inspect
  associated Graphs and unfinished Processes; leave a live ship Graph to finish.
  Otherwise, when the accepted outcome is satisfied, run `lf task complete <issue>`
  and reread status. No follow-up filing is required. Do not replay gate or
  re-arm a merged PR.
- **Reviews stay in the Task conversation.** Surface the Session from
  `lf session list --available --json`. Discuss feedback and preserve agreed
  direction there; closing a Session supplies no navigation authority.
  Headless, required judgment leaves the Task available for the person.

Start authorized remaining work on your Tasks without asking. Do not approve
reviews, arm a merge, cancel or abandon Tasks, or change direction.

## Return to due follow-ups

Read due dates and briefs in the shared plan. List unstarted Tasks that are
due or overdue separately from unrelated backlog, for the person to select;
they are unowned backlog. Do not start one: elapsed time never authorizes
execution or completes the Task. A started Task you own whose date has
arrived takes its ordinary disposition above and is not listed again; before
that date its disposition is dependency, naming the date. Use the date and any
timezone, release or environment prerequisite in the brief; a due date is a
day, not an exact-time alarm. Missing date or provider evidence stays unknown. This pass
installs no schedule; an open conversation does not schedule its next turn.

## Judge the Wave's existing work

Use these questions together over the Wave's existing Tasks; investigate the
material tension, not five mandatory reports or five agents. Report findings
and proposals in the reply.

| Function | Question and possible response |
| --- | --- |
| S1 · Delivery | What useful outcome did a Task deliver? Compare actual results with its intended beneficiary. |
| S2 · Coordination | Where do Tasks collide or depend on each other? Name the concrete interface or ordering without serializing unrelated work. |
| S3 · Present capacity | Is effort advancing outcomes? Distinguish active work, review waits, repeated failures and unused capacity. |
| S4 · Adaptation | What changed in user needs, code or the outside environment? Follow dated evidence that could invalidate a Task's direction. |
| S5 · Identity | Does a Task's evidence challenge the Wave's objective or boundaries? Name the finding, the challenged premise and who needs to decide. |

Follow the relevant signal into its source: PR/path overlap and repeated
conflicts for coordination; retries, CI waits, resource usage and actual outcomes
for capacity; dated dependency, advisory or API changes for adaptation.
Silence in chat, a completed agent turn, or a green PR is not outcome evidence.
Do not change policy merely to produce an action.

## The hierarchy is an intent graph, not a control plane

Tasks progress independently of Wave and repository passes. Neither operation
is an approval prerequisite. A Task's Graphs carry its autonomous work; do not
perform a Graph's steps here. Preserve Graph history, conversation reviews,
worktrees, ownership and execution history.

- Continue a Task you own with `lf task run <issue-id>`; it runs the Task's
  graph until the next interactive step. Each run is a fresh Graph process and
  returns once it is running; `lf task status <issue-id>` follows it. Leave a live one running
  instead of duplicating it. `lf -b task run` and `lf ship` approve Auto-Ship,
  which may merge the PR; use them only when the person asks to ship or land.
- Use supported Task/Work operations, not raw stores, unclaimed process
  signals or a competing worker on this Machine. A missing Machine stays an
  explicit blocker.

Distinguish completed effects, failed writes and unresolved readback; reconcile
uncertain effects before retry. Introduce no long-running process or another
execution cursor.

## Finish

Reply in this conversation with each started Task once, grouped under these
headings in this order, omitting an empty group:

1. **Available** — each review, demo, choice or merge the person can take up, with its link.
2. **Moving** — live work left alone, and what this pass acted on with its result.
3. **Stuck** — a dependency or capacity, deferred, or unknown, with
   the named cause or missing evidence.
4. **Others' work** — each Task another person owns, with that person and
   Machine, “left alone”.

Then list unstarted due follow-ups for selection, other backlog, and after it any
judgment, proposals, unresolved decisions and evidence gaps. Keep each row to its disposition and decisive evidence. If
nothing needed action, say why briefly. Persist only changed decisions, durable learning and unresolved concerns at
their existing owners. Report external posts only after confirming delivery;
this skill does not automatically send the reply to a channel.

When reporting more than one Task, use the status/roadmap reads above and render
operational rows as
`[identifier · Task title](Work URL) — status; next action/owner · [PR #n](PR URL)`.
`lf open --task <issue> --link --format markdown` prints that label with its
Work URL, which opens the Task's conversation through `lf open`; the provider
issue (`reference.issue_url`) is a separate link when provider navigation is
the point. Where links may not activate, add the `lf open '<url>'` it prints.
Use this ID-first form in operational lists; in prose, shorten later
references when unambiguous.

Take the identifier from `task.identifier` and the readable title from `task.name`. Give the Task's disposition as its status, not
`runtime.status` alone, and take the next owner from `next_move.owner`, read
as described under ownership above. Link the Task's optional `pr` using its recorded GitHub number and URL; omit
the PR link when absent. State the next action only when supported by current evidence; leave
unknown state unknown. Include an
workspace slug only when navigating that workspace is the job, using
`reference.workspace.slug`. A Task can have a checkout without a PR. Never infer a
URL, branch, or slug from a title or identifier. If a link is absent, keep the
readable title and available status without inventing a URL.
