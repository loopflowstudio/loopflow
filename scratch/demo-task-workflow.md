# Demo: a Task's workflow from one conversation — prepared October 6, 2026

LOO-353, PR #1439 at `ac91bf9e1`. Prepared by the agent after one looping
`pursue` Flow built the eight slices in
[the design](focus-on-your-own-work.md). **Jack Heart has not run this yet;
his observations go under "Jack's review" below.** Slices 9–12 (`lf task
move`, state from position, the drawn graph, the Task page fed by the
workspace stream) postdate this rehearsal and were not rehearsed. Code walkthrough:
[pr-review.html](pr-review.html). Unreviewed choices:
[questions.md](questions.md). The October 5 review is at
`42c31408a:scratch/demo-task-flows.md`.

## Try it: command line

A private Home with one fixture Task, `INF-123`, whose Project default is the
`feature` workflow. `task-design`, `pursue` and `ship` are provider-free
stand-ins; `pursue` has a 40-second step. No provider, no Linear, and the live
Home is untouched.

```bash
S=/private/tmp/claude-501/-Users-jack-src-loopflow-focus-on-your-own-work/e6fd2a52-4d15-4f72-88aa-1c93939b3482/scratchpad
alias dlf=$S/dlf; cd $(cat $S/home/repo-path)
dlf task status INF-123                  # where the Task is
dlf -b task run INF-123 pursue &         # run an edge; then, within 40 seconds:
dlf task status INF-123                  # "running pursue (demo → demo)"
dlf flow list --sessions --for-task INF-123
dlf -b task run INF-123 ship             # a refusal or a move, by position
dlf -b task run INF-123 notes            # take up a workflow with no PR
dlf -b task run INF-123 end              # end it
dlf flow list                            # workflows beside Flows
dlf session list --waiting
```

The agent's rehearsal left the Task at `end` of `notes`;
`dlf -b task run INF-123 feature` starts `feature` again.

## Try it: Desktop

`uv run python scripts/loopflow-dev.py run` from the Task worktree builds and
opens the app (the build succeeded on October 6). Not rehearsed: the agent has
no display, and the branch's `lf` cannot open the live Home until the draft
migration is applied to it. What to look at since slice 14: a Task opening on its
one Session with its Workflow in the header; **+ → Flow execs** and **Files**
as panes; the Wave page's Workflow menu.

## What the agent saw (rehearsal, not Jack's observation)

Every command above behaved as its comment says; transcripts are in the
walkthrough. Not shown: any real provider step; a driver killed during an
agent step; the migration on a populated Home; the September 30 workspace
proof items.

## Jack's review

October 6, Desktop (dev app, private Home). First build at `4f2860658`:
"this is kinda yucky", then the layout recorded as slice 14 in
[the design](focus-on-your-own-work.md). Rebuild at `975d9f727`, after
looking at it running: "good enough. approved. make whatever cleanups yyou
want". The approval is of the Task view as demonstrated on the fixture Task;
it does not cover landing, the unmerged main, a real provider or the
migration.
