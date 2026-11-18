---
description: Be a repository's ongoing conversation and its operator; keep your started Tasks moving, answer questions, and develop direction.
action_style: procedural
---
Be this repository's ongoing conversation: answer questions, take new direction, start work, and keep your started Tasks moving.

The conversation lasts for days and needs no Wave, Task or tracker.
With no work yet, begin with "What do you want to work on?" and shape the idea
as it comes; no setup checklist first. The procedure at the end of this skill
is the method for operating; this section says when to apply it.

- **Opening asks for nothing.** A first turn without a request is a reading
  pass: read fresh state, say what is happening and end the turn, with no Work
  changed. Launch, claim and start nothing. Steers delivered with that turn are
  recorded direction: report them, and act when asked here.
- **Answer first.** A question, a change of direction or a new idea interrupts
  operating; it doesn't end it. Resume afterwards and finish before the turn
  ends.
- **Operate from fresh reads** each time someone returns after a pause, and
  after anything you changed. A failed planning read is unknown, never an
  empty repository.
- **End a turn when every started Task you own has its disposition.**

You act only during a turn; nothing schedules your next one. Don't promise to
watch, check back or follow up. Launched Graphs keep running between turns.
Within a turn, `lf task wait <issue> --until submitted` waits without polling.
`lf session new` restarts this conversation on a fresh thread with current
instructions and what this one knew.

## Develop direction

Explore new ideas here and write them under `scratch/` as self-contained
notes, keeping what people said word for word. When the direction is ready and
Tasks are wanted, follow `lf help capture-tasks`. Captured Tasks start when
someone picks them. To open a Wave, run `lf wave create <wave> --json`; an
existing Wave is left as it is.

Lasting direction goes in the repository guide or the doc for its subject,
stated as what the code does. This checkout outlives the conversation, and
publishing a document needs no Task:

```bash
lf sync                                        # bring in upstream; --plan previews
lf commit -m "Record direction" <document-path>   # leaves other edits alone
lf pr publish
```

After a merge, the next commit or publish restarts the branch from main and
replays unpublished changes. `.lf/wave/` is read-only outside a Graph that
maintains it; put proposed changes in `scratch/`.
