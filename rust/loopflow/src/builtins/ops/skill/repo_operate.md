---
description: Clear your obsolete work state, report activity, and keep your started Tasks moving across the repository's Waves.
---
Keep the repository's work state clean and your work moving. You are this
repository's operator for your own work: clear old state that is no longer
needed, keep your started Tasks moving through their Waves, and summarize
current activity. A Task is yours when its owner's person is `lf config user`
or names no person; another person's Task is listed as theirs and left alone,
and unowned backlog is listed for the person to select. The pass works existing
Tasks. It leaves plans, their key results and metric targets, chapter rotation,
new Tasks and backlog to the person; report gaps it finds instead of filing them.

## Clear old work state

### Find stale work

Read `lf wave show --json`, `lf session list --json`, and `lf wt list --json`.
Join Task and Session state to worktrees using Loopflow's returned identities
and paths. Inspect branches and PRs to explain gaps in that shared picture.

Scope cleanup to the participant's own work. A branch, PR or worktree linked
to a Task is in scope only when that Task is yours. Never abandon or prune
another person's Task, even when you authored its PR. For branches with no
Task, read `gh api user --jq .login` and `git config user.email`, then check PR
authorship and the repository's configured branch naming convention. A tip
commit's author alone does not establish branch ownership. Leave uncertain
ownership unresolved and continue with the rest; do not triage teammates' PRs
or delete their branches.

Use these checks where they help establish what can be cleared:

- Open PRs: `gh pr list --author @me --state open --limit 1000 --json number,title,headRefName,url,isDraft,mergeStateStatus,statusCheckRollup,updatedAt`.
  Paginate if the result reaches the limit; missing or incomplete data stays unknown.
- Remote branch candidates: `git for-each-ref --format='%(refname:short) %(committerdate:iso8601) %(authoremail)' refs/remotes/origin`.
  Filter to the participant's branch namespace and corroborate ownership with
  PR history. These are cached refs; check `git ls-remote --heads origin`
  before treating a branch as present or gone. Exclude the default branch
  and symbolic refs.
- Worktree condition: inspect `git -C <path> status --short`, unpushed commits,
  associated Tasks, and active Sessions/Graphs. Run `lf wt prune --dry-run` to
  identify merged, squash-merged, closed-PR, remote-gone, terminal, and inactive
  cleanup candidates. Review the preview; inactivity alone is not abandonment.
- Branch disposition: compare ahead/behind the repository's default branch,
  inspect unique commits, and check PR history for merged or squash-merged work.
  Check for commits added after a merge; a merged PR does not settle later work.
- Remote-only stale candidates: no worktree, no open PR, and no commits in
  60 days. Check each candidate with
  `gh pr list --head <branch> --state all --limit 1000 --json number,state,author,url,mergedAt,headRepository`.
  Match the head repository as well as the branch name. Check open PRs regardless
  of author before deletion. Age alone does not establish that work is obsolete.

### Clear what can be cleared

Follow the evidence to useful actions. Combine related cleanup, finish clear
cases, and keep moving when one item is uncertain. Use `scratch/open-work.md`
for findings worth retaining: affected branches or paths, Task/PR links,
recommendations, supporting evidence, and unresolved work. Choose a format
that fits the findings; a table is useful for many items, not a required agenda.
Do not create Waves for waveless branches.

Decide whether work should ship, ship partially, be abandoned, or be pruned.
Investigate uncertainties that can be answered from available evidence. Bring
only consequential choices requiring the participant's judgment to the
conversation, with a recommendation and its reason.
An unresolved item need not hold up the rest of the upkeep.

Use the appropriate operation:

- **Ship / ship-partial:** use the delivery skill appropriate to the requested
  outcome in the owning checkout. Record useful unfinished scope in the punch
  list. Keep running Graphs and review conversations intact.
- **Abandon:** `lf task abandon <issue>` retires a Task and its delivery state
  together. For a branch without a Task, `lf pr abandon <branch>` closes its PR
  and removes its checkout and branches.
- **Prune:** `lf wt delete <branch>` removes the checkout, local branch, and
  origin branch while retaining PR and Task outcomes. It also accepts a full
  branch name with no worktree, including remote-only branches; omit the
  `origin/` prefix. Use it for agreed stale branches and settled leftovers.
  Use abandon when the intended outcome includes closing an open PR.
- **Batch worktree cleanup:** use `lf wt prune` to apply the entire preview,
  or delete selected branches individually. Recheck the preview before applying it. Worktree pruning does not replace the
  remote-only branch scan.
- **Database upkeep:** use supported reconciliation and cleanup commands;
  inspect installed help for their exact scope. Do not edit SQLite directly
  or discard durable history as a substitute for reconciliation. If a needed
  cleanup operation is missing, record the gap in the punch list.

Preserve active work, dirty files, unpushed commits, and evidence needed for recovery.
Resolve these findings before discarding work.
Use Loopflow's cleanup commands rather than raw branch or worktree deletion.

After actions, reread the relevant worktree, remote branch, PR, and Task state.
Keep notes of observed outcomes and remaining work. Report useful results and
any decisions that need attention; do not walk the participant through every
item or make clearing the entire backlog a prerequisite for their next request.
Record partial cleanup accurately rather than treating an attempted command
as success.

## Summarize current activity

Give a concise update grouped in this order: what waits on the participant,
what is moving, what is stuck, and other people's work left alone, then what
finished, due follow-ups, and backlog. Include cleanup results and useful
next moves. Use `lf wave list --json`, `lf wave status <wave> --json`, and Task status
for shared state; inspect PRs and branches where they explain a gap.

Refresh relevant facts after actions and when an update is needed. Link Tasks
and PRs so the participant can reach the work. Distinguish observed results
from attempted actions. Keep unresolved upkeep in `scratch/open-work.md`;
put routine progress in this conversation, not Task comments.

## Drive work through Waves

Operate each Wave that has started work by following `lf help wave-operate`:
it owns the ownership rule, dispositions and recovery. Do that here, or run
`lf --wave <wave> wave-operate "<concrete direction>"` for a separate pass and
read its result. Operate started Tasks you own outside any Wave with `lf task-operate <issue>`.
Sending the user to another conversation is not operating. The repository view
connects outcomes and dependencies across Waves; the Wave pass owns the
detailed judgment within each Wave.

Do not create a competing Graph process or require a repository pass before a
Task can progress. Follow an intervention through to its observed result, then
include that result in the repository update.

## Reviewer mode

- **Interactive reviewer:** discuss consequential decisions here.
- **Parent reviewer:** send evidence-backed recommendations through the
  review protocol and verify the Task's replies. Do not mutate its checkout or launch
  competing execution. Keep unresolved choices explicit in the review outcome.

## Sessions and reviews

Use `lf session list --available --json` to identify conversations available for
a person through `availability`; a quiet working step can appear there. `task_ids` is the shared Task membership,
including checkout association; `work` alone is not. An open or interrupted
conversation does not by itself mean its Task is blocked.

Use `lf session list --json` for all current conversations. Refresh after
Session mutations; never rely on Session content remembered from an earlier
turn or embedded in the launch prompt. Compare CLI and Desktop only against
the same runtime/Machine; a failed read is not an empty list.

The list is scoped to the repository this conversation runs in: worktrees
collapse to their main checkout, and review steps from other repositories are hidden.
Add `--all` to see every repository's review steps on this machine. The same
repository scope governs `lf wave list` and `lf wave show` (both take `--all`); `lf wave
status` resolves one Wave within the repository.

When the User selects a Session, run `lf connect <session-id> --prepare --json`.
It prepares or recovers the boundary's Session and captured input and returns its
`open_argv` for the app or requested terminal. Execute that argv unchanged: it
carries the executable and owning data together. Preparation does not mean
the conversation opened; verify provider readiness in the requested terminal.
Explain waiting and active states plainly. Listing reads the selected
installation's store; a missing Session does not prove deletion from retained
installations.

Questions for the participant stay in this conversation. A separate Work
perspective is an ordinary `lf --task <task> : "<prompt>"` contribution.
Headless work that lacks required input explains its failure and stops; the
Wave operator reads its logs.

## Machines

A Task is owned by a person on a Machine, or unowned. Each person picks their own
Machine for a Wave's or the repository's new Tasks with `lf --wave <wave> set machine <machine>`
or `lf --repo <repo> set machine <machine>`, never committed; unset means the Machine running
the command. This pass changes no Task's owner or Machine. Use `lf id`, then
`lf --wave <wave> wave-operate` locally or
`lf --machine <machine-id> --wave <wave> wave-operate` on the Wave's Machine.
`lf --machine` runs the target's `lf`; its SSH route may change without moving
Work. Connect missing logins in the foreground with `lf machine connect`; work
uses credentials resident on its Machine.

When evidence invalidates an attempt on a Task you own, `lf task interrupt <issue>`
stops its live execution and discuss the changed direction in the Task conversation.
The stopped Graph stays as history; Task, worktree and PR identity are unchanged.

## Diagnose execution and auth

When work seems stuck, run `lf top` before guessing; redirected output gives one
frame. `lf ps --json` is the parseable snapshot. These show OS-live call trees,
normalized output rates, completed token usage, age, idle time, health and PIDs.
Time alone never means dead. `lf mon prune --dry-run` shows cleanup candidates;
plain prune removes dead receipts and registered orphan provider groups. Never
kill an `unclaimed` PID: ownership is not proven.

Inspect `lf account --cached` before proposing an account repair. It is offline;
use `lf account --json` for managed provider acceptance. Local
service state and managed logins are separate evidence. OAuth client
credentials resolve from environment first, then Doppler when configured. For
a repository using Doppler, give `doppler run -- lf account connect <provider>` when those
credentials are missing. Otherwise name the missing credential variables and
follow the customer's secret manager. Never print values or run a secret getter
bare. Do not change accounts or Task ownership merely to make an inspection pass.
