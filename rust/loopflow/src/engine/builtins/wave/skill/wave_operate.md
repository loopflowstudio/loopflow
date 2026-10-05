---
description: Operate one Wave through delivery, coordination, capacity, adaptation and identity; focus on a few useful moves.
action_style: procedural
---
Operate the selected Wave once: read, judge, act, and exit. Aim for one or two
useful moves; let the evidence determine how much is worthwhile.
No action is a valid result. No chat or schedule is required.

## Scope and evidence

Resolve the exact Wave from the invocation's explicit selection or supplied
bound Wave context. Do not guess from an arbitrary GOAL file. If selection is
missing or contradictory, report the missing scope without operating another
Wave. Read its objective and memory, current Project's plan, Task outcomes and
unresolved concerns. The Project is part of the Wave, not another operator.

Start with `lf wave status <exact-wave> --json` and
`lf roadmap --wave <exact-wave> --json`. Read relevant Task detail with
`lf task status <issue> --json`. Follow summaries to the source only where a
claim, conflict or missing fact matters; do not ingest every transcript.

Keep source links/paths, dates and freshness with consequential findings;
separate observations, hypotheses and proposals. Rereading cached content does
not refresh it. Failed reads, stale plans, missing outcomes and unknown liveness
cannot establish an empty backlog, health, or a completed outcome.
Continue independent work from available evidence without repairing auth or
waiting indefinitely.

## Exercise all five functions within the Wave

Use these questions together; investigate the material tension, not five
mandatory reports or five agents.

| Function | Question and possible response |
| --- | --- |
| S1 · Delivery | What useful outcome did a Task deliver? Compare actual results with its intended beneficiary; select an existing next Task when warranted. |
| S2 · Coordination | Where do Tasks collide or depend on each other? Clarify the concrete interface or ordering without serializing unrelated work. |
| S3 · Present capacity | Is effort advancing outcomes? Distinguish active work, review waits, repeated failures and unused capacity before reallocating effort. |
| S4 · Adaptation | What changed in user needs, code or the outside environment? Follow dated evidence that could invalidate the plan; propose a bounded response. |
| S5 · Identity | Do the objective and boundaries still describe useful work? Let Task evidence challenge the Wave's premise, including tension between current commitments and future needs. |

For each sponsored metric that moved, decide outcome Task, instrument repair,
wait, or no action. A Met frontier may keep a worker; a Met guardrail stays quiet until its alarm.
Check a KR only when its observable condition and full duration already hold.
Silence in chat, a completed agent turn, or a green PR is not outcome evidence.

Follow the relevant signal into its source: PR/path overlap and repeated
conflicts for coordination; retries, CI waits, resource usage and actual outcomes
for capacity; dated dependency, advisory or API changes for adaptation; gaps,
contradictory boundaries and accepted decisions for identity. Distinguish urgent
response from preparation or a watchlist. Do not invent a worker count, require
all Waves to move at the same rate, or change policy merely to produce an action.

## Identity travels both ways

A Task finding can challenge Wave purpose; a Wave finding can challenge
repository direction. Record the sourced finding, challenged premise,
consequence, proposed change and competing interpretations in the relevant
Wave memory or existing Task discussion. For cross-Wave concerns, name the
affected Waves and who needs to decide. Repository synthesis participates in
identity without owning it alone.

Within accepted direction, reconcile the Wave plan or memory. Changes beyond
current direction stay proposals for review; never silently rewrite another
Wave's objective. Return accepted decisions, their source and practical
consequence to affected Task briefs or discussions, retaining
disagreement. A draft or failed delivery leaves the handoff pending.

## The hierarchy is an intent graph, not a control plane

Tasks progress independently of Wave and repository passes. Neither operation
is an approval prerequisite. A Task's Flows carry its autonomous work; do not
perform a Flow's steps here. Preserve Flow history, conversation reviews,
worktrees, placement and execution history.

- Start selected work with `lf task run <issue-id>`; use the
  current Project's Flow unless an explicit choice is warranted. Each run
  is a fresh Flow and returns when it ends; background it with your own tool.
  Leave a live one running instead of duplicating it.
- Inspect `lf task status` and existing logs before recovery. Resolve
  impediments; discuss missing judgment in the ongoing Wave chat when present.
  Headless operation stops with the reason when it cannot proceed. After a
  failure, inspect its effects and launch fresh work with
  `lf task run <issue> <flow> --reason "<what changed>"` only when new
  evidence warrants it. Unknown liveness is not idle. Preserve historical review
  evidence; discuss feedback and next work in the ongoing Task conversation.
- Read existing Tasks before `lf task create --wave <wave>`. Give work an
  observable outcome.
- Update the plan through `lf update-plan --wave <wave> --plan
  <plan.json>`; curate durable decisions in `wave/<wave>/MEMORY.md`. Change
  GOAL.md only within accepted direction. Supply the complete content object
  with `metric_targets`, required nonempty `flow` and `krs`; for example
  `{"metric_targets":[],"flow":"feature","krs":[]}`. This edits the Wave's
  one In Progress Linear Project. Preserve authored Flow and other plan fields.
- Use supported Task/Work operations for changes, not raw stores, unclaimed
  process signals or a competing worker on this Home. A missing Home stays an
  explicit blocker.

Distinguish completed effects, proposals, failed writes and unresolved readback;
reconcile uncertain effects before retry. Keep planning at its existing owners
without introducing a long-running process or another execution cursor.

A chapter boundary previews the whole repository with
`lf repo new-chapter <name> --dry-run --json`. Apply with
`lf repo new-chapter <name> --json` only after repository direction and disposition
review gates are satisfied. Chapter rotation is a separate operation with no
per-Wave selector or plan input. Started work moves intact, proven untouched backlog is canceled
with history retained, and predecessors become Completed. Uncertain evidence
blocks automatic retirement. Retry the same name after interruption.

Planned and Completed Projects retain future and past plans. Future plan edits
need a Linear writer; `update-plan` cannot target a Planned successor.
Judge prior chapters using dated evidence, not current metric readings or
reconstructed starting membership. Do not introduce competing current Projects.

## Finish

Reply in this conversation with the decisive source, judgment, action/result
and unresolved decisions or evidence gaps. If no move is useful, say why briefly.
Persist only changed decisions, durable learning and unresolved concerns at
their existing owners. Report external posts only after confirming delivery;
this skill does not automatically send the reply to a channel.

When reporting more than one Task, use the status/roadmap reads above and render
operational rows as
`[identifier · Task title](provider URL) — status; next action/owner`.

Use this ID-first form in operational lists. In prose, use
`[Task title · identifier](provider URL)` on first mention; shorten later
references when unambiguous.

Fill the link from `task.identifier` and `reference.issue_url`, and the readable
title from `task.title`. Take status from `runtime.status` or the roadmap
`section`, and next owner from `next_move.owner`. State the next action only
when supported by current evidence; leave unknown state unknown. Include an
active PR/workspace slug only when navigating that workspace is the job. In
roadmap, use `active_pr.slug`; in status, match `active_pr` to `prs[].id` and
use that PR's `slug`. Fall back to `reference.workspace.slug`. Never infer a
URL, branch, or slug from a title or identifier. If a link is absent, keep the
readable title and available status without inventing a URL.

## Task briefs

Write a description someone can understand without the planning conversation.
Open with what the person is trying to do, what gets in their way, and what should
improve. Use a short title naming that improvement. Preserve useful user language;
use concrete verbs rather than process phrases such as “establish the bounded
outcome” or “settle the publication commitment”.

Usually one or two short paragraphs and a few acceptance bullets are enough.
Use headings only when they help. Describe observable success, including required
numbers, windows, failure cases, and constraints; brevity must not erase them.
Technical Tasks can serve maintainers or operators without inventing a customer.
Mark possible solutions as tentative. Keep architecture, implementation steps,
and detailed proof in the design, linked and available to the worker.

Keep the description current. Put dated progress, chapter allocation, queue
changes, launch attempts, and verification updates in Task comments.
A comment should say what changed and what it means; link detailed receipts instead of pasting raw IDs, timestamps, or routine no-op logs. Draft
proposed comments without posting.
Do not use a description update or worker steering as a substitute log channel.

Comments may be collapsed by default. Keep current blockers, actual dependencies,
accepted scope, and unresolved contrary evidence summarized in the description
when they affect the work. A queue position is not necessarily a dependency.
When a comment changes the accepted scope, reconcile the description and retain
the comment as history. Do not append dated amendments or require readers to
reconstruct the current brief from the thread. Preserve decisions and evidence
before removing superseded prose.

For example, describe: “After an interrupted release, maintainers need to see
whether anything shipped and safely continue unfinished work.” Acceptance can
require that a retry never publishes twice and that failures remain visible.
Put “Moved into the September chapter; scheduled after the installation repair”
in a comment. Include that repair in the description only if it is a real blocker.
Give follow-up Tasks independently useful outcomes, rather than implementation layers.

Link related work where you explain its relationship: builds on, supersedes,
or verified by. Use the link forms above for Tasks and PRs, and preserve cited
decisions and evidence somewhere that survives shipping.
