---
requires: a bound Wave or a proposed new Wave in the accepted direction
produces: one proposed Project plan, available repository preview, agree or challenge upward
---
Shape this Wave's next chapter from accepted user direction and dated evidence.
The Wave endures. Its current plan is one In Progress Linear Project; Planned
Projects hold future plans and Completed Projects retain history. The chapter
name is shared across Waves. Users choose Waves and Tasks; never introduce a
Project planning tier or treat past Projects as disposable.

## Ground the proposal

Read this exact Wave's GOAL.md, MEMORY.md, available chapter review, and
`lf wave status <wave> --no-sync --json`. Inspect the relevant Linear Projects
through available read-only access; record stable IDs, statuses and source dates.
The mandate and memory endure; new KRs, targets and backlog need their own
justification. Use the parent's accepted direction when supplied. Return a
material challenge to that parent instead of opening another session. In an
interactive session, ask in the conversation. Standalone headless work without
accepted direction uses `lf ask` for judgment before settling the proposal.
Existing acceptance remains valid; do not ask again.

Name the beneficiary and what improves in their experience. Keep the objective
on the Wave. Author proof-shaped KRs and metric targets with the full window
and denominator where applicable. Multiple outcomes can belong to one plan;
implementation steps belong in Tasks. Do not copy checked KRs or old targets.
A newly created successor inherits only the predecessor's Flow; an existing
Planned successor keeps its authored content. Preserve that proposal unless
accepted direction calls for a change. A missing default Flow requires resolution.

Produce a complete proposed Project content object:

```json
{"metric_targets":[{"metric_id":"example-metric","target":{"kind":"at_least","value":0.95}}],"flow":"feature","krs":[{"text":"Observable proof","holds":false}]}
```

Use the actual selected Flow as a nonempty string, not an assumed `feature`.
Omitted targets stay unset. Instruments and observations remain Wave-owned;
Project status changes do not copy metric readings. Historical judgments need
dated observations and the targets and revisions applicable to their interval;
unavailable evidence stays unknown.

## Preview the fixed boundary

Keep the proposal under `scratch/` or return it directly. With an accepted
repository target name, request a read-only preview:

```bash
lf repo new-chapter <name> --dry-run --json
```

This previews every Wave, not just this contribution. There is no plan-file or
Wave filter on rotation. Return its exact dated output and identify this Wave's
entry; it describes existing provider content, not the unapplied JSON proposal.
Report unavailable initialization or other-Wave conflicts without changing them.

The operation owns classification: started unfinished Tasks move with identity,
Started timestamp, worktree, PR and captured execution intact. Proven untouched
backlog becomes abandoned locally and canceled in Linear, retaining its issues
and history. Terminal work stays historical unless current execution evidence
conflicts. Missing local or remote work evidence cannot justify cancellation;
unresolved dispositions stay pending. Preparing a Task alone is not execution;
authored work and publication still count. Never replace this classifier with prose rules.
Author new Tasks against the objective and proposed KRs, accounting for carried
work without carrying earlier KR verdicts forward.

## Return, do not apply

Return `agree` or `challenge` against the accepted Wave boundary, with reasons,
exact JSON content, available preview output, proposed new Tasks, next review
date and evidence gaps. The repository start-chapter owner reconciles and applies
accepted changes. A standalone invocation returns the same scoped proposal; it
does not rotate, cancel Tasks, create Projects or edit the parent mandate.

Future plan authoring needs an available authorized Linear writer; there is no
`lf` writer for a Planned Project. `lf wave update-plan --wave <wave> --plan
<plan.json>` replaces only the current Project's complete content. Never apply
next-chapter JSON to the predecessor as a shortcut. If the writer is unavailable,
return the exact pending operation without inventing a command or local owner.

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
