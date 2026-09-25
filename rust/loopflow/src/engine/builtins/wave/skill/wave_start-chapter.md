---
requires: a bound Wave or a proposed new Wave in the accepted direction
produces: one fresh chapter plan, deterministic preview, agree or challenge upward
---
Shape this Wave's next chapter from accepted human direction and dated evidence.
The Wave is durable; its one internal Project is replaced every chapter. Users
choose Waves and Tasks. Never propose a Project portfolio or delegate to a
Project planning tier.

## Ground the proposal

Read this exact Wave's GOAL.md, MEMORY.md, chapter review, and current
`lf status <wave> --json`. The mandate and memory endure; chapter metric targets,
KRs, Flow recommendations, and unopened backlog do not inherit authority.
Use the parent's accepted direction when supplied. Return a material challenge
to that parent instead of opening another human session. With a human present,
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
