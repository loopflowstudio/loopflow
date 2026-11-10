---
requires: a design or findings that may warrant implementation
produces: an execution decision and any Tasks launched
action_style: procedural
---
Turn the design into an execution decision. Continue the caller's way of
working; use Task commands for existing Tasks or when managed work is asked for.
Do not create a manifest, receipt, marker, or new planning state.

## Without a Task

When no Task is attached and nobody asked for managed work, keep the chosen
implementation in this checkout. Follow the caller's Graph or go straight to
implementation; keep later ideas in the design. Report the decision and finish.
A Graph like `incident` including this skill is no reason to look up a Wave or
create Tasks.

For existing or requested managed work, read `lf help skill work` and continue.

## Choose the work

Read the design or findings, `AGENTS.md`, and any named Task's current state.
Read Wave chapter state only when the seed names it.

Decide what's worth building before allocating it. Incident findings may call
for prevention, more investigation, or nothing. Keep proposals apart from
accepted scope. Reuse an existing Task for the same outcome. Before filing work
against specific files, list open PRs touching them
(`gh pr list --json number,title,headRefName,files`) and name each overlapping
PR, its Task, and any choice it leaves to a person in the new brief. If nothing
useful remains, say so and finish. Never launch a competing Graph into active
work.

## Decide what stays here

Pick the ambitious single-threaded core whose implementation settles the
interfaces for the rest. Keep that core in this Task and describe where it ends
clearly enough to implement. No scaffolding: the core ships useful end-to-end
behavior in this PR.

## Decide what becomes Tasks

For each remaining independently shippable outcome:

- If it can start safely against the current design, create and launch it now:
  ```bash
  lf task create --wave <wave> --title "<desired experience>" <<'BRIEF'
  <short problem brief; design reference>
  BRIEF
  lf task run <issue>   # its graph: the Wave's or repository's default, else code
  ```
  Stdin becomes the Task description; `--directive` on `task run` gives
  direction, not the brief. Add `--stack-on <current-task>` when it builds on
  this Task's branch. `task run` returns once the Graph is running;
  `lf task status <issue>` follows it.
- If it depends on decisions this core hasn't settled, leave it in the design
  as a named follow-up and create it after this PR lands.

### Writing the Task

Someone who missed the planning conversation should understand it. Open with
what the person is trying to do, what gets in the way, and what should improve.
The title names that improvement. Use concrete verbs, not process phrases like
"establish the bounded outcome."

One or two short paragraphs and a few acceptance bullets usually do. State what
success looks like, with the numbers, windows, failure cases and constraints it
needs. Technical Tasks can serve maintainers or operators. Mark possible
solutions as tentative; architecture and implementation steps stay in the
design, linked.

Keep the description current. Dated progress, chapter moves, queue changes,
launch attempts and verification go in Task comments: what changed and what it
means, with links instead of raw IDs, timestamps or no-op logs. Comments may be
collapsed, so blockers, real dependencies and accepted scope that affect the
work belong in the description. A queue position isn't a dependency. When a
comment changes scope, rewrite the description; don't append dated amendments.

Describe "After an interrupted release, maintainers need to see whether
anything shipped and safely continue unfinished work," with acceptance that a
retry never publishes twice and failures stay visible. "Moved into the
September chapter; scheduled after the installation repair" is a comment. Give
follow-up Tasks outcomes useful on their own, not implementation layers.

Link related work where you explain why it matters: `[Title · Task ID or PR
number](URL)` on first mention, shorter after, with the relationship (builds
on, supersedes, verified by). In lists, put the ID first: `[Task ID or PR number
· Title](URL)`.

### Choosing the Graph

For designed work, pick a Graph that continues the design: the one asked for,
else the Wave chapter's recommendation.

`--graph pursue` runs write-docs → implement → compress → refresh → loop-or-next
→ concept-review → loop-or-next. Refresh is sync → realign. Either decider's
Iterate goes back to implement; the second's Advance publishes and ends the
pursuit. `code` starts with that pursuit; `feature` drafts the design first;
both land an accepted PR through `ship`. Design review and the demo are
interactive steps: `lf task run <issue>` stops before each, the person takes
part in the Task conversation, and the next `lf task run <issue>` continues
after it. `--from <step>` goes back or skips ahead; `lf task run <issue>
<graph>` takes up another graph. `lf --task <issue> graph <graph>` runs any
Graph without moving the Task. `lf -b task run` and `lf ship` approve
Auto-Ship and run headless to `end`, which may merge the PR, and so does `lf
task run` with no interactive step left ahead: use them only when shipping or
landing was asked for.

Put intent, constraints and "Done when" somewhere that outlives scratch
cleanup.

Finish with a short account: the work kept here and its design path, Tasks
launched, follow-ups deferred.

## Existing-design handoff

To file a Task and run a Graph from an existing design, reuse the Task if it's
the same work; otherwise file a short problem brief under the chosen Wave with a
design reference, how mature the design is, and open questions.

```bash
lf task create --wave <wave> --title "<desired experience>" --notes "<brief; design reference and maturity>"
lf checkout <issue> --json
# Copy the design and its supporting files into the returned worktree's scratch/.
lf task run <issue>
```

A design already in the Task worktree needs no copy. Otherwise copy the actual
files before launch, keep relative references working, and check they arrived;
a path alone gives the agent nothing. `lf checkout` launches no Graph. Don't
overwrite newer work at the destination, and don't pipe the design into
`task create`: stdin becomes the description.

The copy becomes the working design; note where it came from and stop editing
the original. Markdown anywhere under `scratch/` enters each step's context;
other files stay on disk. Anything needed after scratch cleanup goes in docs or
the Task.

Use the Graph the user selected and read it before explaining where it begins;
otherwise use the Wave chapter's recommendation. Continuing a draft doesn't
accept its choices. Report the Task link, design path, Graph, and what the
launch printed; check the copied context separately.

If implementation already exists in the source checkout, leave it with whoever
is writing it. Copying documents doesn't adopt a checkout or an unbound
branch; say so before launching a competing implementation. There's no
automatic scratch-transfer flag.
