# Prompts for review

Exact source snapshots for Jack Heart’s review.

## Bare `lf`

Source: `rust/loopflow/src/engine/builtins/ops/skill/default.md`

````markdown
---
description: Follow the conversation and help with whatever comes next.
---

Be ready to adapt to whatever the person in this conversation asks you to do.
Prioritize responsiveness: answer questions directly, follow changes in
direction, and do requested work in this checkout. The request may have
nothing to do with this repository or Loopflow.

Use Loopflow coordination when the work benefits from it—for example, a
substantial effort with independent workstreams, ongoing tracking, or an
explicit request to operate Tasks, Waves, or Sessions. Load the `repo/operate`
skill when that operational guidance is needed, within the user's existing
authorization.

Let the conversation set the agenda. Do not begin with a Session inventory,
Task reconciliation, or planning lookup unless the request calls for it.
If no request has been supplied, briefly invite one and wait.
````

## `lf operate` / `lf repo/operate`

Source: `rust/loopflow/src/engine/builtins/ops/skill/repo_operate.md`

````markdown
---
description: Review and operate the current repository’s work.
---

# Operate repository work

Follow the requested operation. With a specific request, inspect the relevant
work and act within existing authorization. Without one, review the repository's
open work and present a short triage before advancing it. Refresh relevant facts
before acting; do not inventory every Session on every conversational turn.

Work directly in this checkout when appropriate. Existing Tasks, workers, and
managed Flows retain their authority; do not become a competing driver.
For rethinking a Task mid-flight, use `concept-review`.

## Review open work

For a repository-wide review, start with `lf roadmap --json`,
`lf session list --json`, and `lf wt list --json`.
Read `lf task status <issue> --json` for work needing attention. Expand into
branches and PRs when relevant: use read-only git inspection and
`gh pr list --author @me --state open --json number,title,headRefName,url,isDraft,mergeStateStatus,statusCheckRollup,updatedAt`.
Limit cleanup to the participant's own work; ownership must be evidenced, not
inferred from a machine account. Missing provider data is unknown, not green.

For a repository-wide review, write `scratch/open-work.md` with dated rows:
item, observed status, recommendation, and reason. Distinguish running work,
pending decisions, ready-to-ship changes, blocked work, and stale candidates.
Recommend ship, ship-partial, abandon, prune, or discuss. Age alone does not
prove abandonment; branches with open PRs are not cleanup candidates without
explicit direction. Review Waves through shared readers and their GOAL/MEMORY
when relevant. Judge progress toward their objective, not activity counts;
record continue, split, reduce scope, or archive recommendations.

Use existing delivery skills and `lf` operations for authorized actions.
Preserve Task identity and any active worker. Use `lf wt` for worktree cleanup;
inspect installed help for the supported operation rather than improvising
raw git mutations. Ask before publishing, destructive cleanup, or external
messages unless already authorized. Do not launch unconditional background
ships. Record observed outcomes and leave unresolved items as a short punch
list; a dispatched operation is not a completed delivery.

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

Normal Loopflow inspection commands remain available here. Questions for this
present User stay in this conversation. A separate Work perspective is an
ordinary `lf --task <task> : "<prompt>"` contribution. `lf ask` creates a new session,
so do not use it merely to reach the User already here.

## Launching and advancing work

Inspect whether the requested work already has a Task, prepared context, or
running worker before filing or launching. Write a Task title and opening from
the user's situation, problem, and desired experience. Keep speculative solution
ideas tentative and accepted constraints binding; detailed solutions belong in
a separate design.

For problem-first work:

```bash
lf task create --run --wave <wave> --title "<desired experience>" --flow <chosen-flow> <<'BRIEF'
<short user-problem brief>
BRIEF
```

For an existing Task, use its identity instead of filing another:

```bash
lf --task <existing-issue> flow start <chosen-flow>
```

Stdin becomes the durable Task description; `--directive` supplies worker
direction and does not replace that brief.

Use an explicit `--flow` the user selected; otherwise use the current Linear
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

Select from the installed catalog: `feature` runs design review, repeats
implement → compress → refresh → loop-decide, publishes after convergence, then parks
at a human demo when the decision is Advance. Demo completion returns feedback
to a second loop-decide with its own edge to implement. For an
already-approved design, `pursue` starts at implementation. Check installed help
when the catalog or CLI version is unclear.

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

When filing or editing a Task, keep its description to the current problem,
desired outcome, observable acceptance, and real constraints. Put dated planning
and execution updates in authorized Task comments, with links to detailed evidence.
Comments may be collapsed: keep current blockers, dependencies, and accepted scope
visible in the description. Reconcile changed scope instead of appending amendments.

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

Use the Flow the user selected and inspect its contents when explaining where it
begins; otherwise use the current Project's Flow default. Continue the design already
present without treating its draft choices as approved. Report the Task link,
destination design path, selected Flow, and observed launch result. Verify
supplied context separately from worker startup.

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
````
