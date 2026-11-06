---
description: Use Loopflow's Task, Wave and Project commands inside whatever workflow is already running.
---
# Working with Tasks and Waves

Use Tasks, Waves and Projects when the request calls for them or continues an existing Task. The request, Skill or Graph decides what to do; this skill only explains the commands. Reading it creates nothing.

## Work model

- **Wave**: a lasting objective, its memory and its area of responsibility.
- **Project**: a Wave's plan: Tasks, KRs and targets.
- **Task**: one outcome in one repository, optionally in a Wave, with its
  checkout, conversations, current PR and delivery history.
- **Graph**: steps, some `interactive:` where a person takes part. A Task takes
  up one and stands at a step of it; a run executes it, with or without a Task.
- **Session**: a conversation. **Process**: one actual lf process.

Continue the selected Task or the current checkout's Task, keeping its checkout,
PR and live work. Other Tasks in the repository don't select work here. Don't
create a Task just because a worktree, PR or Graph exists; create one when asked
or when the planning procedure calls for it.

`lf task create --title "<outcome>"` files new work wherever you are. `--run`
starts it; it never picks up existing work. To keep the current conversation,
pass `--session <id>` from `LF_AGENT_CALLER.session_id` or
`lf session list --json`. `--worktree <path>` adopts that linked checkout.

`lf session bind <id> --task <task>` permanently makes an unfinished interactive
Session the Task's primary conversation (closed Sessions and headless Runs
can't be). It moves the Session's directory to the Task's checkout, or adopts the
Session's checkout if the Task has none; files stay put. Binding to a done Task
doesn't reopen it. Binding keeps files, branch and history, starts no Graph and
approves no Auto-Ship. A repository or Wave conversation bound to a Task frees
that scope to open a new one. Remote sources must already be on that Machine.

Change managed worktrees, Task delivery and Work state through `lf`; inspect,
edit and test normally. Checkpoint with `lf commit`.

## Inspect and act

Read `lf task status <issue> --json`, `lf wave status <wave> --json`,
`lf wave list --json` or `lf wave show --json`, not processes or worktrees. A
failed read is unknown, not an empty plan: name the missing service, carry on
with other work, and don't repair auth unasked.

Before starting more execution, look at the Task's live work and earlier
results. Leave a live Graph running. A stopped Graph is history: fix the cause
and run the steps it didn't reach without repeating their effects.

`lf task run` takes up the Task's graph if it has none and runs to the next
interactive step; `--from <step>` starts elsewhere. `lf task move --wave/--no-wave`
changes membership and keeps the work. Review feedback, a linked Task or an idle
process is not a reason to launch. A finished Graph or published PR doesn't
finish the Task; complete it when the accepted outcome is delivered.

For ongoing supervision, pick `task-operate`, `wave-operate` or `repo-operate`,
or the matching Session skill. Operators move your Tasks and leave other
people's Tasks and backlog alone. Contributing to a Task doesn't mean operating
it.

## Delivery and notes

Use the delivery skill for publishing, review handoff or landing. Publishing,
merging and completing a Task are separate. Put extra outcomes in their own
linked Tasks. Publishing again in the same Task makes a new current PR and keeps
the old ones in history. Plain PR delivery needs no Task.

Read the brief, the accepted design and the Wave memory. Wave context
(`.lf/wave/`) is read-only unless the running Graph maintains it; keep proposed
memory changes in `scratch/`, and never start a Graph just to write there. Never
guess a Wave to fill a template.

Use `scratch/` for plans and notes; its Markdown enters context automatically.
Write down what a later agent couldn't recover from the code, git or a fresh
run: what people said, word for word, what a run showed,
outside facts. `lf pr submit` and `lf pr land` run `lf scratch clear`, which
removes added files and keeps existing ones; don't clear it by hand.

Routine progress stays in this conversation. Task descriptions hold the current
problem, outcome and constraints; new direction goes in Task comments. Keep
`<!-- loopflow-progress:... -->` markers on published progress; `lf comment --steer`
is only for new direction. An open conversation doesn't schedule another turn;
Graphs and installed schedules run on their own.
