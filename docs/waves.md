# Waves

A wave is a named agent with a goal. Start one, steer it, and inspect the work
it chose:

```bash
lf start shipper
lf --wave <wave> wave/operate "invoices first"
lf status shipper
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
lf start shipper                       # explicitly start the Wave on this machine
lf --wave <wave> wave/operate "invoices first"
lf status shipper                      # its current chapter and Tasks
lf pause shipper                       # refuse new turns; keep listening and queueing
lf resume shipper                      # start the next queued turn
lf stop shipper                        # stop this Wave; sibling Waves keep running
```

(`lf wave serve shipper` runs one Wave listener in the foreground until Ctrl-C. Use
it while developing a goal; `lf start` normally asks the Home's shared keeper
to serve the Wave.)

## The planning model

Open a Wave to see its enduring objective and current KRs, targets, and Tasks directly. Waves are
durable responsibilities with memory, cadence, chat, and metrics. Each has one
current chapter plan, stored in an internal Linear Project.

At a chapter boundary that Project is replaced. Its metric targets, KRs, and Flow
recommendation start fresh. Started unfinished Tasks move with the same issue,
worktree, PR, and Flow. Untouched backlog is canceled; completed Tasks stay in
history. Missing evidence never authorizes abandonment.

Retry an interrupted rotation with the same chapter ID. Before activation,
preview rereads predecessor plans and Tasks, including newly filed work,
without changing the saved receipt. After activation, the recorded boundary
stays fixed and retries reconcile its pending operations. Refreshed completion
remains historical even after local backlog retirement; conflicting start
evidence leaves the transition unresolved instead of canceling the Task again.
Retry previews include pending recovery of moves and cancellations whose
provider responses were lost, using the same dispositions as application.
If someone reassigns a Task outside the recorded chapter transition, preview
and retry report a membership conflict. Reconcile its parent before retrying;
rotation leaves that Task's provider state unchanged.
If a known Task is missing from the provider's list, rotation reads it by issue
identity. Unavailable evidence stops cutover; it never silently drops the Task.
If the current chapter itself is missing from the portfolio list, PM reads
recover its content and Tasks using the recorded chapter identity. An unreadable
chapter stops rotation before provider changes; historical chapters stay closed.
Rotation archives each recorded predecessor by identity. A missing portfolio
row is not proof of archival; retries recognize an archive that succeeded
before its response was lost.

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

`owner` and `home` are independent, optional automatic-start filters. `owner`
names the OS user that should run the Wave. `home` accepts this machine's
HomeId, hostname, or IP address. Omit either field to leave that dimension
unrestricted. Locally placed Waves are enabled by default. `lf stop <name>`
disables the Wave in this Home's registry; an explicit `lf start <name>` enables
and starts it here. Neither command edits the goal.

Pause turn execution without stopping the listener:

```bash
lf pause shipper --json    # {"wave":"shipper","paused":true}
lf ls                      # TURNS shows paused independently from LIVE
lf resume shipper
```

Pause writes `paused: true` into the canonical `GOAL.md` frontmatter; resume
removes it because enabled turns are the default. Messages continue to queue,
and heartbeat and cron turn starts wait. For a Wave served from another Home,
run the same command there: `lf ssh <home-id> pause shipper`.

Builtin goals resolve by name, so the five Viable System Model charters ship
as `s1`…`s5`:

```bash
lf wave serve s3            # the s3 (control) charter
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
{"metric_targets":[{"metric_id":"task-loop-trust","target":{"kind":"at_least","value":1}}],"flows":{"recommended":null},"krs":[]}
```

Apply it with `lf wave new-chapter --wave <wave> --chapter <id> --plan plan.json`.
Change the current plan with `lf wave update-plan --wave <wave> --plan plan.json`.
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
service, then start the Wave:

```bash
doppler secrets set LF_DISCORD_TOKEN > /dev/null
doppler run -- lfd install
lf start product
```

The service file retains only the non-secret Doppler project and config names.
`lfd` resolves the token from Doppler once at boot and keeps it inside the
trusted Home process. The Wave resident and its provider children never
inherit it. Reload the service after changing or rotating the token. For a
foreground listener without `lfd`, inject the same secret for that process:

```bash
doppler run -- lf wave serve product
```

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
local message. Human and agent speech appears only after Discord returns its
provider message id. Deterministic send intents, cursors, source links, and
direct provider receipts remain durable; harness conversation and usage
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
or not. `update-wave` owns
deliberate end-of-work curation: merge durable context into the existing
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
lf ls --json                  # Wave ids and their current Homes
lf start shipper              # start one Wave on this machine
lf start                      # start eligible repo Waves on this machine
lf pause shipper              # keep serving but refuse new turns
lf resume shipper             # enable queued and future turns
lf stop shipper               # stop this machine's Wave
lf work disable task task_... # exclude one Task from automatic pursuit
lf work enable task task_...  # restore eligibility
```

With a name, `lf start` is an explicit instruction: it records this machine as
the Wave's Home, enables it in the local registry, and starts it. Without names,
it starts only enabled Waves whose optional `owner` and `home` policy matches
this process and whose recorded placement is already local. New Project or Task
Work inherits its parent's recorded Home once. Record a different Wave Home
directly:

```bash
lf work place wave <wave-id> <home-id>
```

Placement is planning state. Run records do not own it or prove that a process
can be signaled; only an exact runtime owner may stop its process.

A Wave name is local to its canonical repository. Its UUID remains stable when
the name or repository changes, so relocation is separate from Home placement:

```bash
lf work relocate wave <wave-id> --name platform
lf work relocate wave <wave-id> --repo ../moved-repository
```

Stop the Wave and its descendants first. Relocation preserves its Linear
Initiative projection, Work state, journal, authored files, and Home placement.
It does not copy or rewrite Home-local Run records. A repository move carries
the complete Wave chord, and renaming a Wave carries descendants whose authored
paths are nested below it. Configured source and target PM Teams must match;
use `lf pm reteam` for an intentional provider ownership change. Divergent
target files fail closed instead of being merged, and retry finishes cleanup
if the locator committed before a crash.

`lfd` is the one keeper process per Home. Its in-process `WaveHost` starts every
eligible Wave known to the local store across repositories, then reconciles
every 30 seconds. Starting or stopping one Wave does not kill `lfd` or disturb
sibling Waves.

`WaveHost` is server machinery, not an agent: it makes no model calls and
chooses no work. Each hosted Wave body is the agent with a goal, memory, and
conversation.

`lf stop <wave>` records `enabled = false` in this Home's SQLite registry. A new
`lfd` process reads the same control and leaves the Wave off. `lf start <wave>`
enables it again. Change `owner`, `home`, or placement to move ownership; use
`lf work enable|disable` to control otherwise assigned Work without producing a
repository diff.

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
lf work place wave <wave-id> <home-id>    # record origin-side planning state
lf ssh <home-id> start shipper
```

After bootstrap, address the authority rather than its current hostname. Every
HomeId-addressed hop makes the target prove its identity:

```bash
lf ssh <home-id> status shipper --json
lf home probe shipper
```

`lf start shipper` starts here; `lf ssh <home-id> start shipper` starts there.
Foreground SSH work can use origin and target subscription accounts. Durable
residents shed forwarded authority before detaching and use credentials
installed on their machine.

Observation follows the same rule. `lf status`, `lf runs`, and `lf usage` read
this Home; prefix them with `lf ssh <home-id>` to read another one. Homes do not
silently replicate or aggregate Run records.

See [Get Started → Go Remote](getting-started.md#go-remote) and
[Security → Account authority over SSH](security.md#understand-account-authority-over-ssh).

## Chapter plans and KRs

```bash
lf status infra --json
lf wave update-plan --wave infra --plan plan.json
```

`plan.json` contains the complete current plan:

```json
{"metric_targets":[],"flows":{"recommended":"task-design"},"krs":[{"text":"A new contributor ships a change using the architecture guide without an undocumented dependency.","holds":false}]}
```

The Wave objective names who benefits and what improves. Chapter KRs prove observable outcomes
across a stated window. Update the current plan explicitly; a new chapter never
copies the previous content or checked KRs.

## Linear

Tasks live in Linear; there are no local task lists. A wave maps to an
Initiative, each chapter to an internal Linear Project, each task to an Issue. Connect
once — `lf pm init` links or creates the Wave Initiative and establishes one
repository Team in `.lf/config.yaml`. Every Wave reuses that Team and issue-key
namespace. Don't paste ids by hand.

```bash
lf pm init --wave infra --team-key LOO     # first Wave establishes the repo Team
lf pm init --all                           # all nested Waves reuse it
lf pm sync --wave infra                    # refresh the local SQLite snapshot
lf pm show --wave infra --no-sync          # deterministic cache-only read
lf pm task create --wave infra --title "Daemon data integrity"
lf pm task done --id 1207... --pr "https://github.com/acme/app/pull/42"
```

A managed Project belongs to exactly one Initiative and exactly the repository
Team. Project titles include the canonical Wave ancestry for orientation
(`Survival / Infrastructure — Gmail`), but stable ids and Project membership —
never titles or issue prefixes — resolve Work.

## Tasks

Every concrete file-writing change begins with a Linear task and runs as a
durable Task Work in its own stable sibling worktree:

```bash
lf task start --wave <wave> "add retry to token refresh"
pbpaste | lf task start --wave incidents
lf task prepare INF-123
lf --task INF-123 research "write scratch/retry-analysis.md"
lf task run INF-123
lf task run INF-124 --stack-on INF-123     # dependent work before the parent merges
lf task run INF-125 --flow incident
```

Task Work advances through one active remote branch and PR to `main`. Its chapter
may recommend one Flow; `--flow` overrides it for this Task worker. Launch pins
the complete Flow definition and exact position until it completes or parks at
a human boundary. Completion clears that Flow state and leaves the Task open
for a later worker or explicit delivery command. After a merge or abandonment,
Loopflow rotates the worktree onto the next branch. The Task inherits the
wave's `GOAL.md` and `MEMORY.md` plus its current chapter KRs and metric targets.

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

The Wave reads its chapter and Tasks with `lf pm show --no-sync`, judges the
KR evidence, and starts Task Work for every independent
file-writing change. Each shipped PR folds into memory and closes its task.

## Next

[The Agent API →](agent-api.md) · [Conducting →](conducting.md) · [Get Started →](getting-started.md)

## Reference

[Configuration](config.md) · [Troubleshooting](troubleshooting.md)
