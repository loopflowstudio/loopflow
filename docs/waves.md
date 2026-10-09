# Waves

```bash
lf wave ensure product --json
```

Prepare the Wave's configured Project for work. With no binding, reserve and create
one ordinary Project; retries reuse its identity. To select an existing Project,
run `lf wave bind-project product <project-uuid>` first. Ensure activates Backlog
or Planned status without changing its name, content, Tasks or optional Flow.

```bash
lf --wave shipper wave-operate "invoices first"
lf wave status shipper
```

A Wave keeps an objective, memory, cadence, budget and metrics across Tasks.
Each invocation makes one finite planning pass. Tasks own implementation and
Flow execution; each Machine retains command and conversation history.

| Path | Holds |
|------|-------|
| `wave/<name>/GOAL.md` | Objective, operating guidance and schedules |
| `wave/<name>/MEMORY.md` | Curated decisions and lessons |
| `wave/<name>/metrics/*.md` | Metric meaning, window and freshness |

In Loopflow on macOS, select a Wave to read its plan, Tasks and execution history. Start a
Session for a conversation, or invoke `wave-operate` for one finite pass. The
Wave's ongoing conversation (`lf session ensure -w <name>`) applies that same
pass each time you return.

## Nested Waves

```text
wave/infrastructure/GOAL.md
wave/infrastructure/MEMORY.md
wave/infrastructure/release/GOAL.md
wave/infrastructure/release/MEMORY.md
```

Address Release as `infrastructure/release`. It has its own objective, plan,
Tasks and schedules. Its prompt reads Infrastructure's top-level Markdown,
then Release's, from the executing checkout; sibling Waves stay out.

Repository-root `MEMORY.md`, when present, comes before Wave files and is also
included when no Wave is selected. Read all files from the executing checkout.

Repository, ancestor and selected Wave memory share one budget, defaulting to
16,000 tokens and 128 KiB. Files receive proportional shares of that budget.
`context_budgets.memory_tokens` and `memory_bytes` configure the shared limits;
`lf context` reports each file's share and usage. Oversized files become excerpts
with links to their complete local snapshots.

Registration records the Wave UUID in `GOAL.md` frontmatter as `id:`. Commit
that field with the Wave files and retain it when moving a directory. Discovery
reconciles its one-segment name and directory parent. Renaming Infrastructure
changes Release's address without changing its ID, Project or Task links.
A fresh Machine uses the same authored IDs. Copying an ID into two Wave directories
is a conflict, not a second Wave.

The release prototype declares its schedule in
`wave/infrastructure/release/GOAL.md`. Connecting its Linear Initiative, moving
release plans and Tasks, and syncing the installed schedule are separate
post-landing operations.

## Operate manually

```bash
lf --wave shipper wave-operate "Review delivery and the checkout dependency"
lf vsm-operate
lf --wave shipper s2 : "Investigate the repeated checkout conflict"
```

`wave-operate` considers delivery, coordination, present capacity, adaptation
and identity (S1–S5) within the selected Wave. `vsm-operate` applies those same
questions across the current repository's Waves, including from a linked
worktree or a Task-bound session. Its Flow runs `s1 → s2 → s3 → s4 → s5`,
with each skill reading earlier findings from one shared working note. Each is
also directly invokable for a focused question, with no scan/assess split.
An explicit scope request can narrow VSM to a Wave; otherwise it uses the repository.
Neither pass requires Discord or a schedule. VSM makes five sequential skill
invocations, without requiring five reports or five actions. `wave-operate`
remains a single skill; whether it should use the VSM Flow is still undecided.

Each pass reads dated evidence and replies in the invoking conversation. It
gives every started Task one disposition: moving, acted on and verified,
waiting on a named person or dependency, paused, or unknown. A started Task
whose Flow stopped or failed is read and its remaining work run fresh; one at
a workflow node waits on you; unstarted backlog is
listed and left for you to select. A no-action result is valid only when every
started Task already has a disposition. Failed reads and stale provider data remain explicit
gaps; they cannot justify closing work or treating a Task as idle.

Task findings can challenge Wave purpose; Wave findings can challenge shared
repository direction. Preserve the source and disagreement with the affected
Wave or Task. Return accepted decisions to those owners with their practical
consequence. Changes beyond accepted direction stay proposals for review.
Repository synthesis does not own identity alone.

Running Task Flows keep progressing when both operators are absent; review
happens in the Task conversation. Projects belong to Waves; they have no separate
operator. Scheduling and connected chat are separate integration work. These
manual passes neither install jobs nor post replies to a channel.

The [execution contract and cutover status](architecture-reference.md#cutover-status)
track remaining implementation and acceptance.

## The planning model

```bash
lf roadmap --json
lf repo new-chapter 2026-10 --plan scratch/chapter.json --dry-run
lf repo new-chapter 2026-10 --plan scratch/chapter.json
```

The Wave's SQLite selection references its accepted Project by UUID. Status and roadmap
retain other Projects and their Tasks, including unresolved predecessor backlog.
JSON marks the selected Project with `current`; another In Progress Project does
not change that selection. Reading never creates a binding or activates a Project. Explicit ensure imports a
legacy Machine-local YAML selection once, preserving its original bytes in SQLite;
later file edits cannot change selection.

Projects hold Tasks, KRs, metric targets and an optional workflow. Projects
created together share a chapter name, such as `2026-10`; there is no chapter table.
Current navigation stays Wave → Task. Completed Projects retain their history.

Author and retain a [chapter plan](architecture/planning.md#rotate-the-plan-preserve-the-work)
with exact Wave/destination IDs and nonempty KRs. Select `create: false` for an
existing Project or allocate one UUID for creation. The repository operation
rotates the listed Waves; `lf wave new-chapter` consumes one entry. Ordinary
Project ensure needs neither a chapter nor KRs.

The preview lists each successor and Task disposition. Started unfinished Tasks
keep identity, checkout, PR and captured execution when moved. Unreviewed backlog
stays with the predecessor until explicit disposition, alongside completed Tasks.
Missing checkout, provider or execution evidence remains unresolved.

Retry with the same plan file after interruption. Rotation validates all destinations
before provider writes and retains reservations and selected issue IDs. It activates
the successor, moves selected work, switches the shared binding, then completes the
predecessor. After the switch, new historical starts stay put. An external move of
selected work remains a conflict. Earlier settled Waves remain settled during retry;
Project names never choose an endpoint or overwrite a later binding.

The operation is recoverable within its owning Machine, not atomic across provider
mutations. Read-only planning and unrelated Wave ensure remain independent of it.

Name the workflow the Project's Tasks take up in its content:

```markdown
workflow: feature

## KRs

- [ ] A new contributor ships a change without an undocumented dependency.
```

A Task with no workflow takes that one up on its first `lf task run <task>`;
naming another takes that up instead, and the Task keeps it.
`lf project workflow set PROJECT NAME` validates the Workflow and rewrites only
that selection, preserving KRs and metric targets. `lf task run` refuses a name
that is not a workflow until a Flow or workflow is named. A Project written with the earlier
`flow:` line reads the same and is rewritten by the next plan update. Existing
Projects observed before the status-model upgrade retain their identity and
custom workflow. The first explicit `lf refresh` or chapter rotation
converts their old `recommended:` line to `workflow:` and marks the recorded current
Project In Progress. Until then, planning reads project that same conversion
without changing Linear. Deliberately Planned successors stay Planned; archived
predecessors stay historical even if their old status says In Progress.

If no receipt identifies the old current Project, adoption requires a single
unambiguous candidate. Resolve competing plans in Linear; names and dates never
break the tie. A missing workflow stays missing. Creation, adoption, plan edits and
rotation do not require one; a bare `lf task run` still needs the Task or its
Project to name a workflow.
Unobserved backlog on another Machine is unresolved, so a missing local Task row
never establishes that work should be canceled.

## The Goal

`GOAL.md` carries the Wave objective and operating guidance. Frontmatter holds
configuration; each finite pass reads the body.

```markdown
---
owner: jack
home: build-vm.example.com
---

## Objective

Keep the runtime architecture legible. Each loop: read Linear tasks and
memory, pick the next useful move, resolve its local blocker, spin off
independent work only when parallelism earns it, and fold what shipped into
memory.

## Process

Make mechanical changes directly; write a scratch design first when the blast
radius crosses storage, auth, or public APIs.
```

Work placement records the owning Machine. Use `lf cron`
for scheduled commands and `lf --wave <name> wave-operate` for an immediate pass.

Builtin goals resolve by name, including the five Viable System Model charters
`s1`…`s5`. Goal authoring is covered in [Authoring → Goals](authoring.md#goals).

### Live metrics

```markdown
---
schema: 1
id: task-loop-trust
stage: installed
instrument: lifecycle-scorecard
unit: ratio
window: 7d
freshness: 30h
---

# Task loops earn trust

Fraction of Tasks settled during the trailing seven days that either
completed with every PR landed through Loopflow auto-merge or stopped with a
non-resumable failure receipt. Open Tasks are excluded. A user-landed PR or
manual Git repair inside the Task fails the metric.
```

Save this as `wave/<name>/metrics/task-loop-trust.md`. The filename and `id`
must match. The enclosing Wave owns the metric across chapter boundaries.
Contracts define no command, query, schedule, secret, or KR copy. Product code
registered as the named instrument pushes pre-aggregated, revision-bound
observations into the local store.

Set targets in the chapter plan, not the instrument contract:

```json
{"metric_targets":[{"metric_id":"task-loop-trust","target":{"kind":"at_least","value":1}}],"workflow":"feature","krs":[]}
```

Apply this complete Wave plan with
`lf update-plan --wave <wave> --plan plan.json`. For the next chapter,
edit the Planned Project in Linear.
An omitted metric has no target in that chapter; its observations remain visible
without a pass/fail verdict. Changing a target preserves instrument identity,
revision, and measurement history. Completed Projects retain their targets and KRs in Linear. Metric observations
remain owned by the Wave; rotation does not freeze another copy of those readings.
The Wave objective stays in `GOAL.md`; Project plans have no second objective.

```bash
lf wave status <wave>                  # owner, value, target, window, freshness, reason
lf wave status <wave> --json           # the shared metric_portfolio DTO
lf roadmap --json                      # the same DTO on every Wave row
```

`installed` metrics appear under Instrumenting. Promote a contract to
`graduated` only after the current revision has produced a complete observation
for the exact window. Graduation certifies collection, not success: a Missed
metric can graduate. Later silence becomes Unknown, a current failed source
read becomes Unavailable, and stale evidence never remains green. Metrics
inform chapter KR judgment but never check a KR automatically.

### Discord bridge

```yaml
# wave/product/GOAL.md frontmatter
chat:
  provider: discord
  guild_id: "123456789012345678"
  channel_id: "234567890123456789"
```

```bash
doppler run -- lf discord serve product
```

Run one foreground bridge for the configured channel. It polls every five
seconds, turns each nonempty, non-bot message into a bounded Wave-attributed
conversation, and replies with that conversation's final answer. Mentions are disabled in replies.
The token is `LF_DISCORD_TOKEN`, injected through Doppler and removed from
provider child environments. The bot needs View Channel, Read Message History,
Send Messages and access to message content.

The cursor exists only in memory. Every start skips existing channel history;
restarting after a failure does not replay missed messages. The bridge uses the
channel binding; `guild_id` is configuration metadata, not a process owner. Run a single bridge per channel to avoid duplicate replies.
There is no local Wave transcript, inbox, listener or automatic service startup.
Use Sessions for native conversations and `lf mon show --wave product` for historical launch inspection.

### Memory

`MEMORY.md` is durable working context agents curate as Wave work moves —
decisions, dead ends, what a downstream task should know. It is an ordinary
reviewed repository file:

```bash
$EDITOR wave/shipper/MEMORY.md
```

Prompts read every top-level `.md` in the selected Wave directory and each
ancestor directory, root first, from the executing checkout. For
`infrastructure/release`, that means `wave/infrastructure/*.md` followed by
`wave/infrastructure/release/*.md`. Memory uses the same reader as goals and
notes. Children, siblings and unrelated Waves stay out of ordinary context;
`scratch/` remains recursive. No registry lookup is needed to gather these files.

Edit memory through the ordinary repository workflow. `realign` reconciles the
plan, code and Wave memory; the prompt identifies the selected Wave's memory
as the file to curate.
At a parent Wave, `realign` explicitly reads the child Waves' top-level Markdown
to bring relevant findings into the parent's memory.

### Machine

```bash
lf id
lf machine add jack@mini.local --label mini
lf machine list --json
lf wave place <wave-id> <machine-id>
lf --machine <machine-id> --wave shipper wave-operate
lf --machine <machine-id> wave status shipper --json
```

A Machine is a stable machine identity with a replaceable route. Placement records
where Work belongs; it does not start a process or confer signal authority.
New Project or Task Work inherits its parent's recorded Machine once. Readers
operate on the selected Machine; they never silently aggregate other Machines.

```bash
lf wave rename <wave-id> --name platform
lf wave rename <wave-id> --repo ../moved-repository
```

Relocation preserves the Wave UUID, Linear projection, authored files, Work and
Machine placement. It protects unmerged authored work and uses a receipt to finish
filesystem cleanup after the locator commits. It does not move historical Wave
journals or Machine-local execution history. Source and target PM Teams must match; use
`lf repo reteam` for a provider ownership change.

See [Machines and processes](architecture/machines.md) and
[Security](security.md#understand-account-authority-over-ssh).

## Chapter plans and KRs

```bash
lf wave status infra --json
lf update-plan --wave infra --plan plan.json
lf project workflow set PROJECT research    # only the workflow
lf project workflow customize research                     # .lf/workflows/research.yaml, written from the builtin
```

`plan.json` contains the complete current plan:

```json
{"metric_targets":[],"workflow":"feature","krs":[{"text":"A new contributor ships a change using the architecture guide without an undocumented dependency.","holds":false}]}
```

The Wave objective names who benefits and what improves. Chapter KRs prove observable outcomes
across a stated window. Update the current plan explicitly; a new chapter copies only
the workflow, retaining any explicitly prepared successor plan.

## Linear

Tasks live in Linear; there are no local task lists. A Wave maps to an
Initiative, its Project for each Chapter maps to a Linear Project, and each
Task maps to an Issue. The chapter name groups the current Projects. Connect
once — `lf repo connect` links or creates the Wave Initiative and establishes one
repository Team in `.lf/config.yaml`. Every Wave reuses that Team and issue-key
namespace. Don't paste ids by hand.

```bash
lf repo connect infra --team-key LOO # first Wave establishes the repo Team
lf repo connect --all                  # all nested Waves reuse it
lf refresh infra              # refresh the local SQLite snapshot
lf wave status infra                   # deterministic cache-only read
lf task create --wave infra --title "Daemon data integrity"
lf task move 1207... end --reason "Dark mode delivered"
```

A managed Project belongs to exactly one Initiative and exactly the repository
Team. Project titles remain as authored in Linear, including ordinary names such
as `Summer work — customer requests`. Refresh, sync and reteam preserve them;
stable IDs and Project membership resolve Work.

## Tasks

Every concrete file-writing change begins with a Linear task and runs as a
durable Task Work in its own stable sibling worktree:

```bash
lf task create --wave <wave> --title "add retry to token refresh"
pbpaste | lf task create --wave incidents
lf checkout INF-123
lf --task INF-123 research "write scratch/retry-analysis.md"
lf task run INF-123
lf task run INF-124 --stack-on INF-123 # dependent work before the parent merges
lf task run INF-125 incident
```

Task Work advances through one active remote branch and PR to `main`. Its
Project's `workflow:` (`feature`, `code`, `research`, or a file in
`.lf/workflows/`) is the one it takes up. The Task moves through its
nodes: each `lf task run` takes one edge leaving the current node and runs
that edge's Flow, and at a node the Task waits on you in its conversation.
See [workflows](authoring.md#workflows).
Each launch starts a fresh Flow: one driver process holding the expanded graph
and the step processes it starts. Every Flow remains history and none is privileged. A finished Flow
leaves Task Work open until an explicit completion or delivery operation
settles it.

Task context includes the Wave's `GOAL.md` and `MEMORY.md` plus its Project's
KRs and targets. Explicit PR rotation selects the next serial branch while
preserving the Task's worktree directory.

AgentSessions and Processes own typed nullable Task/Wave ancestry.
Historical work events retain their original attribution. Launching `lf` in a registered Task checkout binds automatically unless
an explicit selector overrides it. A later bind can attach a conversation to
a done or landed Task without reopening Work. Assignment is permanent and
states the permanent target in CLI; Desktop confirms it. An existing Task cannot change.
See [Sessions](lf-reference.md#session-conversations-and-reviews)
for rename, bind, and the distinction between ancestry and Flow membership.

Each Task PR keeps its own benefit-focused title. After the opening summary,
Loopflow adds the canonical Task name, Linear link, and merge consequence.
Publication refreshes that context without replacing the title or summary.

Task events remain durable evidence for the next finite Wave pass. Status,
steering and recovery use the same commands for people and agents:
[The Agent API → Steer](agent-api.md#steer).

```bash
lf land --next parser-proof   # after verified merge, rotate to the next
lf land -c                    # after verified merge, complete the Task
lf task move INF-124 end --reason "investigation recorded"   # no PR needed
```


A Task's state is where it stands on its workflow: ready at `start`, active
between, done at `end`. Reaching `end`, by an edge or by `task move`, completes
the Task; no other command does. It is refused over uncommitted changes, an
unsettled PR or unresolved execution, and retires a PR whose branch never moved.
An edge that runs nothing lands nothing: it accepts uncommitted changes and
keeps the checkout.
`task move ISSUE end` also finishes planning-only Tasks without creating a
checkout, recording the reason once in Linear. If completion reports pending PM
writeback, repeat the command to reconcile Linear. Canceled and duplicate
issues cannot be changed to completed.

Linear completing a Task that is active here is shown on the Task as an error.
Its work goes on; `end` then takes `--force`. Linear completing a Task that
never left `start` withdraws it from what is offered.

Keep each PR reviewable — roughly 1000 LOC. A Task may need several serial
PRs, but it still needs one concrete finish line.

## Independent evidence

A Wave can launch independent Tasks to test competing mechanisms for an
uncertain chapter KR. Each Task returns evidence, an artifact, an exact gap,
or a counterexample. The Wave compares the findings and directs the next work.
Keep these Tasks in the one current chapter; no additional Project is needed.

To remove a wave, stop it, then delete `wave/<name>/`.

## Worked example

A `wave/billing/` directory for a billing rewrite. `GOAL.md` sets the intent —
"replace the legacy billing system with a metered usage model." The current chapter
holds the proof claims. Reviewed contracts under `wave/billing/metrics/` define
live evidence such as usage-event latency and invoice correctness; the Wave
re-judges strategy from their current readings each iteration.

The backing Linear project holds the concrete Tasks:

```text
Usage events       → Event capture and storage
Metering API       → Public metering endpoint
Invoice generation → Monthly invoice calculation
Migration shim     → Legacy API compatibility layer
Cleanup            → Remove old billing code
```

The Wave reads its chapter and Tasks with `lf wave status`, judges the
KR evidence, and starts Task Work for every independent
file-writing change. Each shipped PR folds into memory and closes its task.

## Next

[The Agent API →](agent-api.md) · [Conducting →](conducting.md) · [Get Started →](getting-started.md)

## Reference

[Configuration](config.md) · [Troubleshooting](troubleshooting.md)
