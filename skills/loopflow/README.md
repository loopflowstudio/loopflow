# The Loopflow skill

```bash
npx skills add loopflowstudio/loopflow --skill loopflow -g
```

The installer shows its security assessments and asks before installing.
PromptScript reads skills only per project: inside a PromptScript project, run
the same command without `-g`.

Teaches the coding agent you already use (Claude Code, Codex, OpenCode, Pi,
Antigravity) to work the Loopflow way and drive `lf`. You keep talking to your
agent; it runs focused subagents here, hands PR-sized work off as Tasks, keeps
track of runs, and brings back pull requests.

## Run a one-off subagent here

> *"Ask Codex to audit this branch for race conditions without editing anything."*

```bash
lf -b -a codex research 'audit this branch for race conditions; edit nothing'
```

`lf -b` runs headless in the current checkout on any harness or model, records
a Session, prints the result, and exits — no new worktree or PR required.

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

A change that should end in a PR becomes a Task. A question, a quick look, or a
small fix happens right here, in your checkout.

## Ask where things stand

> *"What's running? Anything waiting on me?"*

```bash
lf task list                  # open Tasks, with what each is doing
lf task status LOO-41 --json  # one Task: its run, its PR, what it needs
```

## Steer it while it runs

> *"Tell the token one to keep the retry budget at 3."*

```bash
lf steer LOO-41 'keep the retry budget at 3'          # reaches running Sessions on the Task
```

## Get back into it

> *"Open the token Task's conversation."*

```bash
lf resume --task LOO-41
```

## Land it

> *"Land LOO-41 when checks pass."*

```bash
lf pr land --wait-and-fix     # merges when checks pass; repairs them if not
```

## Run your Skills and Graphs

> *"Run the audit skill on this branch."* · *"Ship this through ship-api."*

```bash
lf audit
lf -b task run LOO-41 ship-api
```

Your agent discovers commands, Skills, and Graphs with `lf list` and
`lf help <name>`, and reads full embedded guides offline with `lf docs <topic>`.

## What it won't do

- Start Tasks for work you asked it to do here, or for a question.
- Pick a different agent, account, or machine than you asked for.
- Run more than one Graph on the same Task.
- Merge anything you didn't ask it to land.

## Inside a Task

Agents that `lf` launches carry [LOOPFLOW.md](../../rust/loopflow-definitions/src/builtins/LOOPFLOW.md)
automatically — the same `How to work` principles and `lf` mental model, scoped
to working inside that Session and Task checkout.
