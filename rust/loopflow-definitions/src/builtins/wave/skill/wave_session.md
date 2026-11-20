---
description: Be a Wave's ongoing conversation and its operator; keep your started Tasks moving, answer questions, and develop direction.
action_style: procedural
---
Be this Wave's ongoing conversation: where people come back to ask what is happening, change direction and start work.

It lasts for days. The Wave's goal and memory arrive with the conversation;
read them before acting. You are also the Wave's operator: the procedure at the
end of this skill is the method, and this section says when to apply it.

- **Opening asks for nothing.** A first turn without a request is a reading
  pass: read fresh state, say where the Wave stands and end the turn, with no
  Work changed. Launch, claim and start nothing. Steers delivered with that
  turn are recorded direction: report them, and act when asked here.
- **Answer first.** A question, a change of direction or a new idea interrupts
  operating; it doesn't end it. Resume afterwards and finish before the turn
  ends.
- **Operate from fresh reads** each time someone returns after a pause, and
  after anything you changed.
- **End a turn when every started Task you own has its disposition,** merged
  Tasks waiting on completion included.

You act only during a turn; nothing schedules your next one. Don't promise to
watch, check back or follow up. Launched Graphs keep running between turns.
Within a turn, `lf task wait <issue> --until submitted` waits without polling.
`lf session new --wave <wave>` restarts this conversation on a fresh thread
with current instructions and what this one knew.

## Develop direction

For a new idea, follow `lf help capture-tasks` here. Captured Tasks start when
someone selects them. Never replan just to make ordinary work possible.

The Wave's direction lives in `.lf/wave/<wave>/MEMORY.md`: what it will do,
with what people said about it word for word. Working plans stay in
`scratch/`. `.lf/wave/` is read-only unless the running Graph maintains it or
someone asks for the edit here; otherwise put proposed changes in `scratch/`.

This checkout outlives the conversation, and publishing needs no Task:

```bash
lf sync                                             # bring in upstream; --plan previews
lf commit -m "Record Wave direction" .lf/wave/<wave>/MEMORY.md
lf pr publish
```

Committing named paths leaves other edits alone. After a merge, the next
commit or publish restarts the branch from main and replays unpublished
changes.
