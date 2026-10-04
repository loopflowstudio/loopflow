---
description: Operate one Wave through delivery, coordination, capacity, adaptation and identity; focus on a few useful moves.
action_style: procedural
---
Operate the selected Wave once: read, judge, act, and exit. Aim for one or two
useful authorized moves; let the evidence determine how much is worthwhile.
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
cannot establish an empty backlog, health, approval or permission to close work.
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
current authority stay proposals for review; never silently rewrite another
Wave's objective. Return accepted decisions, their source and practical
consequence to affected Task briefs or authorized discussions, retaining
disagreement. A draft or failed delivery leaves the handoff pending.

## The hierarchy is an intent graph, not a control plane

Tasks progress independently of Wave and repository passes. Neither operation
is an approval prerequisite. Each Task's selected Flow carries its work through
landing; do not perform that Flow's steps or create another cursor here. Preserve selected Flows, review gates,
worktrees, placement and execution history.

- Start selected authorized work with `lf --task <issue-id> flow start`; use the
  current Project's Flow unless an explicit choice is warranted. Existing
  execution is reconciled through Task operations, never a duplicate driver.
- Inspect `lf task status` and existing logs before recovery. Resolve authorized
  impediments; discuss missing judgment in the ongoing Wave chat when present.
  Headless operation stops with the reason when it cannot proceed. Retry failed
  work with `lf --task <issue> flow start --reason "<what changed>"` only when new
  evidence warrants it. Unknown liveness is not idle. Authored Task review
  Sessions retain their own feedback and completion contract.
- Read existing Tasks before `lf task create --wave <wave>`. Use `--run` only
  when execution is intended and authorized. Give work an observable outcome.
- Update an authorized plan through `lf update-plan --wave <wave> --plan
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
without introducing a resident or another execution cursor.

A chapter boundary previews the whole repository with
`lf repo new-chapter <name> --dry-run --json`. Apply with
`lf repo new-chapter <name> --json` only after repository direction and disposition
review gates are satisfied; existing authorization remains valid. An ordinary
Wave pass does not authorize rotation. The operation has no per-Wave selector
or plan input. Started work moves intact, proven untouched backlog is canceled
with history retained, and predecessors become Completed. Uncertain evidence
blocks automatic retirement. Retry the same name after interruption.

Planned and Completed Projects retain future and past plans. Future plan edits
need an authorized Linear writer; `update-plan` cannot target a Planned successor.
Judge prior chapters using dated evidence, not current metric readings or
reconstructed starting membership. Do not introduce competing current Projects.

## Finish

Reply in this conversation with the decisive source, judgment, action/result
and unresolved decisions or evidence gaps. If no move is useful, say why briefly.
Persist only changed decisions, durable learning and unresolved concerns at
their existing owners. External posting requires authorization and confirmed
delivery; this skill does not automatically send the reply to a channel.

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
changes, launch attempts, and verification updates in Task comments when posting
is authorized. A comment should say what changed and what it means; link detailed
receipts instead of pasting raw IDs, timestamps, or routine no-op logs. Draft
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

## Resident workspace and document publication

Keep accepted decisions in `wave/<wave>/MEMORY.md`, with names and dates.
Keep working plans in `scratch/`. The scope's resident checkout survives conversation
replacement; publication needs no synthetic Task or separate export checkout.

At a deliberate maintenance or publication boundary, coordinate with active writers,
inspect `git status --short` and `lf sync --plan`, then run `lf sync`. Do not sync on
every message. Fetch failure leaves local work usable. Resolve conflicts in place
and run `lf sync --continue`, or use `lf sync --abort` to restore the starting point.
Commit changes that block integration deliberately, preserving unrelated work.
If resolver notes collide with stashed files, sync restores the originals and keeps
the new notes beside them as `<name>.lf-sync-N`. Reconcile both locally. A failed
restoration retains its stash and prints recovery instructions; keep it until all
edits are recovered.

When publication is authorized:

```bash
git diff -- wave/<wave>/MEMORY.md
lf commit -m "Record accepted decisions" wave/<wave>/MEMORY.md
git diff origin/main...HEAD --stat
lf pr publish
```

Selected-path commits preserve unrelated staged and unstaged edits. Inspect the
entire committed range before publishing: that is what the PR contains. Resident
commit and publication untrack scratch without deleting local files. Scratch,
including PR copy, and edits made after the selected commit survive delivery.

After merge, run `lf sync` before the next document commit: once everything
committed has landed, it restarts the branch from main. Reuse the same branch;
automatic pruning retains it. Moved worktrees are reused at their actual path.
Missing checkouts recover committed state only. Live conversations retain their
placement until an idle driver boundary permits adoption of the resident workspace.

`lf wt create <name> --resident` creates or reuses an independent document workspace.
PR landing clears scratch in non-resident workspaces, whether or not a Task is
associated. Memory updates need no PR or distribution schedule.
