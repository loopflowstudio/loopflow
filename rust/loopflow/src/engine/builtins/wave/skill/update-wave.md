---
requires: diff vs main | scratch/ analysis | both
produces: wave/<wave>/ (GOAL.md, MEMORY.md), chapter/Task updates, scratch/ cleanup
---
Keep the Wave's durable identity current and fold what the branch learned into memory.

## Read

Read scratch, the diff, the identified Wave's GOAL/MEMORY and the repo guide.
Use `lf status <wave> --json` when chapter or Task state matters; memory curation
alone needs no running Wave or PM connection. Do not infer a different Wave.
With no named Wave, preserve actionable rules beside their code or in the local
skill that exercises them; keep unresolved placement in the PR. Do not create
root `.lf/` learning notes or make another copy of general customer guidance.

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

Author GOAL and MEMORY, then initialize with `lf pm init --wave <wave>`. This
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
checkpoint. Retain unresolved constraints, contrary evidence, and recorded decisions.
Give follow-up Tasks independently useful outcomes, rather than implementation layers.

Link related work where you explain its relevance. In prose and PR bodies, use
`[Title · Task ID or PR number](known URL)` on first mention; shorten later references
when unambiguous. State the relationship, such as builds on, supersedes, or verified by.
Use known URLs and preserve cited decisions and evidence somewhere that survives shipping.
In operational lists, put the ID first: `[Task ID or PR number · Title](known URL)`.
