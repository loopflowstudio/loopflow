---
requires: latest chapter review when available; Task worktree for tracked application
produces: accepted chapter plans, deterministic application receipts, publication status
---
Open a new chapter from human direction. Waves endure; each Wave has exactly
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
Ask what mattered, surprised, or should become possible; reflect the human's
language and unresolved tensions. Discuss any Wave boundary changes.

**Gate 1:** the human explicitly accepts direction before scoped planning Runs.
With a human present, ask in the conversation. Headless, use `lf ask` and wait
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
chapter directory. Reconcile challenges and show the human the consequential
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
default to `lf pr submit` for a human merge. Until merged, explicitly report
that operational application is complete but archive publication is pending.
Corrections append dated observations; never rewrite earlier evidence.

## Task briefs

Write the Task from the user's perspective. Explain what they're trying to do, what
gets in their way, and why it matters. Give it a title naming the problem or desired
experience. Show what success would look like in a concrete moment of their work.
Preserve the user's own language when it anchors intent.

Ground the Task in observations, real constraints, and examples of success. Possible
solutions can help explain the idea; mark what remains uncertain. The design doc develops
the architecture, APIs, implementation sequence, and verification. A Task is ready for
design when the problem is clear, even if the solution isn't. Link accepted decisions
and keep them binding.

Keep the Task useful to someone choosing what to work on now. As understanding changes,
update the problem and desired experience. Keep blockers and decisions that affect that
choice visible, with links to evidence.

Preserve earlier reasoning in durable records without making readers replay every
checkpoint. Retain unresolved constraints, contrary evidence, and human decisions.
Give follow-up Tasks independently useful outcomes, rather than implementation layers.

Link related work where you explain its relevance. In prose and PR bodies, use
`[Title · Task ID or PR number](known URL)` on first mention; shorten later references
when unambiguous. State the relationship, such as builds on, supersedes, or verified by.
Use known URLs and preserve cited decisions and evidence somewhere that survives shipping.
In operational lists, put the ID first: `[Task ID or PR number · Title](known URL)`.

Apply these rules within the chapter’s existing proposal and acceptance boundaries.
Preserve the accepted brief when applying it; record application history in the
chapter record instead of prepending it to the Task.
