---
requires: diff vs main | scratch/ analysis | both
produces: wave/<wave>/ (GOAL.md, MEMORY.md), chapter/Task updates, scratch/ cleanup
---
Keep the Wave's durable identity current and fold what the branch learned into memory.

## Read

Read scratch, the diff, the owning Wave's GOAL/MEMORY and the repo guide.
Use the named Wave when supplied; otherwise identify the owner from the work's
context and repository Wave objectives. If the repository has no Waves, create
one named for the repository, with a repo-wide GOAL grounded in its purpose and
a MEMORY for its durable lessons. This local memory setup needs no PM binding
or running Wave. If existing Waves leave ownership genuinely ambiguous, ask the
present human; headless, record the question in scratch. Never use `.lf/` as a
fallback memory location.

Use `lf status <wave> --json` when chapter or Task state matters; memory curation
alone needs no running Wave or PM connection. Put actionable rules beside their
code or in the skill that exercises them, and durable lessons in Wave memory.

## Reconcile

- Curate durable decisions into `wave/<wave>/MEMORY.md`; never append a transcript.
- Edit GOAL only when mandate, bounds, or cadence changed. Keep chapter KRs and
  Task lists out of the enduring mandate.
- The Wave has exactly one current chapter plan, stored in an internal Project.
  Correct its metric targets, Flow recommendation, and KRs through
  `lf wave update-plan --wave <wave> --plan <plan.json>`. Preserve unrelated
  authored content. The objective belongs to the Wave; never add a chapter
  objective. KRs prove outcomes, not task counts or implementation activity.
- Close shipped Tasks with `lf pm task done --id <issue>`; correct stale wording
  with `lf pm task update --id <issue> --title "…" --notes "…"`.
- File useful discoveries with `lf pm task create --wave <wave> --title "…" --notes "…"`.
  Filing does not authorize starting a worker.
- Enduring metric contracts live under `wave/<wave>/metrics/`. They belong to
  the Wave and retain their identity across chapters. Targets belong to the
  chapter plan; omitting a target leaves it unset.

A chapter boundary belongs to start-chapter and `lf wave new-chapter`, never
ad hoc Project creation, Task transfer, or backlog cleanup. Started Tasks carry;
untouched backlog expires; uncertain evidence stays unresolved. Do not rotate a
chapter as incidental cleanup.

## New Wave

Author GOAL and MEMORY. Local memory curation stops there. When connecting the
Wave to durable planning, initialize with `lf pm init --wave <wave>`. This
provisions the first empty chapter. Author its plan through `lf wave update-plan`.
Do not create another Project or introduce a Project operator.

## Finish

Check that memory is curated, the mandate is enduring, current chapter evidence
is truthful, and concrete work lives in Tasks. Remove scratch only after its
useful decisions and evidence are captured and the destination verified.
Link history from durable records; do not stack competing narratives in a Task
or use worker steering merely to archive prose. Commit repository edits through `lf commit`;
publication remains a separate authorization boundary.

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
