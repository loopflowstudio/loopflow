# The Loopflow skill

```bash
npx skills add loopflowstudio/loopflow --skill loopflow -g
```

The installer shows its security assessments and asks before installing.
PromptScript reads skills only per project: inside a PromptScript project, run
the same command without `-g`.

Teaches the coding agent you already use (Claude Code, Codex, OpenCode, Pi) to
work through `lf`. You keep talking to your agent; it hands work off, keeps track
of it and brings back pull requests.

## Ask for work, get pull requests

> *"Split the auth cleanup into Tasks and run them with lf."*

Your agent:

```bash
lf task create 'validate session tokens on refresh'   # one Task per outcome
lf task create 'delete the legacy cookie path'
lf task run LOO-41                                    # each in its own worktree and branch;
lf task run LOO-42                                    # returns once it's running
```

and comes back with a link per Task. Each ends as its own pull request, so you
review them one at a time while your agent stays free to talk.

A change that should end in a PR becomes a Task. A question, a quick look or a
one-line fix happens right here, in your checkout.

## Ask where things stand

> *"What's running? Anything waiting on me?"*

```bash
lf task list                  # open Tasks, with what each is doing
lf task status LOO-41         # one Task: its run, its PR, what it needs
```

## Steer it while it runs

> *"Tell the token one to keep the retry budget at 3."*

```bash
lf comment LOO-41 'keep the retry budget at 3'   # reaches the run before its next step
```

## Get back into it

> *"Open the token Task's conversation."*

```bash
lf resume --task LOO-41
```

## Land it

> *"Land LOO-41 when checks pass."*

```bash
lf land --wait-and-fix        # merges when checks pass; repairs them if not
```

## Run your Skills and Graphs

> *"Run the audit skill on this branch."* · *"Ship this through ship-api."*

```bash
lf audit
lf -b task run LOO-41 ship-api
```

Your agent finds what exists with `lf list` and reads any command, Skill or Graph
with `lf help <name>` before using it.

## What it won't do

- Start Tasks for work you asked it to do here, or for a question.
- Pick a different agent, account or machine than you asked for.
- Run more than one agent on the same Task.
- Merge anything you didn't ask it to land.

## Inside a Task

Agents that `lf` launches for a Task get the same guidance from Loopflow itself:
do that Task's work in its checkout, follow its Graph, and report back. They
don't start new Tasks unless the step asks for it.
