# Waves

Use a [Wave](glossary.md#loopflows-words) when an objective will take several
Tasks and needs attention over time. A Wave keeps its objective, memory,
conversation, schedule, and measurements as its plans change.

In the Mac app, select a Wave in a repository to see its conversation beside
its work. A [repository](glossary.md#borrowed-from-software-engineering) is a
project folder tracked by Git. The commands below control the same Wave.
Replace `shipper` or `<wave>` with its name.

```bash
lf start shipper                       # start the Wave on this machine
lf --wave <wave> wave/operate "invoices first"
lf status shipper                      # current chapter and Tasks
lf pause shipper                       # queue messages without starting new turns
lf resume shipper                      # allow queued and future turns
lf stop shipper                        # stop this Wave; other Waves keep running
```

A [Task](glossary.md#loopflows-words) is one piece of work with a finish line.
The Wave chooses useful work and keeps what it learns. Each computer running
Loopflow, called a [Home](glossary.md#loopflows-words), keeps its own launch
records; the Wave does not gather every machine's records into one place.

The Wave's instructions are ordinary files reviewed with the repository's code:

| Path | Holds |
|------|-------|
| **`wave/<name>/GOAL.md`** | The objective and instructions for each turn |
| **`wave/<name>/MEMORY.md`** | Decisions and lessons worth keeping between turns |
| **`wave/<name>/metrics/*.md`** | Definitions of measurements, including their time windows and how old a reading may be |

For terminal development, `lf wave serve shipper` keeps the Wave listener in
the foreground until Ctrl-C. A listener receives messages and starts turns.
`lf start` normally asks the Home's shared background service to do this, so
the Wave can keep listening after the command returns.

## The planning model

Open a Wave to see its objective and current [chapter](glossary.md#loopflows-words):
a plan containing Tasks, measurement targets, and key results (KRs). A
[KR](glossary.md#loopflows-words) states an outcome and the evidence that would
prove it over a stated period. One internal Linear Project holds that plan;
ordinary navigation is Wave → Task.

Starting a chapter replaces the current plan. KRs, targets, and the recommended
[Flow](glossary.md#loopflows-words)—the sequence of steps for Tasks—start fresh.
Started unfinished Tasks keep the same issue, worktree, pull request, and Flow.
A [worktree](glossary.md#borrowed-from-software-engineering) is a separate
checkout for a Task's edits; a pull request (PR) proposes those edits for merge.
Untouched backlog is canceled, completed Tasks remain in history, and missing
evidence never justifies abandoning work.

Use `lf wave new-chapter --wave <wave> --chapter <id> --plan plan.json` to
start a chapter. Choose a new chapter identifier for `<id>` and prepare the
plan as described under [Chapter plans and KRs](#chapter-plans-and-krs).
Retry an interrupted transition with the **same chapter ID** so Loopflow can
finish the recorded operation:

- Before activation, the preview rereads the old plan and Tasks, including
  newly filed work, without changing the saved transition record.
- After activation, the boundary stays fixed. Retries finish pending moves
  and cancellations, including requests whose replies were lost. The preview
  shows the same planned outcomes that applying the transition uses.
- A Task subsequently found completed remains historical, even if it was
  previously retired locally. Conflicting evidence that it started leaves
  the transition unresolved rather than canceling it again.
- If someone moved a Task outside this transition, preview and retry report
  a membership conflict and leave its Linear state alone. Reconcile which
  chapter owns it before retrying.
- If a Task or current chapter is absent from a list, Loopflow reads its
  recorded identifier directly. Unavailable evidence stops the transition;
  historical chapters stay closed.
- Each old chapter is archived by identifier. A missing list entry does not
  prove archival; a retry recognizes an archive that succeeded before its
  reply was lost.

## The Goal

Edit `GOAL.md` to change the Wave's instructions. Its
[frontmatter](glossary.md#borrowed-from-software-engineering), the YAML block
between `---` lines, holds settings. The body is the prompt used on each turn.

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

Make routine changes directly; write a scratch design first when a change
affects storage, account access, or interfaces other programs use.
```

The [operating system (OS)](glossary.md#borrowed-from-software-engineering)
manages users and running programs. A hostname or IP address locates a machine
on a network; a HomeId identifies it even when that address changes.
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
and scheduled turn starts wait. A
[heartbeat](glossary.md#loopflows-words) is a periodic check for work;
[cron](glossary.md#borrowed-from-software-engineering) specifies scheduled times. For a Wave served from another Home,
run the same command there: `lf ssh <home-id> pause shipper`.

Built-in goals can be selected by name. The
[Viable System Model](glossary.md#loopflows-words) groups responsibility into
operations, coordination, control, intelligence, and identity. Its five goal
templates are `s1`…`s5`:

```bash
lf wave serve s3            # the s3 (control) charter
```

Writing a goal well — the weight of each section, frontmatter fields, KR
craft — is covered in [Authoring → Goals](authoring.md#goals).

### Live metrics

Use a [metric](glossary.md#loopflows-words) to measure progress repeatedly.
Its contract defines what is counted, over which window, and how fresh a reading
must be. A target says what result this chapter needs. Keeping them separate
lets the measurement's history survive a new target or chapter.

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
The contract does not run commands or queries, schedule collection, store
secrets, or repeat a KR. Its `instrument` names the product code that collects
and totals observations for that contract revision. Writing a contract alone
does not implement a new collector.

The example below uses [JSON](glossary.md#borrowed-from-software-engineering),
a structured data format. `at_least` means the measured value must reach the
stated minimum. Set targets in the chapter plan, not the metric contract:

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
lf status <wave> --json     # structured measurements in the metric_portfolio field
lf roadmap --json           # the same measurements on every Wave row
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

Choose [Discord](glossary.md#borrowed-from-software-engineering) when the
conversation should live in an existing server's text channel. Discord calls
a server a *guild*. Its bot is the account Loopflow uses to read and send messages.
Configure that channel for future messages:

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

The bot's token is a secret credential. Store it with
[Doppler](glossary.md#borrowed-from-software-engineering), the secret manager,
in the Home service repository's configuration. A
[daemon](glossary.md#borrowed-from-software-engineering) is a background service;
`lfd` is Loopflow's. Reload it, then start the Wave:

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
Changing the chat provider takes effect when the listener restarts and starts
one saved conversation segment. Earlier local segments stay selectable and
read-only; the API (the interface programs call) and `--epoch` use their exact
segment IDs.

While the Discord segment is active, Discord holds the conversation. Its
Gateway connection delivers new messages. After a reconnect, Loopflow first
fetches missed messages through Discord's web API using HTTP requests and its last saved
position. Each message is saved in the local journal before that position
advances. The Wave reads this saved history, including its own replies.

The Mac composer and `lf chat "text"` post through the bot and prefix messages
with the Wave name. When Discord sends the message back, Loopflow preserves
whether it was a message or an interrupt. **Open in Discord** is beside the
composer. A failed Discord send does not quietly become a local message;
it appears only after Discord returns its message ID. Send records, saved
positions, source links, and Discord replies survive restarts. The coding
tool's conversation and usage stay in the executing Home's Run records.

`home_id` comes from `lf home id` and must match the Wave's recorded placement.
Another Home refuses before contacting Discord. An OS-held lock also prevents
a second checkout on the owner Home from listening to the same channel.

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

Read and edit the file whether or not the Wave is running. The `update-wave`
skill curates it at the end of work: retain useful decisions, correct stale
entries, and remove details that only mattered during one Run. Keep unfinished
work in Linear Tasks so clearing a Task's scratch notes does not lose it.

### Home

A **Home** identifies one machine even when its network address changes.
[Work](glossary.md#loopflows-words), the saved planning record, names its
assigned Home. Each Home keeps its own running-process observations, journal
of events, and Run records. SSH is the connection for running commands on
another machine; recording a Home's address does not open that connection.

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

A Wave name is local to its repository. Its unique identifier (UUID) stays the same when
the name or repository changes, so relocation is separate from Home placement:

```bash
lf work relocate wave <wave-id> --name platform
lf work relocate wave <wave-id> --repo ../moved-repository
```

Stop the Wave and its child Waves and Tasks first. Relocation preserves the
Linear planning connection, Work state, journal, goal and memory files, and
Home placement. Run records stay on the Home that wrote them.

Moving a repository carries its whole group of Waves (the chord). Renaming a
Wave carries children whose files are nested below it. The old and new
repositories must use the same Linear Team; use `lf pm reteam` to change that
ownership deliberately. Conflicting destination files stop the move. If the
new location was saved before a crash, retry finishes the remaining cleanup.

`lfd` is the Home's shared background service. Its `WaveHost` component starts
eligible Waves across known repositories, then checks again every 30 seconds.
Starting or stopping one Wave leaves the others running. This hosting code
makes no AI calls and chooses no work; each Wave's agent makes those decisions.

`lf stop <wave>` saves `enabled = false` in this Home's local database. A new
`lfd` process reads the same control and leaves the Wave off. `lf start <wave>`
enables it again. Change `owner`, `home`, or placement to move ownership; use
`lf work enable|disable` to control otherwise assigned Work without producing a
repository diff.

`home: localhost`, `home: 127.0.0.1`, and `home: ::1` always match the current
machine. Loopflow also matches its stable HomeId, hostname and short hostname,
and local interface addresses, including a directly assigned public address.
An `lf ssh <host> ...` invocation additionally treats the SSH destination as
this machine for that foreground command. Prefer the HomeId for machines behind
[NAT](glossary.md#borrowed-from-software-engineering), where several machines
share one public network address, or whose public address changes.

Register a remote Home by asking that machine for its own identity, then record
the route locally and start the Wave there:

```bash
lf ssh jack@mini.local home id --json
lf home observe <home-id> ssh://jack@mini.local
lf work place wave <wave-id> <home-id>    # record origin-side planning state
lf ssh <home-id> start shipper
```

After this setup, use the Home ID so a changed hostname does not change which machine is selected. Every
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

`holds` records whether a KR is supported by the evidence. Empty target or KR
lists mean that chapter has none. The Wave objective names who benefits and
what improves. Chapter KRs prove observable outcomes
across a stated window. Update the current plan explicitly; a new chapter never
copies the previous content or checked KRs.

## Linear

[Linear](glossary.md#borrowed-from-software-engineering) stores the shared plan.
An Initiative groups a Wave's chapter Projects; a Team owns the issues, which
are Linear's Task records. The issue key is a label such as `INF-123`.
PM means planning management in commands such as `lf pm`.

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
a review boundary. Completion clears that Flow state and leaves the Task open
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

Keep each PR understandable as one change. A Task may need several PRs in
sequence, but it still needs one concrete finish line.

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
file-writing change. Keep useful conclusions in memory. A merged PR closes its Task only when
completion was requested; bare `lf pr land` leaves the Task open.

## Next

[The Agent API →](agent-api.md) · [Conducting →](conducting.md) · [Get Started →](getting-started.md)

## Reference

[Configuration](config.md) · [Troubleshooting](troubleshooting.md)
