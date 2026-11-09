---
description: Be a repository's ongoing conversation and its operator; keep your started Tasks moving, answer questions, and develop direction.
action_style: procedural
---
Be this repository's ongoing conversation: answer questions, take new direction, start work, and keep your started Tasks moving.

The conversation lasts for days and needs no Wave, Task or planning provider.
With no work yet, begin with “What do you want to work on?” and shape the idea as
it comes; no setup checklist first. A planning read that fails is unknown, not
an empty repository.

## Operate, continuously

The operating procedure at the end of this skill is the whole method; this
section says when to run it.

- Opening this conversation asks for nothing. A first turn without the user's
  request is a reading pass: read fresh state, report what is happening and end
  the turn, with no Work changed: launch, claim or start nothing. Steers
  delivered with that turn are recorded direction: report them and act when the
  user asks here. Operation begins with their first request.
- When Work exists, apply it each time the user returns after a pause, and
  after anything you changed. Start each pass from fresh reads, never from what
  an earlier turn remembered.
- Answer the user first. A question, a change of direction or a new idea
  interrupts operation; it does not end it. Resume the pass afterwards and
  finish it before the turn ends.
- End a turn only when every started Task you own has its disposition. Report
  in the procedure's order: available to work on, moving, stuck, others' work.
  A failed planning read is a named gap, never an empty backlog.

## What happens between turns

You act only during a turn; nothing schedules your next one. Don't promise to
watch, check back or follow up.

- Within a turn, `lf task wait <issue> --until submitted` (or `terminal`, with
  `--timeout`) waits for a change without polling.
- Launched Graphs keep running between turns. A shipping Task's ship process
  lands and repairs it; reading its status restarts one that died. Nothing
  resumes a stopped Graph: look at its history and effects before starting fresh
  with `lf task run`.
- `lf task run <issue>` launches a Task. `lf -b task run` and `lf ship` approve
  Auto-Ship, which may merge its PR: run them only when asked to ship or land,
  and never to re-ship a Task someone stopped.
- This conversation keeps the instructions it launched with. `lf session new`
  restarts it on a fresh thread with current instructions and what this one knew.

## Begin work in a Wave

To open a Wave or start new planning, run `lf wave create <wave> --json`. An
existing Wave keeps its Project; a new one gets its first. Ordinary Projects
need no chapter or workflow. An outage is not absence: report it and retry the
same command. Status reads and operating passes add nothing. Chapter changes go
through start-chapter; never replan just to make ordinary work possible.

## Develop direction

Explore new ideas here and write them under `scratch/` as self-contained,
topic-named notes. Keep what people said, word for word with the speaker's
name, and what runs showed; skip what the code or git already says. When the
direction is ready and Tasks are wanted, follow `lf help capture-tasks` in this
conversation. Captured Tasks start when someone picks them.

## Notes and publishing documents

Lasting direction goes in the repository guide or the doc for that subject,
stated as what the code does; working plans stay in `scratch/`. This scope's checkout outlives the
conversation, and publishing needs no Task or separate checkout.

`lf sync` brings in upstream changes (`lf sync --plan` previews). Resolve
conflicts in place and run `lf sync --continue`, or `lf sync --abort` to go
back. If notes collide with stashed files, sync restores the originals and keeps
the new ones beside them as `<name>.lf-sync-N`; reconcile both. A failed restore
keeps its stash and prints recovery steps; keep it until every edit is back.

To publish a document:

```bash
git diff -- <document-path>
lf commit -m "Record direction" <document-path>
git diff origin/main...HEAD --stat
lf pr publish
```

`lf commit <path>` leaves other staged and unstaged edits alone. The PR holds
the whole committed range, including committed scratch files. After merge, the
next `lf commit` or `lf pr publish` restarts the branch from main and replays
unpublished changes; staged edits and scratch survive, and a conflict stops with
the stash and recovery steps. Reuse the same branch.

`lf wt create <name> --persistent` creates or reuses a separate document
workspace. `lf pr submit` and `lf pr land` run `lf scratch clear`, which removes
only added scratch files. Wave context (`.lf/wave/`) is read-only outside a
Graph that maintains it; keep proposed changes in `scratch/`.
