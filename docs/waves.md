# Waves

A Wave owns an enduring responsibility, its current plan, and a conversation.
Open its chat or run a bounded planning pass:

```bash
lf chat --follow -w shipper
lf --wave <wave> wave/operate "invoices first"
lf wave status shipper
```

The Wave remembers what it learns, works the next blocker, spins off durable
Tasks when parallelism earns it, and stays steerable. It is planning and
coordination, not a central process owner: the Home running each harness keeps
that launch's evidence locally.

Three reviewed surfaces author a wave:

| Path | Holds |
|------|-------|
| **`wave/<name>/GOAL.md`** | The wave's intent and loop prompt — what it's for, how it judges progress |
| **`wave/<name>/MEMORY.md`** | What the wave remembers between loops — curated through reviewed file edits |
| **`wave/<name>/metrics/*.md`** | Wave-owned live metric contracts — meaning, window, and freshness |

Waves live in **Loopflow** (macOS): open the repository and select the Wave to
put its conversation beside its work map. The same controls exist from the
CLI:

```bash
lf chat --follow -w shipper            # connect to its conversation
lf --wave shipper wave/operate "invoices first"
lf wave status shipper                  # current chapter and Tasks
```

Opening chat connects its internal service. Task execution and scheduled work
do not depend on keeping a chat window open.

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

`owner` and `home` are independent, optional automatic-start filters. `owner`
names the OS user that should run the Wave. `home` accepts this machine's
HomeId, hostname, or IP address. Omit either field to leave that dimension
unrestricted. Home placement remains explicit; opening chat does not move it.

Builtin goals resolve by name, so the five Viable System Model charters ship
as `s1`…`s5`:

```bash
lf chat --follow -w s3            # the s3 (control) charter
```

Writing a goal well — the weight of each section, frontmatter fields, KR
craft — is covered in [Authoring → Goals](authoring.md#goals).

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
dated readings for `lf wave status <wave> --chapter <id> --json`. The Wave objective
stays in `GOAL.md`; chapter plans have no second objective.

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

### Discord chat

Wave Chat is local by default. Omitting `chat` (or explicitly choosing
`provider: local`) keeps messages in the Wave journal and exposes the local
composer in Loopflow:

```yaml
chat:
  provider: local
```

Bind one existing guild text channel to replace the local backing for future
messages:

```yaml
# wave/product/GOAL.md
---
chat:
  provider: discord
  home_id: "home_0123456789abcdef0123456789abcdef"
  guild_id: "123456789012345678"
  channel_id: "234567890123456789"
---
```

Store the bot token in the Home daemon repository's Doppler config, reload the
service, then open chat:

```bash
doppler secrets set LF_DISCORD_TOKEN > /dev/null
doppler run -- lfd install
lf chat --follow -w product
```

The service file retains only the non-secret Doppler project and config names.
`lfd` resolves the token from Doppler once at boot and keeps it inside the
trusted Home process. The Wave resident and its provider children never
inherit it. Reload the service after changing or rotating the token.

The bot needs **View Channel**, **Read Message History**, **Send Messages**, and
**Add Reactions**, with Message Content enabled in the Discord developer portal.
A backing change takes effect when the listener restarts and starts one durable
conversation segment. Earlier local segments stay selectable and read-only; the
API and `--epoch` flag retain their exact segment ids.

Discord is the transcript authority while its segment is active. The Gateway
pushes new messages to Loopflow; every reconnect first catches up over REST from
the journal's committed cursor. Each external message is journaled before the
cursor advances. The Wave reads that durable channel tail, including its own
replies, so restart recovery needs no consumed-message queue.

The Mac composer and `lf chat "text"` post through the bot, visibly prefix the
message with the Wave name, and preserve message or bare interrupt intent
when the provider echo reaches the listener. The Open in Discord action stays
beside the native composer. A provider failure never falls through to a hidden
local message. A message appears only after Discord returns its provider message
id, whether it came from a user or an agent. Deterministic send intents, cursors,
source links, and direct provider receipts remain durable; harness conversation and usage
evidence belongs to Home-local Run records.

`home_id` is the portable binding owner (`lf home id`) and must match the Wave's
durable placement. Another Home fails before contacting Discord, while an
OS-held lease prevents another checkout on the owner Home from observing the
same channel.

Read the current conversation segment or an earlier one explicitly:

```bash
lf chat --history --json --wave product
lf chat --history --json --wave product --epoch chat-epoch-42
```

### Memory

`MEMORY.md` is durable working context agents curate as Wave work moves —
decisions, dead ends, what a downstream task should know. It is an ordinary
reviewed repository file:

```bash
$EDITOR wave/shipper/MEMORY.md
```

The file is the whole memory surface — read and edit it directly, running Wave
or not. `realign` curates it: merge durable context into the existing
structure, correct stale entries, and drop transient Run detail. When a task ships,
its context folds forward into memory and the remaining Linear tasks — fold,
don't drop.

### Home

A **Home** is a stable machine identity. Work records its execution placement;
the Home records its currently observed route and keeps its own process,
journal, and Run evidence. Changing a hostname or SSH route does not change
that identity, and the record never opens SSH by itself.

```bash
lf home id                    # this machine's stable HomeId
lf wave list --json                  # Wave ids and their current Homes
lf chat --follow -w shipper    # connect to chat on this Home
lf wave place <wave-id> <home-id>
```

New Project and Task Work inherit their parent’s recorded Home once. Placement
expresses where work belongs; it is separate from chat availability.

Placement is planning state. Run records do not own it or prove that a process
can be signaled; only an exact runtime owner may stop its process.

A Wave name is local to its canonical repository. Its UUID remains stable when
the name or repository changes, so relocation is separate from Home placement:

```bash
lf wave relocate <wave-id> --name platform
lf wave relocate <wave-id> --repo ../moved-repository
```

Stop the Wave and its descendants first. Relocation preserves its Linear
Initiative projection, Work state, journal, authored files, and Home placement.
It does not copy or rewrite Home-local Run records. A repository move carries
the complete Wave chord, and renaming a Wave carries descendants whose authored
paths are nested below it. Configured source and target PM Teams must match;
use `lf repo reteam` for an intentional provider ownership change. Divergent
target files fail closed instead of being merged, and retry finishes cleanup
if the locator committed before a crash.

`lfd` is the one keeper process per Home. Its in-process `WaveHost` starts every
eligible Wave known to the local store across repositories, then reconciles
every 30 seconds. Starting or stopping one Wave does not kill `lfd` or disturb
sibling Waves.

`WaveHost` is server machinery, not an agent: it makes no model calls and
chooses no work. Each hosted Wave body is the agent with a goal, memory, and
conversation.

Chat has no enablement or pause switch. Its listener is managed internally.

`home: localhost`, `home: 127.0.0.1`, and `home: ::1` always match the current
machine. Loopflow also matches its stable HomeId, hostname and short hostname,
and local interface addresses, including a directly assigned public address.
An `lf ssh <host> ...` invocation additionally treats the SSH destination as
this machine for that foreground command. Prefer the HomeId for machines behind
NAT or with changing public addresses.

Register a remote Home by asking that machine for its own identity, then record
the route locally and start the Wave there:

```bash
lf ssh jack@mini.local home id --json
lf home observe <home-id> ssh://jack@mini.local
lf wave place <wave-id> <home-id>    # record origin-side planning state
lf ssh <home-id> chat --follow -w shipper
```

After bootstrap, address the authority rather than its current hostname. Every
HomeId-addressed hop makes the target prove its identity:

```bash
lf ssh <home-id> wave status shipper --json
lf wave probe shipper
```

Use `lf ssh <home-id> chat --follow -w shipper` to chat on the selected machine.
Foreground SSH work can use origin and target subscription accounts. Durable
residents shed forwarded authority before detaching and use credentials
installed on their machine.

Observation follows the same rule. `lf wave status`, `lf runs`, and `lf usage` read
this Home; prefix them with `lf ssh <home-id>` to read another one. Homes do not
silently replicate or aggregate Run records.

See [Get Started → Go Remote](getting-started.md#go-remote) and
[Security → Account authority over SSH](security.md#understand-account-authority-over-ssh).

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

The wave stays steerable while several independent tasks run — task events
enter its inbox as typed observations and wake it once. Steering, status,
resume, and recovery are the same verbs agents use:
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
