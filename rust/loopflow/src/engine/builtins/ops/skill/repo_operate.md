---
description: Clear obsolete work state, report activity, advance Waves, and capture Tasks.
---

# Operate repository work

Keep the repository's work state clean and its work moving. Clear old state
that is no longer needed, summarize current activity, drive Tasks forward
through their Waves, and capture new Tasks as direction emerges.

## Clear old work state

### Find stale work

Read `lf roadmap --json`, `lf session list --json`, and `lf wt list --json`.
Join Task and Session state to worktrees using Loopflow's returned identities
and paths. Inspect branches and PRs to explain gaps in that shared picture.

Scope cleanup to the participant's own work. Read `gh api user --jq .login`
and `git config user.email`, then check PR authorship and the repository's
configured branch naming convention. A tip commit's author alone does not
establish branch ownership. Leave uncertain ownership unresolved and continue with the rest; do not
triage teammates' PRs or delete their branches.

Use these checks where they help establish what can be cleared:

- Open PRs: `gh pr list --author @me --state open --limit 1000 --json number,title,headRefName,url,isDraft,mergeStateStatus,statusCheckRollup,updatedAt`.
  Paginate if the result reaches the limit; missing or incomplete data stays unknown.
- Remote branch candidates: `git for-each-ref --format='%(refname:short) %(committerdate:iso8601) %(authoremail)' refs/remotes/origin`.
  Filter to the participant's branch namespace and corroborate ownership with
  PR history. These are cached refs; check `git ls-remote --heads origin`
  before treating a branch as present or gone. Exclude the default branch
  and symbolic refs.
- Worktree condition: inspect `git -C <path> status --short`, unpushed commits,
  associated Tasks, and active Sessions/Flows. Run `lf wt prune --dry-run` to
  identify merged, squash-merged, closed-PR, remote-gone, terminal, and inactive
  cleanup candidates. Review the preview; inactivity alone is not abandonment.
- Branch disposition: compare ahead/behind the repository's default branch,
  inspect unique commits, and check PR history for merged or squash-merged work.
  Check for commits added after a merge; a merged PR does not settle later work.
- Remote-only stale candidates: no worktree, no open PR, and no commits in
  60 days. Check each candidate with
  `gh pr list --head <branch> --state all --limit 1000 --json number,state,author,url,mergedAt,headRepository`.
  Match the head repository as well as the branch name. Check open PRs regardless
  of author before deletion. Age selects candidates, not permission to delete.

### Clear what can be cleared

Follow the evidence to useful actions. Combine related cleanup, finish clear
cases, and keep moving when one item is uncertain. Use `scratch/open-work.md`
for findings worth retaining: affected branches or paths, Task/PR links,
recommendations, supporting evidence, and unresolved work. Choose a format
that fits the findings; a table is useful for many items, not a required agenda.
Do not create Waves for waveless branches.

Decide whether work should ship, ship partially, be abandoned, or be pruned.
Investigate uncertainties that can be answered from available evidence. Bring
only consequential choices requiring the participant's judgment or missing
authorization to the conversation, with a recommendation and its reason.
An unresolved item need not hold up the rest of the upkeep.

Use the appropriate operation within existing authorization:

- **Ship / ship-partial:** use the delivery skill appropriate to the requested
  outcome in the owning checkout. Preserve useful unfinished scope in a Task
  when authorized. Keep existing workers and review gates intact; a cleanup
  scan does not authorize publication or merge.
- **Abandon:** `lf task abandon <issue>` retires a Task and its delivery state
  together. For a branch without a Task, `lf pr abandon <branch>` closes its PR
  and removes its checkout and branches. Use `lf task delete <issue>` only when
  the issue itself should be removed.
- **Prune:** `lf wt delete <branch>` removes the checkout, local branch, and
  origin branch while retaining PR and Task outcomes. It also accepts a full
  branch name with no worktree, including remote-only branches; omit the
  `origin/` prefix. Use it for agreed stale branches and settled leftovers.
  Do not prune a branch with an open PR unless explicitly authorized; use
  abandon when the intended outcome includes closing that PR.
- **Batch worktree cleanup:** apply `lf wt prune` only when the entire preview
  is authorized; otherwise delete the agreed branches individually. Recheck
  the preview before applying it. Worktree pruning does not replace the
  remote-only branch scan.
- **Database upkeep:** use supported reconciliation and cleanup commands;
  inspect installed help for their exact scope. Do not edit SQLite directly
  or discard durable history as a substitute for reconciliation. If a needed
  cleanup operation is missing, capture it as a Task within existing filing
  authorization, or leave the gap in the punch list.

Present the exact destructive choices when authorization is missing. Preserve
active work, dirty files, unpushed commits, and evidence needed for recovery.
Do not use force to bypass these findings without explicit discard authority.
Use Loopflow's cleanup commands rather than raw branch or worktree deletion.

After actions, reread the relevant worktree, remote branch, PR, and Task state.
Keep notes of observed outcomes and remaining work. Report useful results and
any decisions that need attention; do not walk the participant through every
item or make clearing the entire backlog a prerequisite for their next request.
Record partial cleanup accurately rather than treating an attempted command
as success.

## Summarize current activity

Give a concise update: what finished, what is running, what is waiting, what is
blocked, and what needs a decision. Include cleanup results and useful next
moves. Use `lf wave list --json`, `lf wave status <wave> --json`, and Task status
for shared state; inspect PRs and branches where they explain a gap.

Refresh relevant facts after actions and when an update is needed. Link Tasks
and PRs so the participant can reach the work. Distinguish observed results
from attempted actions. Keep unresolved upkeep in `scratch/open-work.md`;
put routine progress in this conversation, not Task comments.

## Drive work through Waves

For each Wave that needs attention, apply `wave/operate`: read its objective,
memory, plan, and current work; make useful authorized moves that advance its
Tasks. Use `lf --wave <wave> wave/operate "<concrete direction>"` when a separate
Wave pass is useful. The repository view connects outcomes and dependencies
across Waves; the Wave pass owns the detailed judgment within each Wave.

Leave healthy workers moving. Preserve Task identity, selected Flows, review
gates, and existing execution. Do not create a competing driver or require a
repository pass before a Task can progress. Follow an intervention through to
its observed result, then include that result in the repository update.

## Capture new Tasks

Be ready to turn emerging requests, cleanup gaps, and discoveries into Tasks.
Check existing work first; reuse the Task when it is the same problem. Capture
the desired outcome, observable acceptance, and real constraints in the right
Wave. Resolve unclear placement with the participant rather than inventing it.
Filing a Task need not start execution. Keep tentative ideas distinct from
accepted direction; use `concept-review` when existing work needs rethinking.

## Reviewer mode

- **Interactive reviewer:** discuss consequential decisions here; obtain only
  authorization not already supplied.
- **Parent reviewer:** send evidence-backed recommendations through the
  review protocol and verify the Task's replies. Do not mutate its checkout or launch
  competing execution. Destructive actions require prior authorization; missing
  authority remains explicit in the review outcome.

## Sessions and reviews

Use `lf session list --json` to identify conversations and unresolved reviews.

The list is scoped to the repository this conversation runs in: worktrees
collapse to their main checkout, and review steps from other repositories are hidden.
Add `--all` to see every repository's review steps on this machine. The same
repository scope governs `lf wave list` and `lf roadmap` (both take `--all`); `lf wave
status` resolves one Wave within the repository.

When the User selects a Session, run `lf session connect <session-id> --json`.
It prepares or recovers the boundary's AgentSession and captured input and returns its
`open_argv` for the app or requested terminal. Execute that argv unchanged: it
carries the executable and owning data together. JSON preparation does not mean
the conversation opened; verify provider readiness in the requested terminal.
Explain waiting and active states plainly. Listing reads the selected
installation's store; a missing Session does not prove deletion from retained
installations.

Questions for the participant stay in this conversation. A separate Work
perspective is an ordinary `lf --task <task> : "<prompt>"` contribution.
`lf ask` creates a new session, so do not use it merely to reach the participant
already here.

## Launching and advancing work

Inspect whether the requested work already has a Task, prepared context, or
running worker before filing or launching. Keep the Task title and description
focused on the current problem, desired experience, observable acceptance, and
real constraints. Reconcile changed scope instead of appending amendments.
Keep current blockers, dependencies, and accepted scope visible in the description.
Keep speculative solutions tentative; detailed solutions belong in a separate
design.

To capture work without starting it:

```bash
lf task create --wave <wave> --title "<desired experience>" <<'BRIEF'
<short user-problem brief>
BRIEF
```

When execution is intended and authorized, start the captured Task or reuse an existing one:

```bash
lf --task <existing-issue> flow start <chosen-flow>
```

Stdin becomes the durable Task description; `--directive` supplies worker
direction and does not replace that brief.

Use the Flow template the user selected; otherwise use the current Linear
Project's required `flow:` default. Read the actual Flow before describing its
review gates. Do not infer policy from obsolete fix/feature flags or
first/loop/finally settings.

Current planning edits use `lf wave update-plan --wave <wave> --plan <plan.json>`.
The complete content object has `metric_targets`, a nonempty `flow` string and
`krs`; for example, `{"metric_targets":[],"flow":"feature","krs":[]}`.
It updates the Wave's one In Progress Linear Project. Planned Projects hold
future plans; Completed Projects retain history. Future plan edits require an
available authorized Linear writer, not a current-plan update.

Chapter rotation is repository-wide: preview with
`lf repo new-chapter <name> --dry-run --json`, then apply the accepted name with
`lf repo new-chapter <name> --json`. It takes no plan file or per-Wave selector.
Respect the start-chapter direction and plan-review gates; prior authorization
still applies. Retry interruptions with the same name against fresh Linear
state. Started unfinished Tasks retain identity and execution; proven untouched
backlog is canceled, retaining issues and history. Predecessor Projects become
Completed. Missing evidence and competing plans require resolution, never a
new name chosen to bypass them. Rotation output is dated evidence, not another
durable Chapter owner.

For an existing invocation or an exact review:

```bash
lf --task <issue> flow start
lf session complete <session-id>
```

Complete ends the exact review when the User asks to proceed and returns its
saved feedback to the next step. The following loop-decide owns navigation.
The `advance` skill resolves the next action from a review, Task, or unbound
design. State what actually started after checking status.

Use `lf task delete ISSUE` to remove a Task from Linear and reconcile its local
record. Read any partial-outcome report and retry the same command. Authored files
and retained PRs survive; deletion does not certify process termination.

## Existing-design handoff

When the user asks to file a Task and run a Flow from an existing design,
keep the design separate. Reuse the Task if it is the same work; otherwise file
a short user-problem brief under the selected Wave, with a design reference,
its maturity, and open questions. Do not invent ownership.

```bash
lf task create --wave <wave> --title "<desired experience>" --notes "<brief; design reference and maturity>"
lf checkout <issue> --json
# Copy the selected design and required evidence into the returned worktree's scratch/.
lf --task <issue> flow start <chosen-flow>
```

Inspect the current context first: a design already in the Task worktree needs
no transfer. For a separate source, copy the actual documents and supporting
files before launch, preserve relative references, and check their contents in
the destination. A path alone does not supply context. Preparation launches no
worker; put any initial directive on preparation, since an already prepared
Task rejects a new `run --directive`. Do not overwrite newer destination work.

The destination becomes the working design; retain source provenance without
maintaining competing active copies. Markdown under its recursive `scratch/`
tree enters worker context; other assets remain on disk. Preserve material
needed after scratch cleanup in durable documentation or existing records.
Do not pipe the design into Task creation: stdin becomes the Task description.

Continue the design already present without treating its draft choices as
approved. Report the Task link, destination design path, selected Flow, and
observed launch result. Verify supplied context separately from worker startup.

If implementation already exists in the source checkout, preserve it and its
writer. Document transfer does not adopt a checkout; current preparation does
not adopt an unbound existing branch/worktree. Report that gap before launching
a competing implementation. No automatic scratch-transfer flag is available.

## Placement and bounded contributions

Use shared readers: `lf wave list --json` for the repository, `lf wave status <wave> --json`
for one Wave, and `lf roadmap --json` for the plan joined to runtime evidence.
Do not reconstruct their state from processes, checkouts or Linear alone.

A Work names a stable Home authority. Placement changes through `lf wave place <wave-id> <home-id>`.
Use `lf home id`, then `lf --wave <wave> wave/operate` locally or
`lf ssh <home-id> --wave <wave> wave/operate` at its placement. `lf ssh` runs the target's `lf`;
its SSH route may change without moving Work. Foreground provider accounts can
be forwarded; durable workers use credentials installed on their Home.

Prepare a Task without launching it with `lf task checkout <issue> --json`.
For one bounded contribution use `lf --task <issue> research "<question>"` or
`lf --wave <wave> wave/operate "<direction>"`. `--task TASK` / `--wave WAVE`
attributes a skill, inline prompt, or Flow. Attribution resolves this command's
`--task`, then the checkout's Task, then an ancestor's explicit `LF_AS` declaration.
Process ancestry supplies no Work attribution. A direct contribution does not
advance the Task's selected managed Flow or claim exclusive ownership. `flow start`
sets the same declaration and drives the ordinary Flow in the Task checkout.

Task scratch Markdown enters each contribution at launch. Give independent
contributions distinct paths, wait for the artifacts needed, and inspect their
contents. A bounded contributor leaves edits uncommitted and never claims
unrelated dirty files. Checkpoint only after the coherent contributions finish.
Use `lf --task <issue> flow start <chosen-flow>` for managed pursuit. Dependent
work starts as a separate Task with `--stack-on <parent-task>`; the child binds
to the parent's active PR. Never create another branch for the same Task.

When evidence invalidates the attempt, update the Task and wait for required
contributions, then `lf restart <issue> "<changed direction>"`. Restart
checkpoints and pushes the existing tree, preserves Task/worktree/PR identity,
and starts the current Project's default Flow fresh. It interrupts an exact live
Task worker; other attributed Flows and conversations remain independent. Reconcile prior
scratch against the new evidence rather than treating it as approved design.
An explicitly selected Flow governs even when it differs from that default.

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
bare. Do not change accounts or placement merely to make an inspection pass.
