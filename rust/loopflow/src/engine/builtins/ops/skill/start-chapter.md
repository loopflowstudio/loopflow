---
requires: latest chapter review when available; Task worktree for tracked application
produces: accepted chapter plans, deterministic application receipts, publication status
---
Open a new chapter from user direction. Waves endure; each Wave has exactly
one current internal Project, replaced with fresh content at this boundary.
The Wave owns its objective and instruments; chapter JSON owns metric_targets,
KRs, and the Flow recommendation. Never create a second objective or inherit
omitted targets. Each metric_targets entry names metric_id and target
({"kind":"at_least","value":0.95} or {"kind":"at_most","value":10}).
Started unfinished Tasks move automatically. Untouched backlog expires as
abandoned/canceled. Completed Tasks remain historical. Code, worktrees, PRs,
Flow positions, Wave memory, and chat survive.

## Brief, then accept direction

Read the latest merged `.lf/chapters/<id>/start.md` and its review. Run
review-chapter when the evidence is missing or stale. Read the Wave roster,
GOAL/MEMORY, `lf status <wave> --json`, and `lf roadmap --json`.
Lead with experienced improvements, remaining friction, active work, and gaps.
Ask what mattered, surprised, or should become possible; reflect the user's
language and unresolved tensions. Discuss any Wave boundary changes.

**Gate 1:** the user explicitly accepts direction before scoped planning Runs.
In an interactive session, ask in the conversation. Headless, use `lf ask` and wait
for an explicit completed decision. Silence, elapsed time, or provider exit is
not acceptance. Existing accepted direction remains valid; do not ask again.

The tracked archive and charter edits belong in a Task worktree. If not in one,
return the direction draft and continue application in a Task worktree. Create
`.lf/chapters/<UTC timestamp>-<Run id prefix>/draft.md` with the accepted direction,
exact old chapter IDs, KR wording, evidence dates, and gaps.

## Shape one plan per Wave

Launch bounded `lf -b --wave <name> wave/start-chapter "<chapter id, accepted
brief, exact prior ledger>"` Runs. For a proposed uninitialized Wave use an
unbound Run with its exact name and boundary. Children propose, return agree or
challenge, and never mutate live planning. Do not launch Project planning Runs.

Archive each complete proposal, exact JSON plan and dry-run receipt under the
chapter directory. Reconcile challenges and show the user the consequential
Wave objectives, fresh KRs and metric targets, inherited started Tasks, untouched backlog closures, new Tasks,
and unresolved evidence. A failed/missing proposal stays explicit.

**Gate 2:** obtain explicit acceptance of the complete reconciled plan and its
Task dispositions before mutation. A material change after acceptance returns
to this conversation. Fixed carryover rules are not negotiable per-Task prompt
choices; changed evidence is reconciled by the API.

## Apply through one deterministic owner

Apply accepted Wave charter edits in this Task worktree. Initialize accepted
new Waves through the ordinary initialization path. Then run once per Wave:

```bash
lf wave new-chapter --wave <wave> --chapter <id> --plan <plan.json> --json
```

Save actual JSON output and exit status as the application receipt. The same
Wave/chapter ID is the idempotency key: retry an incomplete operation with the
same content and ID. Never mint another ID to escape an error. The API refreshes
Task facts, preserves starts racing with rotation, switches one binding, moves
active Tasks, cancels untouched backlog, and archives the predecessor resumably.
Never reimplement this as PM create/move/close/archive commands. Never restart
moved Tasks or count abandoned backlog as completed work.

File accepted fresh Tasks through `lf pm task create --wave <wave> --title
<title> --notes <description>`. The Wave resolves the current chapter. Rotation
itself never launches workers. Record failures and unresolved dispositions;
partial application is pending, never prose success. Seal `draft.md` as
`start.md` only when all accepted operations have succeeded.

## Publish separately

Curate durable lessons into Wave memory. Checkpoint the prior review, scoped
reports, accepted plans, actual receipts and charter edits with `lf commit`.
Obtain push authority unless already given, then use the selected PR workflow;
default to `lf pr submit` for a manual merge. Until merged, explicitly report
that operational application is complete but archive publication is pending.
Corrections append dated observations; never rewrite earlier evidence.

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
receipts instead of pasting raw IDs, timestamps, or routine no-op logs. Chapter
records still own application receipts. Proposals draft comments without posting.
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
Preserve the accepted brief when applying it; record application history in the
chapter record; summarize relevant changes in authorized Task comments, never
prepend or append allocation logs to the description.
