# Waves

```bash
lf --wave shipper wave/operate "invoices first"
lf wave status shipper
```

A Wave keeps an objective, memory, cadence, budget and metrics across Tasks.
Each invocation makes one finite planning pass. Tasks own implementation and
Flow execution; each Home retains command and conversation history.

| Path | Holds |
|------|-------|
| `wave/<name>/GOAL.md` | Objective, operating guidance and schedules |
| `wave/<name>/MEMORY.md` | Curated decisions and lessons |
| `wave/<name>/metrics/*.md` | Metric meaning, window and freshness |

In Loopflow on macOS, select a Wave to read its plan, Tasks and execution history. Start a
Session for a conversation, or invoke `wave/operate` for a bounded planning pass.

## Operate manually

```bash
lf --wave shipper wave/operate "Review delivery and the checkout dependency"
lf vsm-operate
lf --wave shipper s2 : "Investigate the repeated checkout conflict"
```

`wave/operate` considers delivery, coordination, present capacity, adaptation
and identity (S1–S5) within the selected Wave. `vsm-operate` applies those same
questions across the current repository's Waves, including from a linked
worktree or a Task-bound session. Its Flow runs `s1 → s2 → s3 → s4 → s5`,
with each skill reading earlier findings from one shared working note. Each is
also directly invokable for a focused question, with no scan/assess split.
An explicit scope request can narrow VSM to a Wave; otherwise it uses the repository.
Neither pass requires Discord or a schedule. VSM makes five sequential skill
invocations, without requiring five reports or five actions. `wave/operate`
remains a single skill; whether it should use the VSM Flow is still undecided.

Each pass reads dated evidence, aims for one or two useful authorized moves,
and replies in the invoking conversation. Evidence determines how much is
worthwhile; the number is guidance. A no-action result is useful when
the evidence supports it. Failed reads and stale provider data remain explicit
gaps; they cannot justify closing work or treating a Task as idle.

Task findings can challenge Wave purpose; Wave findings can challenge shared
repository direction. Preserve the source and disagreement with the affected
Wave or Task. Return accepted decisions to those owners with their practical
consequence. Changes beyond accepted direction stay proposals for review.
Repository synthesis does not own identity alone.

Tasks keep progressing through their selected Flows and human review gates
when both operators are absent. Projects belong to Waves; they have no separate
operator. Scheduling and connected chat are separate integration work. These
manual passes neither install jobs nor post replies to a channel.

The [execution contract and cutover status](architecture-reference.md#cutover-status)
track remaining implementation and acceptance.

## The planning model

```bash
lf roadmap --json
lf repo new-chapter 2026-10 --dry-run
lf repo new-chapter 2026-10
```

A Wave's one In Progress Linear Project holds its current Tasks, KRs, metric
targets and default Flow. Projects created together share a chapter name, such
as `2026-10`. The chapter is that group of Projects; there is no chapter table,
plan packet or local switch. Current navigation stays Wave → Task. Completed
Projects retain previous plans and Tasks in Linear.

Create a Planned Project in Linear to prepare the next plan. Rotation reuses the
Planned Project with the requested name in each Wave, or creates an empty one
with the predecessor's `flow:`. It never copies checked KRs or metric targets.
Every Wave participates, including Waves with no Tasks. A new Wave with no
Projects starts with `flow: feature`, or uses its explicitly Planned successor.

The preview lists every successor and Task disposition. Started unfinished Tasks
keep identity, checkout, PR and captured execution when moved. Proven untouched
backlog is canceled; its issues and local history remain. Completed Tasks stay
with the predecessor. Missing checkout, provider or execution evidence remains
unresolved and prevents automatic retirement.

Retry the same command after interruption. Rotation reads fresh provider state,
activates each successor, moves or cancels Tasks, then completes its predecessor.
During that sequence both Projects can be In Progress. A mix of the requested
name and one shared predecessor name is recoverable; competing predecessor
names are reported for resolution in Linear. Nothing wins because its name is
newer. A Wave without a current Project requires an unambiguous predecessor;
the command never selects an arbitrary historical plan.

There is no transaction across Linear mutations or across Homes. A second Home
observes the same statuses on `lf wave sync --wave <wave>` or its next normal
planning refresh. Lost responses are reconciled by stable Project and issue IDs.
A successful preview does not authorize ignoring later external reassignments.

Set the Project's default Flow in its content:

```markdown
flow: feature

## KRs

- [ ] A new contributor ships a change without an undocumented dependency.
```

`lf task run <task>` uses that Flow unless `--flow` overrides it. Existing
Projects observed before the status-model upgrade retain their identity and
custom default Flow. The first explicit `lf wave sync` or chapter rotation
converts their old `recommended:` line to `flow:` and marks the recorded current
Project In Progress. Until then, planning reads project that same conversion
without changing Linear. Deliberately Planned successors stay Planned; archived
predecessors stay historical even if their old status says In Progress.

If no receipt identifies the old current Project, adoption requires a single
unambiguous candidate. Resolve competing plans in Linear; names and dates never
break the tie. A missing default stays missing: set `flow:` before rotation.
Unobserved backlog on another Home is unresolved, so a missing local Task row
never authorizes cancellation.

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

Work placement records the owning Home. Use `lf cron`
for scheduled commands and `lf --wave <name> wave/operate` for an immediate pass.

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
{"metric_targets":[{"metric_id":"task-loop-trust","target":{"kind":"at_least","value":1}}],"flow":"feature","krs":[]}
```

Apply this complete Wave plan with
`lf wave update-plan --wave <wave> --plan plan.json`. For the next chapter,
edit the Planned Project in Linear.
An omitted metric has no target in that chapter; its observations remain visible
without a pass/fail verdict. Changing a target preserves instrument identity,
revision, and measurement history. Completed Projects retain their targets and KRs in Linear. Metric observations
remain owned by the Wave; rotation does not freeze another copy of those readings.
The Wave objective stays in `GOAL.md`; Project plans have no second objective.

```bash
lf wave status <wave>            # owner, value, target, window, freshness, reason
lf wave status <wave> --json     # the shared metric_portfolio DTO
lf roadmap --json           # the same DTO on every Wave row
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
Use Sessions for native conversations and `lf runs --wave product` for historical launch inspection.

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

### Home

```bash
lf home id
lf home observe <home-id> ssh://jack@mini.local
lf wave place <wave-id> <home-id>
lf ssh <home-id> --wave shipper wave/operate
lf ssh <home-id> wave status shipper --json
```

A Home is a stable machine identity with a replaceable route. Placement records
where Work belongs; it does not start a process or confer signal authority.
New Project or Task Work inherits its parent's recorded Home once. Readers
operate on the selected Home; they never silently aggregate other Homes.

```bash
lf wave relocate <wave-id> --name platform
lf wave relocate <wave-id> --repo ../moved-repository
```

Relocation preserves the Wave UUID, Linear projection, authored files, Work and
Home placement. It protects unmerged authored work and uses a receipt to finish
filesystem cleanup after the locator commits. It does not move historical Wave
journals or Home-local execution history. Source and target PM Teams must match; use
`lf repo reteam` for a provider ownership change.

See [Homes and processes](architecture/homes.md) and
[Security](security.md#understand-account-authority-over-ssh).

## Chapter plans and KRs

```bash
lf wave status infra --json
lf wave update-plan --wave infra --plan plan.json
```

`plan.json` contains the complete current plan:

```json
{"metric_targets":[],"flow":"task-design","krs":[{"text":"A new contributor ships a change using the architecture guide without an undocumented dependency.","holds":false}]}
```

The Wave objective names who benefits and what improves. Chapter KRs prove observable outcomes
across a stated window. Update the current plan explicitly; a new chapter copies only
the default Flow, retaining any explicitly prepared successor plan.

## Linear

Tasks live in Linear; there are no local task lists. A Wave maps to an
Initiative, its Project for each Chapter maps to a Linear Project, and each
Task maps to an Issue. The chapter name groups the current Projects. Connect
once — `lf wave connect` links or creates the Wave Initiative and establishes one
repository Team in `.lf/config.yaml`. Every Wave reuses that Team and issue-key
namespace. Don't paste ids by hand.

```bash
lf wave connect --wave infra --team-key LOO     # first Wave establishes the repo Team
lf wave connect --all                           # all nested Waves reuse it
lf wave sync --wave infra                    # refresh the local SQLite snapshot
lf wave status infra --no-sync          # deterministic cache-only read
lf task create --wave infra --title "Daemon data integrity"
lf task complete 1207... --summary "Dark mode delivered"
```

A managed Project belongs to exactly one Initiative and exactly the repository
Team. Project titles include the canonical Wave ancestry for orientation
(`Survival / Infrastructure — Gmail`), but stable ids and Project membership —
never titles or issue prefixes — resolve Work.

## Tasks

Every concrete file-writing change begins with a Linear task and runs as a
durable Task Work in its own stable sibling worktree:

```bash
lf task create --run --wave <wave> --title "add retry to token refresh"
pbpaste | lf task create --run --wave incidents
lf task checkout INF-123
lf --task INF-123 research "write scratch/retry-analysis.md"
lf task run INF-123
lf task run INF-124 --stack-on INF-123     # dependent work before the parent merges
lf task run INF-125 --flow incident
```

Task Work advances through one active remote branch and PR to `main`. Its
Project's Flow supplies the default; `--flow` selects any other template.
Launch creates an invocation containing the expanded graph and its execution
state. Source edits and chapter transfers do not change that captured graph.
Finished and replaced invocations remain history; the Task has at most one
managed FlowSession. Completion leaves Task Work open until an explicit
completion or delivery operation settles it.

Task context includes the Wave's `GOAL.md` and `MEMORY.md` plus its Project's
KRs and targets. Explicit PR rotation selects the next serial branch while
preserving the Task's worktree directory.

AgentSessions and FlowSessions own typed nullable Task/Wave ancestry.
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
steering, resume and recovery use the same commands for people and agents:
[The Agent API → Steer](agent-api.md#steer).

```bash
lf pr land --next parser-proof   # merge this PR, then rotate to the next
lf pr land -c                    # merge this PR, then complete the Task
lf task complete INF-124 --summary "investigation recorded"   # no PR needed
```

`task complete` also finishes planning-only Tasks without creating a checkout.
It records the summary once in Linear; repeating the command preserves it.
Placed Tasks still require a clean checkout and settled PRs. If their local
completion reports pending PM writeback, repeat the same command to reconcile
Linear without changing the original completion. Canceled and duplicate issues
cannot be changed to completed through this command.

`task complete` also finishes planning-only Tasks without creating a checkout.
It records the summary once in Linear; repeating the command preserves it.
Placed Tasks still require a clean checkout and settled PRs. If their local
completion reports pending PM writeback, repeat the same command to reconcile
Linear without changing the original completion. Canceled and duplicate issues
cannot be changed to completed through this command.

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

The Wave reads its chapter and Tasks with `lf wave status --no-sync`, judges the
KR evidence, and starts Task Work for every independent
file-writing change. Each shipped PR folds into memory and closes its task.

## Next

[The Agent API →](agent-api.md) · [Conducting →](conducting.md) · [Get Started →](getting-started.md)

## Reference

[Configuration](config.md) · [Troubleshooting](troubleshooting.md)
