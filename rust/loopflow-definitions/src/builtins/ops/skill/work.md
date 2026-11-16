---
description: Use Loopflow's Task, Wave and Project commands inside whatever workflow is already running.
---
# Working with Tasks and Waves

Use Tasks, Waves and Projects when the request calls for them or continues an existing Task. The request, Skill or Graph decides what to do; this skill explains the commands. Reading it creates nothing.

## The model

- **Wave**: a lasting objective, its memory and its area of responsibility.
- **Project**: a Wave's plan: Tasks, KRs and targets.
- **Task**: one outcome in one repository, optionally in a Wave, with its
  checkout, conversations, current PR and delivery history.
- **Graph**: steps, some `interactive:` where a person takes part. A Task takes
  up one and stands at a step of it.
- **Session**: a conversation. **Process**: one running `lf`.

Continue the selected Task or this checkout's Task, keeping its checkout, PR
and live work. A worktree, a PR or a Graph is no reason to create a Task;
create one when asked. Change worktrees, delivery and Work state through `lf`;
read, edit and test normally, and checkpoint with `lf commit`.

## Read, then act

Read `lf task status <issue> --json`, `lf wave status <wave> --json`,
`lf wave list --json` or `lf wave show --json`, not processes or worktrees. A
failed read is unknown, not an empty plan: name the missing service, carry on
with other work, and don't repair auth unasked.

```bash
lf task create --title "<outcome>"        # file new work; --run starts it
lf task run <issue>                       # run its graph to the next interactive step
lf task run <issue> --from <step>         # start elsewhere
lf task move <issue> --wave <wave>        # change membership, keep the work
lf session bind <id> --task <task>        # make this conversation the Task's
lf task complete <issue>                  # the outcome is delivered
```

Before starting more execution, look at the Task's live work. Leave a live
Graph running. A stopped Graph is history: fix the cause and run the steps it
didn't reach. Review feedback, a linked Task or an idle process is no reason to
launch. Publishing, merging and completing a Task are three separate facts; a
finished Graph or a published PR doesn't finish the Task.

For ongoing supervision use `task-operate`, `wave-operate` or `repo-operate`.
Operators move your Tasks and leave other people's alone. Contributing to a
Task doesn't mean operating it.

## Child agents

A child takes one piece of your work: a review, a search, a second opinion on
another model. `lf -b <skill> '<brief>'` runs one headless in this checkout,
on any harness or model (`-a codex`), recorded and counted toward the Task.
Your harness's own subagent is fine too. Work that ends in its own PR is a
Task: `lf task create --run`.

Look for a Skill or Graph that fits before briefing from scratch (`lf list`,
`lf help <name>`). When the same child job keeps coming up, write it as a
skill under `.lf/skills/`.

You answer for what a child returns; check it before you use it. The brief
says what to do, what done looks like and where the result goes. Redirect a
running child with `lf steer <its Session ID> '<direction>'`; steer a Task
with `lf steer <ID>`. Wait for a child's exit or `lf task wait <ID>`, never in
a sleep loop. A child does the work itself and starts no children unless its
brief says to.

## Notes

`scratch/` holds plans and notes; its Markdown enters context automatically.
`lf pr submit` and `lf pr land` clear the files a branch added there, so don't
clear it by hand. Wave context (`.lf/wave/`) is read-only unless the running
Graph maintains it; put proposed memory changes in `scratch/`. Never guess a
Wave to fill a template.

A Task's description holds the current problem, outcome and constraints; new
direction goes in comments, and routine progress stays in the conversation.
An open conversation doesn't schedule another turn.
