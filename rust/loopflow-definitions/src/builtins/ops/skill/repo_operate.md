---
description: Clear your obsolete work state, report activity, and keep your started Tasks moving across the repository's Waves.
---
Clear your old work state, keep your started Tasks moving, and report what is happening across the repository.

A Task is yours when its person is `lf config user` or it names no person.
Another person's Task is listed as theirs and left alone; unowned backlog is
listed for someone to pick up. This pass works existing Tasks only. Plans,
targets, projects and new Tasks are out of scope: report gaps, don't file them.

## Clear old work

Read `lf wave show --json`, `lf session list --json` and `lf wt list --json`,
and join Tasks and Sessions to worktrees by the ids and paths they return.
Leave paused Tasks, their branches and their checkouts alone during cleanup and operation.
Then look for what has no future:

- Open PRs of yours: `gh pr list --author @me --state open --json number,title,headRefName,url,isDraft,mergeStateStatus,updatedAt`.
- Worktrees: `lf wt prune --dry-run` lists merged, closed-PR, remote-gone and
  inactive candidates. Inactive is not abandoned: check for dirty files,
  unpushed commits, linked Tasks and live Sessions.
- Branches: a merged PR doesn't cover commits added after the merge.
- Remote-only branches with no worktree, no open PR and no commits in 60 days.
  Age alone doesn't make work obsolete.

Clean up only what is yours. A branch, PR or worktree linked to a Task is
yours only when the Task is; for a branch with no Task, check PR authorship
and the branch namespace, not just the tip commit's author. Unsure means leave
it. Finish the clear cases and keep going past an uncertain one:

- **Ship it:** run the delivery skill in the Task's checkout.
- **Abandon it:** `lf task abandon <issue>` retires a Task and its PR;
  `lf pr abandon <branch>` does the same for a branch with no Task.
- **Prune it:** `lf wt delete <branch>` removes the checkout and its local and
  origin branch, keeping PR and Task history. `lf wt prune` applies the whole
  preview; recheck it first.

Never discard dirty files, unpushed commits or live work, and never use raw
`git branch -D`, worktree removal or SQLite edits. Afterwards reread the state
and report what happened, not what was attempted. Bring only real judgment
calls to the conversation, each with a recommendation. Keep in
`scratch/open-work.md` only what the next pass couldn't read again from git or
`lf`. Clearing the whole list is never a prerequisite for the next request.

## Keep work moving

Operate each Wave that has started work by following `lf help wave-operate`,
here or with `lf --wave <wave> wave-operate "<direction>"`, and operate your
started Tasks outside any Wave with `lf task-operate <issue>`. Sending someone
to another conversation is not operating. Follow each intervention to its
result. Never start a competing Graph run.

`lf task run <issue>` continues a Task to its next interactive step. `lf task
auto on` turns auto on, so the run stands in at interactive steps and may
merge the PR; `lf -b task run` stands in for its own run, and a run with no
interactive step left ahead may merge it too. Use them only when asked to ship or land, and never turn auto back on
for a Task someone turned it off for. When a Task of yours is heading the wrong way,
`lf task interrupt <issue>` stops it; discuss the new direction in its
conversation.

## Report

In this order: what is available to work on, what is moving, what is stuck,
other people's work left alone, then what finished and the backlog. Link every
Task and PR. Routine progress goes in this conversation, not Task comments.

## Reading the Machine

- **Sessions.** `lf session list --available --json` shows conversations
  waiting for input; an open conversation doesn't mean its Task is blocked.
  Reread after changing anything, and never trust Session state from an
  earlier turn. A failed read is not an empty list. To open one, run
  `lf resume <session-id> --prepare --json` and execute its `open_argv`
  unchanged.
- **Execution.** When work looks stuck, run `lf session list --json` and
  `lf graph list --runs --state current --json`: what runs, confirmed
  against the OS. Time alone
  never means dead. `lf machine doctor` reports strays and kills nothing.
  Never kill a PID you can't account for.
- **Machines.** A Task belongs to a person on a Machine. Each person picks
  where new Tasks run with `lf --wave <wave> set machine <machine>` or
  `lf --repo <repo> set machine <machine>`, never committed. This pass changes
  no Task's person or Machine; operate a remote Task on its own Machine with
  `lf --machine <machine-id>`.
- **Accounts.** Check `lf account --json` (saved service authentication,
  offline) before proposing an account fix. Claude and Codex own native
  sign-in. Never print credential values, and don't change accounts or
  ownership just to get a pass through.
