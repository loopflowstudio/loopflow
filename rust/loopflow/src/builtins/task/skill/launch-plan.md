---
requires: a design or findings that may warrant implementation
produces: an execution decision and any Task handoffs
action_style: procedural
---
Turn the design into an execution decision. Continue the caller's chosen way
of working; use Task controls for existing or explicitly requested managed work.
Do not create a manifest, receipt, marker, or new planning state.

## Taskless work

When no Task is associated and the request does not ask for managed work, keep
selected implementation in this checkout. Follow the caller's Graph or continue
implementation directly; retain independent later ideas in the design. Report
that decision and finish this step. Do not look up a Wave or create Tasks merely
because a Graph, such as `incident`, includes this skill.

For existing or explicitly requested managed work, read `lf help skill work` and
continue below.

## Orientation

Read the supplied design or findings, repo guide, and any named Task's current
state. Consult Wave chapter state only when the seed identifies it.

Choose what is worth implementing before allocating work. Incident findings may
justify prevention, more investigation, or no further work. Preserve unresolved
evidence and distinguish proposals from accepted scope. Reuse existing Tasks for
the same outcome; do not create a prevention project merely to fill a Graph step.
Before filing work against specific files, list the open PRs that change them
(`gh pr list --json number,title,headRefName,files`) and name each overlapping
PR, its Task and any choice it reserves for a person in the new brief.
When no useful change remains, record that decision and finish.

A standalone invocation need not already have a Task. Use the existing-design
handoff below for selected work, retaining unresolved ownership or direction
as an explicit next decision. Never launch a competing Graph into active work.

## Decide what stays here

Choose the ambitious single-threaded core whose implementation will settle the
contract for the rest of the work. Keep that core in this Task and describe its
boundary clearly enough for implementation. Avoid scaffolding:
the core should ship useful end-to-end behavior in this PR.

## Decide what becomes Tasks

For each remaining independently shippable outcome, choose one of two actions:

- If it can safely start against the current contract, create and launch it now
  when no local artifact needs staging:
  ```bash
  lf task create --wave <wave> --title "<desired experience>" <<'BRIEF'
  <short user-problem brief; durable design reference>
  BRIEF
  lf task run <issue>   # its graph: the Wave's or repository's default, else code
  ```
  Stdin files the Task description; `--directive` on `task run` supplies
  direction, not that durable brief. Use `--stack-on <current-task>` when it
  must build on this Task's branch. `task run` returns once its Graph is
  running; `lf task status <issue>` follows it, and `-b` waits for the end.
- If it depends on decisions this core has not settled, leave it in the design
  as a named follow-up. Create it after this PR settles; do not invent durable
  staging state inside Loopflow.

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
A comment should say what changed and what it means; link detailed receipts instead of pasting raw IDs, timestamps, or routine no-op logs. Chapter
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

For already designed work, select a Graph that continues that design. The
user's explicit Graph selection governs; otherwise use the Wave chapter recommendation.
Keep the design and its evidence available in each execution context before
launch; use staged preparation below when artifacts must cross contexts.

For an approved design, `--graph pursue` enters implement → compress → refresh
→ loop-or-next → concept-review → loop-or-next. Refresh runs sync → realign.
Either decider's Iterate returns to implementation; the second's Advance
publishes and ends the pursuit. The `code` graph starts with that pursuit;
`feature` drafts the design first; both land an accepted PR through `ship`.
Design review and the demo are interactive steps: `lf task run <issue>` stops
before each, the person takes part in the Task conversation, and the next
`lf task run <issue>` continues after it. `--from <step>` goes back or skips
ahead, `lf task run <issue> <graph>` takes up another graph, and
`lf -b task run <issue>` runs straight through headless. `lf --task <issue>
graph <graph>` runs any Graph without moving the Task.
Preserve intent, constraints, and done-when
proof in durable records so the Task remains useful after cleanup of added scratch files.

Finish with a short accounting of the selected work and its design/evidence
path, Tasks launched, and follow-ups intentionally deferred.

## Existing-design handoff

When the user asks to file a Task and run a Graph from an existing design,
keep the design separate. Reuse the Task if it is the same work; otherwise file
a short user-problem brief under the selected Wave, with a design reference,
its maturity, and open questions. Do not invent ownership.

```bash
lf task create --wave <wave> --title "<desired experience>" --notes "<brief; design reference and maturity>"
lf checkout <issue> --json
# Copy the selected design and required evidence into the returned worktree's scratch/.
lf task run <issue>
```

Inspect the current context first: a design already in the Task worktree needs
no transfer. For a separate source, copy the actual documents and supporting
files before launch, preserve relative references, and check their contents in
the destination. A path alone does not supply context. Preparation launches no
Graph. Do not overwrite newer destination work.

The destination becomes the working design; retain source provenance without
maintaining competing active copies. Markdown under its recursive `scratch/`
tree enters each step's context; other assets remain on disk. Preserve material
needed after scratch cleanup in durable documentation or existing records.
Do not pipe the design into Task creation: stdin becomes the Task description.

Use the Graph the user selected and inspect its contents when explaining where it
begins; otherwise use the Wave chapter recommendation. Continue the design already
present without treating its draft choices as approved. Report the Task link,
destination design path, selected Graph, and observed launch result. Verify
supplied context separately from the launch.

If implementation already exists in the source checkout, preserve it and its
writer. Document transfer does not adopt a checkout; current preparation does
not adopt an unbound existing branch/worktree. Report that gap before launching
a competing implementation. No automatic scratch-transfer flag is available.
