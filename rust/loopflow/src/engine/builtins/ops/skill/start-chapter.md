---
requires: latest chapter review when available; Task worktree for tracked edits
produces: accepted Project plans, repository rotation result, publication status
---
Open a new chapter from user direction. A chapter is the shared name of every
Wave's one In Progress Linear Project. Planned Projects hold future plans;
Completed Projects retain past plans and Tasks. Existing Project identities
survive. The Wave owns its objective, memory and instruments; Project content
owns metric targets, KRs and the required `flow:` default. There is no separate
Chapter record or local planning directory.

Started unfinished Tasks move with identity, Started timestamp, worktree, PR
and captured execution intact. Proven untouched backlog becomes abandoned
locally and canceled in Linear; its issues and history remain. Terminal Tasks
stay historical unless current execution evidence conflicts. Unresolved
dispositions require reconciliation; missing evidence never authorizes retirement.

## Brief, then accept direction

Read the available dated chapter review and Linear Project plans by stable ID,
including their statuses and content. Run review-chapter when evidence is missing
or stale. Read the Wave roster, GOAL/MEMORY,
`lf wave status <wave> --no-sync --json`, and `lf roadmap --json`.
Name stale or unavailable planning and historical evidence explicitly; today's
Project content and metric readings cannot establish an earlier boundary.
Lead with experienced improvements, remaining friction, active work, and gaps.
Ask what mattered, surprised, or should become possible; reflect the user's
language and unresolved tensions. Discuss any Wave boundary changes.

**Gate 1:** the user explicitly accepts direction before scoped planning Runs.
In an interactive session, ask in the conversation. Headless, use `lf ask` and wait
for an explicit completed decision. Silence, elapsed time, or provider exit is
not acceptance. Existing accepted direction remains valid; do not ask again.

Tracked charter edits belong in a Task worktree. If not in one, return the
direction draft and continue those edits in a Task worktree. Keep temporary
proposals and evidence under `scratch/`, recording accepted direction, target
chapter name, stable Project IDs, exact KR wording, evidence dates and gaps.
These notes do not own live planning or recovery.

## Shape one plan per Wave

Launch bounded `lf -b --wave <name> wave/start-chapter "<chapter name, accepted
brief, exact prior ledger>"` Runs. For a proposed uninitialized Wave use an
unbound Run with its exact name and boundary. Children propose, return agree or
challenge, and never mutate live planning. Do not launch Project planning Runs.

Keep each complete proposal and exact JSON plan in scratch. Preview the whole
repository with `lf new-chapter <name> --dry-run --json`; there is no
per-Wave rotation or plan-file argument. A preview classifies current provider
facts, not unapplied proposal content. Reconcile challenges and show the user
Wave objectives, proposed KRs and metric targets, retained successor content,
inherited started Tasks, backlog cancellations, new Tasks and unresolved evidence.
A failed or missing proposal stays explicit.

**Gate 2:** obtain explicit acceptance of the complete reconciled plan and its
Task dispositions before mutation. Existing acceptance remains valid. A material
change after acceptance returns to this conversation. Fixed carryover rules are
not negotiable per-Task prompt choices; changed evidence is reconciled by the API.

## Apply through one deterministic owner

Apply accepted Wave charter edits in this Task worktree and connect accepted
new Waves. Preserve existing Linear Projects. For an authored successor plan,
create or edit its Planned Project in Linear through an available authorized
provider interface, retaining its Wave Initiative and repository Team. There is
no `lf` command to write a future Project plan; report that exact limitation if
no provider writer is available. Never use `wave update-plan` to write a future
plan into the predecessor.

Without a prepared successor, rotation creates an empty plan carrying only the
predecessor's Flow; a Wave with no Projects starts with `feature`. An existing
Planned successor retains its authored Flow, KRs and targets. Missing default
Flow or competing current Projects require explicit resolution in Linear.
Refresh the repository preview after preparation, then apply the accepted name:

```bash
lf new-chapter <name> --dry-run --json
lf new-chapter <name> --json
```

Retain dated command output and exit status as evidence. Retry an interrupted
rotation with the same name. Fresh Linear statuses and stable Project IDs own
recovery; command output is not a durable recovery record. Rotation activates
successors, rechecks Task dispositions, moves started work, cancels untouched
backlog and completes predecessors. It is not atomic across Waves or Homes;
partial application can leave both Projects In Progress. Report conflicting
plans or external reassignments instead of choosing by name recency. Never
reimplement rotation as individual provider mutations, restart moved Tasks,
delete predecessor Projects, or count canceled backlog as completed work.

To apply an accepted plan after its Project becomes current, use:

```bash
lf update-plan --wave <wave> --plan <plan.json>
```

The file replaces the complete current Project content:

```json
{"metric_targets":[],"flow":"feature","krs":[{"text":"Observable proof","holds":false}]}
```

`flow` must be a nonempty string. Each metric target names `metric_id` and a
`target` such as `{"kind":"at_least","value":0.95}`. Omitted targets stay
unset; readings remain Wave-owned and rotation copies none of them. Read back
the resulting Project content and status before claiming the plan applied.

File accepted fresh Tasks through `lf task create --wave <wave> --title
<title> --notes <description>`. The Wave resolves the current Project. Rotation
itself never launches workers. Record failures and unresolved dispositions;
partial application is pending, never prose success. A successful rotation
alone does not prove separately accepted plan edits or Task creation succeeded.

## Publish separately

Curate durable lessons into Wave memory and retain useful decisions and evidence
in existing durable records before scratch cleanup. Linear retains Project
plans and history; do not create a second local chapter owner. Checkpoint
coherent repository edits with `lf commit`. Obtain push authority unless already
given, then use the selected PR workflow; default to `lf submit` for a manual
merge. Until merged, explicitly report that operational application is complete
but repository publication is pending. Corrections append dated observations;
never rewrite earlier evidence.

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
receipts instead of pasting raw IDs, timestamps, or routine no-op logs. Retain
dated application evidence in existing records. Proposals draft comments without posting.
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

Link related work where you explain its relevance. In prose and PR bodies, use
`[Title · Task ID or PR number](known URL)` on first mention; shorten later references
when unambiguous. State the relationship, such as builds on, supersedes, or verified by.
Use known URLs and preserve cited decisions and evidence somewhere that survives shipping.
In operational lists, put the ID first: `[Task ID or PR number · Title](known URL)`.

Apply these rules within the chapter’s existing proposal and acceptance boundaries.
Preserve the accepted brief when applying it; retain dated application evidence;
summarize relevant changes in authorized Task comments, never
prepend or append allocation logs to the description.
