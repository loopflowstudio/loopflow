---
requires: a bound Wave or a proposed new Wave in the accepted direction
produces: one fresh chapter plan, deterministic preview, agree or challenge upward
---
Shape this Wave's next chapter from accepted user direction and dated evidence.
The Wave is durable; its one internal Project is replaced every chapter. Users
choose Waves and Tasks. Never propose a Project portfolio or delegate to a
Project planning tier.

## Ground the proposal

Read this exact Wave's GOAL.md, MEMORY.md, chapter review, and current
`lf status <wave> --json`. The mandate and memory endure; chapter metric targets,
KRs, Flow recommendations, and unopened backlog do not inherit authority.
Use the parent's accepted direction when supplied. Return a material challenge
to that parent instead of opening another session. In an interactive session,
ask in the conversation. Standalone headless work without accepted direction
uses `lf ask` for judgment before settling the proposal.

Name the beneficiary and what improves in their experience. Keep the objective on the Wave. Author fresh metric targets and proof-shaped KRs: observable conditions, including the full
window and denominator where applicable. Multiple outcomes can belong to one
Wave chapter. Implementation steps belong in Tasks. Do not copy old checkmarks,
metric targets, backlog, or Flow recommendations.

Produce a JSON plan with this exact shape:

```json
{"metric_targets":[{"metric_id":"example-metric","target":{"kind":"at_least","value":0.95}}],"flows":{"recommended":null},"krs":[{"text":"Observable proof","holds":false}]}
```

## Preview the fixed boundary

For an existing Wave, write the proposed plan to a temporary file outside the repository and run:

```bash
lf wave new-chapter --wave <wave> --chapter <id> --plan <plan.json> --dry-run --json
```

Return its exact receipt with the proposal. The API owns classification:
started unfinished Tasks move with the same identity, worktree, PR and Flow;
untouched backlog is abandoned/canceled; completed work stays historical;
missing evidence stays unresolved. Preparing a Task alone is not execution.
These are fixed lifecycle rules, not recommendations the proposal may override.
New Tasks are freshly authored against the Wave objective and new KRs. Account for inherited
active work without carrying old KR verdicts forward.

For a new uninitialized Wave, report that the deterministic preview requires
initialization. Do not invent a receipt or create live planning before acceptance.

## Return, do not apply

Return `agree` or `challenge` against the accepted Wave boundary, with reasons,
exact JSON content, the preview receipt, proposed new Tasks, next review date,
and evidence gaps. Repository start-chapter archives and applies accepted plans.
A standalone invocation returns the same scoped proposal; it does not rotate
chapters, close Tasks, create Projects, or edit the parent mandate itself.

The Wave owns the enduring objective and measurement instruments. The chapter
owns KRs, Tasks, and targets referring to those instruments by metric_id.
Never author a separate chapter objective or a target in a Wave instrument.
An omitted target is unset; it is not copied from the previous chapter.
Review closed chapters using their frozen metric readings and
metrics_evaluated_at, not today's readings or newly edited instrument files.

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
