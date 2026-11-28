---
requires: a design or findings that may warrant implementation
produces: an execution decision and any Tasks launched
action_style: procedural
---
Turn the design into an execution decision: what gets built here, what becomes its own Task, and what waits.

Continue the caller's way of working. With no Task attached and nobody asking
for managed work, keep the chosen implementation in this checkout, keep later
ideas in the design, report the decision and finish. A Graph that includes
this skill is no reason to look up a Wave or create Tasks. For existing or
requested managed work, continue it.

## Choose the work

Decide what is worth building before allocating it. Findings may call for
prevention, more investigation, or nothing; if nothing useful remains, say so
and finish. Keep proposals apart from the scope people asked for. Reuse an
existing Task for the same outcome, and never launch a competing Graph into
active work. Before filing work against specific files, list the open PRs that
touch them (`gh pr list --json number,title,headRefName,files`) and name each
overlap in the brief.

**What stays here.** Pick the ambitious single-threaded core whose
implementation settles the interfaces for the rest. It ships useful end-to-end
behavior in this PR; no scaffolding.

**What becomes Tasks.** Each remaining outcome that ships on its own:

```bash
lf task create --wave <wave> --title "<desired experience>" <<'BRIEF'
<short problem brief; design reference>
BRIEF
lf task run <issue>
```

Stdin becomes the description. Add `--stack-on <current-task>` when it builds
on this branch. Launch now what can start safely against the current design;
leave what depends on unsettled decisions in the design as a named follow-up.

## The brief

Write it for someone who missed the conversation. Open with what a person is
trying to do, what gets in the way and what should improve; the title names
that improvement. A short paragraph and a few acceptance bullets, keeping exact
numbers and failure cases:

> After an interrupted release, maintainers need to see whether anything
> shipped and safely continue unfinished work.

Possible solutions are marked as ideas; architecture stays in the linked
design. Give follow-up Tasks outcomes useful on their own, not implementation
layers. The description holds the current problem, scope and real blockers;
dated progress goes in comments. When scope changes, rewrite the description
instead of appending.

## Launching

`lf task run <issue>` runs the Task's graph to its next breakpoint and
returns; `lf task status <issue>` follows it. `lf task auto on` follows the
workers’ recommendations through breakpoints and may merge the PR;
`lf -b task run` does this for one run, and a run with no breakpoint left
ahead may merge it too. Use them only when shipping was asked for.

To hand an existing design to a Task, the files have to travel; a path in
another checkout gives the agent nothing:

```bash
lf checkout <issue> --json     # prepares the worktree, launches nothing
# copy the design into the returned worktree's scratch/, then:
lf task run <issue>
```

Don't overwrite newer work there, and don't pipe the design into
`task create`. If the implementation already exists in another checkout, say so
before launching a competing one.

Finish with the work kept here and its design path, the Tasks launched and
what each launch printed, and the follow-ups deferred.
