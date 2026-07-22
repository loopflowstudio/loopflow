# Waves

A wave is a named agent with a goal. You author its intent and let it
coordinate; it remembers what it learns, works the next blocker, spins off
durable Tasks when parallelism earns it, and stays steerable the whole
time.

Two files author a wave, both living in your repo and reviewed like code:

| File | Holds |
|------|-------|
| **`wave/<name>/GOAL.md`** | The wave's intent and loop prompt — what it's for, how it judges progress |
| **`wave/<name>/MEMORY.md`** | What the wave remembers between loops — curated through reviewed file edits |

Waves live in **Loopflow** (macOS): open the repository, select the wave, and
start it beside its conversation and work map. The same controls exist from
the CLI:

```bash
lf start shipper                       # explicitly start the Wave on this machine
lf chat --steer "invoices first"       # steer the live body, else queue
lf status shipper                      # its Project → Task hierarchy
lf pause shipper                       # refuse new turns; keep listening and queueing
lf resume shipper                      # start the next queued turn
lf stop shipper                        # stop this Wave; sibling Waves keep running
```

(`lf wave shipper` runs one Wave listener in the foreground until Ctrl-C. Use
it while developing a goal; `lf start` normally asks the Home's shared keeper
to serve the Wave.)

## The planning model

Three nouns, kept distinct by kind rather than size:

| Noun | What it means | What it owns |
|------|---------------|--------------|
| **Wave** | Durable operating context | Memory, cadence, chat, and project selection |
| **Project** | Measured bet inside exactly one wave | Definition, KRs, and closure criteria |
| **Task** | Concrete work that advances a project | One implementation step, investigation, doc, or shipped change |

Keep the hierarchy shallow. Every project belongs to one wave; projects don't
contain projects and don't own memory or cadence. If a project seems to need
subprojects, split it into siblings, promote the durable context into a wave,
or demote the pieces into tasks.

```text
Product wave
  Loopflow API project
    Linear tasks
  Wave Chat project
    Linear tasks
```

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

## Measures

- **Quality**: fresh-store tests cover every live persistence path.
- **Done means**: a landed PR of real product code, Linear task closed and PR-linked.

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
lf wave s3            # the s3 (control) charter
```

Writing a goal well — the weight of each section, frontmatter fields, KR
craft — is covered in [Authoring → Goals](authoring.md#goals).

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
doppler run -- lf wave product
```

The bot needs only **View Channel**, **Read Message History**, and **Send
Messages**, with Message Content enabled in the Discord developer portal. A
backing change takes effect when the listener restarts and starts one durable
conversation epoch. Earlier local epochs stay selectable and read-only.

Discord is the transcript authority while its epoch is active. Loopflow reads
bounded history from Discord on demand and stores no duplicate presentation
transcript. The Mac composer and `lf chat "text"` post through the bot, visibly
prefix the message with the Wave name, and preserve message, steer, or interrupt
intent when the provider echo reaches the resident. The Open in Discord action
stays beside the native composer. A provider failure never falls through to a
hidden local message. Human and agent speech appears only after Discord returns
its provider message id. Every API message names its epoch and exact local
journal event or Discord message source.

Loopflow retains source-linked inputs until the resident consumes them,
execution turns, deterministic send intents, cursors, and provider receipts as
Run evidence. `home_id` is the binding's durable owner (`lf home id`). Another
Home fails before contacting Discord, while an OS-held lease prevents another
checkout on the owner Home from consuming the same channel.

Read the active epoch or an earlier one explicitly:

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

A **Home** is a stable machine identity. Work records the Home where it runs;
the Home records its currently observed route. Changing a hostname or SSH route
does not change that location record, and the record never opens SSH by itself.

```bash
lf home id                    # this machine's stable HomeId
lf ls --json                  # Wave ids and their current Homes
lf start shipper              # start one Wave on this machine
lf start                      # start eligible repo Waves on this machine
lf pause shipper              # keep serving but refuse new turns
lf resume shipper             # enable queued and future turns
lf stop shipper               # stop this machine's Wave
lf work disable task task_... # exclude one Task without stopping its current Run
lf work enable task task_...  # restore eligibility
```

With a name, `lf start` is an explicit instruction: it records this machine as
the Wave's Home, enables it in the local registry, and starts it. Without names,
it starts only enabled Waves whose optional `owner` and `home` policy matches
this process and whose recorded placement is already local. New Project or Task
Work inherits its parent's recorded Home once. Record a different Home only
while the Wave has no live Run:

```bash
lf work place wave <wave-id> <home-id>
```

Project and Task movement stays closed while their Runs remain the
executor. The shared Run supervisor will open that placement boundary without
another Work-to-Run routing bridge.

A Wave name is local to its canonical repository. Its UUID remains stable when
the name or repository changes, so relocation is separate from Home placement:

```bash
lf work relocate wave <wave-id> --name platform
lf work relocate wave <wave-id> --repo ../moved-repository
```

Stop the Wave and its descendants first. Relocation preserves its Linear
Initiative projection, Work and Run history, journal, authored files, and Home
placement. A repository move carries the complete Wave chord, and renaming a
Wave carries descendants whose authored paths are nested below it. Configured
source and target PM Teams must match; use `lf pm reteam` for an intentional
provider ownership change. Divergent target files fail closed instead of being
merged, and retry finishes cleanup if the locator committed before a crash.

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

See [Architecture → Processes](architecture.md#processes).

## Projects and KRs

A project is a measured bet inside a wave. Its definition and KRs live in
Linear Project content — not in a repo file, a status table, or its own memory.

```bash
lf pm project create --wave infra --title "Technical Architecture" \
  --definition "Loopflow's architecture is legible from the top down." \
  --first task-design --loop slice --finally ship \
  --kr "Top-down architecture documentation is complete and published."
```

KRs should read as **proof under duration**: observable end states
demonstrated on real work over a stated window, not capability checkboxes
that pass once on a demo. [Authoring → Writing KRs](authoring.md#writing-krs)
carries the craft and examples.

## Linear

Tasks live in Linear; there are no local task lists. A wave maps to an
Initiative, each project to a Linear Project, each task to an Issue. Connect
once — `lf pm init` links or creates the Wave Initiative and establishes one
repository Team in `.lf/config.yaml`. Every Wave reuses that Team and issue-key
namespace. Don't paste ids by hand.

```bash
lf pm init --wave infra --team-key LOO     # first Wave establishes the repo Team
lf pm init --all                           # all nested Waves reuse it
lf pm sync --wave infra                    # refresh the local SQLite snapshot
lf pm show --wave infra --no-sync          # deterministic cache-only read
lf pm task create --wave infra --project stability --title "Daemon data integrity"
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
lf task start <linear-project-id> "add retry to token refresh"
pbpaste | lf task start incident-management
lf task run INF-123
lf task run INF-124 --stack-on INF-123     # dependent work before the parent merges
```

Task Work advances through zero or more serial PRs to `main`. Its Project
configures `first`, `loop`, and `finally` flows; Task launch resolves all three
and pins them for the lifetime of the Task. `--first`, `--loop`, and `--finally`
override one Task at launch. The first flow runs once, the loop repeats, and the
finally flow gates, records learnings, and lands. After a merge or abandonment,
Loopflow rotates the worktree onto the next branch. The Task inherits the wave's
`GOAL.md` and `MEMORY.md` plus its Project definition and KRs.

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

## Evidence Portfolios

There are two different portfolios in the hierarchy:

- The Wave's **bet portfolio** is its set of Projects. It allocates attention
  according to each Project's KR evidence and fit with the Wave objective.
- A Project's optional **approach portfolio** is a set of independent Tasks
  testing different mechanisms for one uncertain KR.

An approach portfolio is useful when a premature architecture choice would be
expensive and several safe probes can run independently. Each Task should name
its mechanism and return evidence, an artifact, an exact gap, or a
counterexample. Keep routes independent long enough to expose their own failure
modes; then let the Project Work synthesize and redirect them.

Do not represent competing approaches as duplicate Projects, launch multiple
Tasks with the same favored brief, or count activity as evidence. Block a route
whose missing dependency is as hard as the original question, and reopen it
only when a materially new mechanism appears. The Wave judges whether the
Project still earns attention; it does not micromanage the individual probes.

See [Bet Portfolios and Approach Portfolios](https://loopflow.studio/docs/wave-authoring#bet-portfolios-and-approach-portfolios)
for the authoring pattern and command examples.

## Crons

Crons schedule supplementary flows on a wave. They live in `GOAL.md`
frontmatter and are read by the resident loop: when a schedule comes due while
the loop is idle, it opens a system pass and dispatches the flow with
judgment. Edits land without a restart.

```markdown
<!-- wave/shipper/GOAL.md -->
---
crons:
  - flow: sync
    schedule: "0 0 0 1 * * *"
---
```

Schedules use 6/7-field cron syntax (seconds first). A schedule that comes due
mid-turn fires at the next turn boundary; occurrences older than 24 hours are
missed, not replayed.

## Drafting wave content

Draft with `lf design` or write the files by hand — see
[Authoring → Drafting](authoring.md#drafting). Once `wave/<name>/` exists,
the Mac app picks it up, and `lf start <name>` starts it from the CLI.
To remove a wave, stop it, then delete `wave/<name>/`.

## Worked example

A `wave/billing/` directory for a billing rewrite. `GOAL.md` sets the intent —
"replace the legacy billing system with a metered usage model" — and the
measures the loop re-judges each iteration: usage events recorded within 5
seconds, invoices correct for all plan types, legacy endpoints unchanged
during migration.

The backing Linear project holds the concrete Tasks:

```text
Usage events       → Event capture and storage
Metering API       → Public metering endpoint
Invoice generation → Monthly invoice calculation
Migration shim     → Legacy API compatibility layer
Cleanup            → Remove old billing code
```

The wave reads Projects and Tasks with `lf pm show --no-sync`, directs the
highest-priority Project, and starts Task Work for every independent
file-writing change. Each shipped PR folds into memory and closes its task.

## Next

[The Agent API →](agent-api.md) · [Conducting →](conducting.md) · [Get Started →](getting-started.md)

## Reference

[Configuration](config.md) · [Troubleshooting](troubleshooting.md)
