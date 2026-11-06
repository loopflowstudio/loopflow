---
description: Clear your obsolete work state, report activity, and keep your started Tasks moving across the repository's Waves.
---
Clear your old work state, keep your started Tasks moving through their Waves, and report what is happening across the repository.

A Task is yours when its person is `lf config user` or it names no person.
Another person's Task is listed as theirs and left alone; unowned backlog is
listed for someone to pick up. This pass works existing Tasks only: plans,
targets, chapters, new Tasks and backlog are out of scope. Report gaps instead
of filing them.

## Clear old work state

### Find stale work

Read `lf wave show --json`, `lf session list --json` and `lf wt list --json`,
and join Tasks and Sessions to worktrees by the ids and paths they return. Look
at branches and PRs only to explain gaps.

Clean up only your own work. A branch, PR or worktree linked to a Task is yours
only when the Task is. Never abandon or prune another person's Task, even if you
wrote its PR. For a branch with no Task, check `gh api user --jq .login`,
`git config user.email`, PR authorship and the repository's branch naming; a tip
commit's author alone is not enough. When unsure, leave it and move on.

- Open PRs: `gh pr list --author @me --state open --limit 1000 --json number,title,headRefName,url,isDraft,mergeStateStatus,statusCheckRollup,updatedAt`.
  Paginate if it hits the limit.
- Remote branches: `git for-each-ref --format='%(refname:short) %(committerdate:iso8601) %(authoremail)' refs/remotes/origin`,
  filtered to your branch namespace. These refs are cached; confirm with
  `git ls-remote --heads origin`. Skip the default branch and symbolic refs.
- Worktrees: `git -C <path> status --short`, unpushed commits, linked Tasks and
  live Sessions or Graphs. `lf wt prune --dry-run` lists merged, squash-merged,
  closed-PR, remote-gone and inactive candidates. Inactive is not abandoned.
- Branches: compare against the default branch, look at unique commits, and
  check for commits added after a merge; a merged PR doesn't cover later work.
- Remote-only branches with no worktree, no open PR and no commits in 60 days:
  check `gh pr list --head <branch> --state all --limit 1000 --json number,state,author,url,mergedAt,headRepository`,
  matching the head repository too, and any open PR by anyone. Age alone doesn't
  make work obsolete.

### Clear it

Finish the clear cases and keep going when one is uncertain. In
`scratch/open-work.md`, keep only what the next pass couldn't read again from
git or `lf`: what people said, word for word, what a check showed at
the time, and open questions, with Task and PR links. Don't create Waves for
waveless branches.
Bring only real judgment calls to the conversation, with a recommendation and
why.

- **Ship / ship-partial:** run the matching delivery skill in the Task's
  checkout. Note leftover scope in `scratch/open-work.md`. Leave running Graphs
  and review conversations alone.
- **Abandon:** `lf task abandon <issue>` retires a Task and its PR together. For
  a branch with no Task, `lf pr abandon <branch>` closes its PR and removes its
  checkout and branches.
- **Prune:** `lf wt delete <branch>` removes the checkout, local branch and
  origin branch, keeping PR and Task history. It also takes a remote-only branch
  name (without `origin/`). Use abandon when an open PR should close.
- **Batch:** `lf wt prune` applies the whole preview; recheck it first. It
  doesn't cover the remote-only scan.
- **Database:** use Loopflow's reconcile and cleanup commands (check their
  help). Never edit SQLite directly. If a needed command is missing, note it.

Never discard dirty files, unpushed commits or live work. Use Loopflow's cleanup
commands, not raw `git branch -D` or worktree removal. Afterwards, reread the
worktree, branch, PR and Task state and report what actually happened, not what
was attempted. Clearing the whole list is never a prerequisite for the next
request.

## Summarize current activity

Report in this order: what is available to work on, what is moving, what is
stuck, other people's work left alone, then what finished, due follow-ups and
backlog. Include cleanup results and next moves. Use `lf wave list --json`,
`lf wave status <wave> --json` and Task status; check PRs and branches where
they explain a gap. Link every Task and PR. Routine progress goes in this
conversation, not Task comments; open upkeep stays in `scratch/open-work.md`.

## Drive work through Waves

Operate each Wave with started work by following `lf help wave-operate`, which
owns the ownership rule and dispositions. Do it here, or run
`lf --wave <wave> wave-operate "<concrete direction>"` and read the result.
Operate your started Tasks outside any Wave with `lf task-operate <issue>`.
Sending someone to another conversation is not operating. This view connects
outcomes across Waves; each Wave pass judges the work inside it.

Never start a competing Graph process, and never make a Task wait for this pass.
Follow each intervention to its result and include that result in the update.

## Reviewer mode

- **Interactive reviewer:** discuss judgment calls here.
- **Parent reviewer:** send recommendations through the review protocol and
  check the Task's replies. Don't touch its checkout or start competing
  execution. Name open choices in the review outcome.

## Sessions and reviews

`lf session list --available --json` shows conversations available to work in,
by `availability`; a quiet working step can appear there. `task_ids` is the Task
membership; `work` alone is not. An open or interrupted conversation doesn't
mean its Task is blocked.

`lf session list --json` lists all current conversations. Reread after changing
a Session; never trust Session state from an earlier turn or the launch prompt.
A failed read is not an empty list. Compare CLI and Desktop only on the same
Machine.

Lists cover this repository (worktrees fold into the main checkout); `--all`
shows every repository on this Machine. `lf wave list` and `lf wave show` work
the same way; `lf wave status` resolves one Wave here.

To open a Session, run `lf resume <session-id> --prepare --json` and execute the
returned `open_argv` unchanged. Preparing it doesn't mean it opened; check the
provider is ready in the terminal. A Session missing from the list may still
exist in another installation.

Questions stay in this conversation. A separate perspective on a Task is
`lf --task <task> : "<prompt>"`. Headless work missing input explains why and
stops; the Wave operator reads its logs.

## Machines

A Task belongs to a person on a Machine, or to no one. Each person picks the
Machine for new Tasks with `lf --wave <wave> set machine <machine>` or
`lf --repo <repo> set machine <machine>`, never committed; unset means the
Machine running the command. This pass changes no Task's person or Machine. Use
`lf id`, then `lf --wave <wave> wave-operate` locally or
`lf --machine <machine-id> --wave <wave> wave-operate` on the Wave's Machine.
`lf --machine` runs that Machine's `lf`. Connect missing logins in the
foreground with `lf machine connect`; work uses its own Machine's credentials.

When a Task you own is heading the wrong way, `lf task interrupt <issue>` stops
it; discuss the new direction in the Task conversation. The stopped Graph stays
as history; Task, worktree and PR are unchanged.

## Diagnose execution and auth

When work looks stuck, run `lf top` before guessing (`lf ps --json` for a
snapshot). It shows live call trees, output rates, tokens, age, idle time,
health and PIDs. Time alone never means dead. `lf mon prune --dry-run` shows
cleanup candidates; plain prune removes dead records and orphaned provider
groups. Never kill an `unclaimed` PID.

Check `lf account --cached` (offline) before proposing an account fix;
`lf account --json` shows managed status. OAuth client credentials come from the
environment, then Doppler; in a Doppler repository, give
`doppler run -- lf account connect <provider>`, otherwise name the missing
variables. Never print values or run a secret getter bare. Don't change accounts
or Task ownership just to get a pass through.
