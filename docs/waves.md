# Waves

```bash
lf --wave shipper wave/operate "invoices first"
lf status shipper
```

A Wave keeps an objective, memory, cadence, budget and metrics across Tasks.
Each invocation makes one finite planning pass. Tasks own implementation and
Flow execution; each Home retains its Run evidence.

| Path | Holds |
|------|-------|
| `wave/<name>/GOAL.md` | Objective, operating guidance and schedules |
| `wave/<name>/MEMORY.md` | Curated decisions and lessons |
| `wave/<name>/metrics/*.md` | Metric meaning, window and freshness |

In Loopflow on macOS, select a Wave to read its plan, Tasks and Runs. Start a
Session for a conversation, or invoke `wave/operate` for a bounded planning pass.

## The planning model

```bash
lf roadmap --json
lf wave history --wave infrastructure --json
lf status infrastructure --chapter 2026-09 --json
```

Open a Wave for its objective, current Tasks, KRs and metric targets. Open
chapter history to inspect its plan and dated evidence at an earlier boundary.
A Wave's memory, cadence, budget, chat and metric instruments endure across
those boundaries.

A Chapter is a repository-wide planning clock. Every Wave shares the current
Chapter. Each (Wave, Chapter) pair has exactly one Project holding that Wave's
Tasks, KRs, targets and Flow template. A Wave with no work has an empty Project.
Linear Projects hold the plans; the repository's Chapter links them together.
Current workspace navigation is Wave → Task. History exposes the Project for
a selected Wave and Chapter without making Project another operator.

Rotate every Wave together:

```bash
lf wave new-chapter --chapter 2026-10 --plan chapter.json --dry-run --json
lf wave new-chapter --chapter 2026-10 --plan chapter.json --json
```

`chapter.json` contains a complete plan for every Wave, keyed by stable Wave ID:

```json
{
  "plans": {
    "<infrastructure-wave-id>": {"flow":"feature","metric_targets":[],"krs":[]},
    "<product-wave-id>": {"flow":"feature","metric_targets":[],"krs":[]}
  }
}
```

The preview lists all affected Waves and Task dispositions. Empty plans are
explicit; omitting a Wave cannot silently retire its backlog. Each successor
Project gets fresh KRs, targets and Flow selection. Started unfinished Tasks
keep their identity, worktree, PR and captured invocation when transferred;
untouched backlog is deleted from Linear and retired locally; completed Tasks remain historical.
Local Task and PR history survive deletion.
Unavailable evidence never counts as untouched work.

Retry an interrupted operation with the same Chapter ID. Preparation refreshes
provider membership and resolves missing listed Tasks by stable identity.
Activation changes the repository's current Chapter once all Wave plans are
prepared; it never exposes a successful half-rotated local plan. External Task
moves and Project retirement can remain visibly pending after activation.
Retries use the recorded boundary, settle lost responses against provider
state, and preserve unexpected external reassignments for reconciliation.
A lost deletion response requires acknowledgement or explicit provider trash
evidence; absence from a list or an unreadable issue leaves it unresolved.
Historical targets, KR judgments and dated readings stay frozen even when a
transferred Task ships later.

## The Goal

`GOAL.md` is the loop surface: frontmatter carries machine config, the body is
the prompt the wave runs each loop.

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

Work placement records the owning Home. `lf wave enable|disable` changes local
eligibility without changing the goal or stopping a running Task. Use `lf cron`
for scheduled Runs and `lf --wave <name> wave/operate` for an immediate pass.

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
`lf wave update-plan --wave <wave> --plan plan.json`. For a new Chapter, include
it in the repository-wide [chapter plan](#the-planning-model).
An omitted metric has no target in that chapter; its observations remain visible
without a pass/fail verdict. Changing a target preserves instrument identity,
revision, and measurement history. Rotation freezes the previous targets and
dated readings for `lf status <wave> --chapter <id> --json`. The Wave objective
stays in `GOAL.md`; chapter plans have no second objective.


```bash
lf status <wave>            # owner, value, target, window, freshness, reason
lf status <wave> --json     # the shared metric_portfolio DTO
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
Run, and replies with that Run's final answer. Mentions are disabled in replies.
The token is `LF_DISCORD_TOKEN`, injected through Doppler and removed from
provider child environments. The bot needs View Channel, Read Message History,
Send Messages and access to message content.

The cursor exists only in memory. Every start skips existing channel history;
restarting after a failure does not replay missed messages. The bridge uses the
channel binding; `guild_id` is configuration metadata, not a process owner. Run a single bridge per channel to avoid duplicate replies.
There is no local Wave transcript, inbox, listener or automatic service startup.
Use Sessions for native conversations and `lf runs --wave product` for Run history.

### Memory

`MEMORY.md` is durable working context agents curate as Wave work moves —
decisions, dead ends, what a downstream task should know. It is an ordinary
reviewed repository file:

```bash
$EDITOR wave/shipper/MEMORY.md
```

The file is the whole memory surface — read and edit it directly, running Wave
or not. `update-wave` owns
deliberate end-of-work curation: merge durable context into the existing
structure, correct stale entries, and drop transient Run detail. When a task ships,
its context folds forward into memory and the remaining Linear tasks — fold,
don't drop.

### Home

```bash
lf home id
lf home observe <home-id> ssh://jack@mini.local
lf wave place <wave-id> <home-id>
lf ssh <home-id> --wave shipper wave/operate
lf ssh <home-id> status shipper --json
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
journals or Home-local Runs. Source and target PM Teams must match; use
`lf repo reteam` for a provider ownership change.

See [Homes and processes](architecture/homes.md) and
[Security](security.md#understand-account-authority-over-ssh).

## Chapter plans and KRs

```bash
lf status infra --json
lf wave update-plan --wave infra --plan plan.json
```

`plan.json` contains the complete current plan:

```json
{"metric_targets":[],"flow":"task-design","krs":[{"text":"A new contributor ships a change using the architecture guide without an undocumented dependency.","holds":false}]}
```

The Wave objective names who benefits and what improves. Chapter KRs prove observable outcomes
across a stated window. Update the current plan explicitly; a new chapter never
copies the previous content or checked KRs.

## Linear

Tasks live in Linear; there are no local task lists. A Wave maps to an
Initiative, its Project for each Chapter maps to a Linear Project, and each
Task maps to an Issue. Chapter itself is the repository's clock. Connect
once — `lf wave connect` links or creates the Wave Initiative and establishes one
repository Team in `.lf/config.yaml`. Every Wave reuses that Team and issue-key
namespace. Don't paste ids by hand.

```bash
lf wave connect --wave infra --team-key LOO     # first Wave establishes the repo Team
lf wave connect --all                           # all nested Waves reuse it
lf wave sync --wave infra                    # refresh the local SQLite snapshot
lf status infra --no-sync          # deterministic cache-only read
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
lf task prepare INF-123
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
current root invocation. Completion leaves Task Work open until an explicit
completion or delivery operation settles it.

Task context includes the Wave's `GOAL.md` and `MEMORY.md` plus its Project's
KRs and targets. Explicit PR rotation selects the next serial branch while
preserving the Task's worktree directory.

Runs carry nullable Task/Wave fields; Sessions read those fields through their
Runs. Launching `lf` in a registered Task checkout binds automatically unless
an explicit selector overrides it. A later bind can attach a conversation to
a done or landed Task without reopening Work. Assignment is permanent and
requires confirmation of the exact target; an existing Task cannot change.
See [Sessions](lf.md#sessions)
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

The Wave reads its chapter and Tasks with `lf status --no-sync`, judges the
KR evidence, and starts Task Work for every independent
file-writing change. Each shipped PR folds into memory and closes its task.

## Next

[The Agent API →](agent-api.md) · [Conducting →](conducting.md) · [Get Started →](getting-started.md)

## Reference

[Configuration](config.md) · [Troubleshooting](troubleshooting.md)
