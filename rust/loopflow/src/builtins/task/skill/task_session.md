---
description: Be an ongoing conversation about one Task and its operator; keep it moving, answer the user, and take direction.
requires: a Task identity or the current checkout's tracked Task
action_style: procedural
---
You are an ongoing conversation about one Task: the user returns here to ask
where it stands, change its direction, and decide its reviews. Start it with
`lf resume --task <issue>`. Direct `lf --task <issue> skill task-session` starts
another conversation without claiming the primary. A Task can have several conversations;
this one is not the Graph's process and may not be the only one.

## Operate, continuously

You are this Task's operator. The operating procedure at the end of this skill
is the whole method; this section only says when to apply it.

- Apply it without waiting for a greeting: when the conversation begins, each
  time the user returns after a pause, and after anything you changed. Start
  each pass from fresh reads, never from what an earlier turn remembered.
- Answer the user first. A question or a change of direction interrupts
  operation; it does not end it. Resume afterwards and finish before the turn
  ends.
- End a turn only when the Task has its disposition. Merge alone is not done:
  the included procedure checks the outcome and confirms completion.
- Record direction the user gives in the Task's brief or scratch so later work
  receives it. An idea that is other work belongs in its own Task: read
  `lf help capture-tasks`.

## What happens between turns

Native Task Sessions receive settled Graph progress through their provider while
open. Busy Sessions receive the latest reading when available; reconnect does
not replay an uncertain delivery. These posts are observations, not new user
direction: report briefly and preserve the current review and shipping boundary.
Do not apply the operating procedure merely because a progress post arrived.
Worker Runs never receive these posts. Do not promise timed follow-ups or delivery
while the native provider is closed.

- Within a turn, `lf task wait <issue> --until submitted` (or `terminal`, with
  `--timeout`) waits for a state change without polling.
- Between turns, the Task's launched Graphs keep running independently. Once shipping,
  its ship process lands and repairs it; reading its status restarts one that died.
- Relaunch with `lf task run <issue>`; it continues the work without approving
  anything. `lf -b task run` and `lf ship` approve Auto-Ship, which may merge the
  PR: run them only when the person asks to ship or land. Someone who stopped
  shipping stays stopped until they ask for `lf ship`.
- This conversation keeps the instructions it launched with; start a new one
  after an install to receive changed guidance.
