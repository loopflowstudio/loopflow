---
layout: default
title: Architecture
---

# Architecture

Loopflow records commands, preserves agent conversations, and drives Graphs.
Process records an lf command and its outcome; LfSession records a conversation; a Graph is one lf process
and the step processes it starts. An agent runs in one of three places: in the
command that launched it, in a terminal the Machine server's holder keeps, or
under a detached Graph process. [What runs on a Machine](#what-runs-on-a-machine)
is the whole process model.

This guide specifies the accepted model. The reference owns
[cutover status](reference-cutover.md#cutover-status), the remaining work
map and the checked current-source inventory.

This guide is for developers changing Loopflow. It starts with the smallest
complete path, then opens into the areas that own the system. Command
syntax lives in the [`lf` reference](../lf-reference.md); the exhaustive checked inventory
lives in [Architecture Reference](reference.md).

## Run one Skill

```bash
lf implement
```

That command discovers `implement`, assembles its context, chooses a provider,
records its Process, reserves an LfSession, captures the input, then launches the
provider. It needs no Wave, Task, or server process. The conversation persists
after that command exits; a later command can connect to the same LfSession.

```text
request
  |
  v
lf CLI --> Skill discovery --> prompt --> provider route --> harness
                                                          |
                                                          v
                                               LfSession history
                                               + immutable input/output artifacts
```

Before the provider starts, the current Machine has the conversation reservation
and its immutable input. SQLite owns identity, attribution, attachment authority and
history references; payload files retain large captured inputs and output.
Admission failure stops before provider launch. General command logging has a
different boundary: unavailable/bootstrap stores leave explicitly unrecorded
Processes rather than bypassing installation authority to create a receipt.

LfSession history records provider outcomes, retries and usage. Process records
the command's outcome. A provider can finish successfully before a later command
operation fails; neither result overwrites the other. Missing telemetry stays
missing. The [execution contract](reference-models.md#core-models-and-apis)
defines these ownership boundaries.

The implementation follows the same order as the diagram:

| Stage | Concrete owner | Produces |
| --- | --- | --- |
| Parse and dispatch | [`lf/mod.rs`](../../rust/loopflow/src/lf/mod.rs) | One command and launch context |
| Find the Skill | [`skills/catalog.rs`](../../rust/loopflow/src/skills/catalog.rs) | One selected Skill source |
| Assemble context | [`prompt/mod.rs`](../../rust/loopflow/src/prompt/mod.rs) | System and task prompts |
| Select credentials and route | [`provider_account.rs`](../../rust/loopflow/src/provider_account.rs) | Harness, account, model, credential |
| Launch and normalize | [`harness/`](../../rust/loopflow/src/harness) | Provider output and usage events |
| Record command and conversation evidence | [`journal/`](../../rust/loopflow/src/journal) and the store | Process completion, LfSession history and immutable payloads |

[Follow the complete execution path →](execution.md)

## Add one capability at a time

The Skill runner is useful by itself. The rest of Loopflow grows outward by
adding one kind of capability at each layer. Tracked Work owns autonomous
progression; attributed conversations consume the same execution and delivery
operations available to helpers without becoming standing Work identities.

```text
one Skill run
    |
    +-- Graph: compose Skills, mechanical operations, and review boundaries
    |
    +-- Work: preserve purpose and input across independent processes
    |
    +-- Task delivery: bind concrete Work to one active remote branch, worktree, and PR
    |
    +-- Controllers: pursue Work end to end using the layers above
    |
    +-- Machine: place execution, credentials, services, files, and locks
    |
    `-- Views: project planning, command, conversation, provider, Git, and OS evidence
```

| Area | What it adds | Start here |
| --- | --- | --- |
| Execution | Skill discovery, prompt assembly, provider routing, harnesses, command and conversation history | [Execution](execution.md) |
| Planning | Graph composition, Wave/Task Work, Steer, questions, review GraphSteps | [Planning](planning.md) |
| Delivery | Managed worktrees, commits, one active Task branch/PR, CI repair, merge | [Delivery](delivery.md) |
| Machines | Execution destinations, Task ownership and SSH routing | [Machines and processes](machines.md) |
| Terminals | Where an agent runs, held terminals, views, headless work | [Transport](../../rust/loopflow/src/transport/README.md) |
| Client access and installation | Server admission, registration/update and continuity across replacement | [Client API server](server.md) |
| Data | Truth owners, SQLite, files, external systems, projections, consistency | [Data and persistence](data.md) |
| Codebase | Source territories, public surfaces, processes, extension points | [Codebase map](codebase.md) |

The source tree makes the planning boundary literal:

```text
work/
wave/{mod,config,context,
      memory,metrics,relocate}
project
task

ops/task.rs                   Task launch: a fresh Graph in the Task checkout
lf/commands/graph.rs           Graph process: one process per Graph

execution kernel: graph/ + agent/ + prompt/ + harness/ + command and conversation history
composition surfaces: lf/ + bin/
```

`work` never imports the Graph runner. The execution kernel works without either
layer and never loads Work. CLI callers resolve Work identity and Wave memory,
then pass ordinary launch inputs into the kernel.

Release delivery also separates proof from authority:

```text
merged release commit
        |
        v
candidate ref --> hosted matrix --> signed artifact receipt
                                           |
                                           v
                                  immutable version tag
                                           |
                                           v
                                      publication
```

The candidate ref and receipt are disposable recovery state. The version tag
is created only after the exact commit and artifact hashes are proven; retries
after that point publish the same bytes under the same tag.

Each area page starts with a real command or artifact, follows its request or
data flow, and ends with the contracts that neighboring areas may rely on.

## What runs on a Machine

```bash
lf implement                 # the agent is this command's child
lf resume --task LOO-12      # a view of a terminal the holder keeps
lf task run LOO-12           # starts a Graph process and returns
ps -axo command | grep '^lf '
# lf server run
# lf holder run
# lf --task task_… graph run
```

Three processes stay up: one Machine server, a holder for each build that
still has agents, and a Graph process for each running Graph. Every other `lf`
is a command that ends.

```text
your terminal            lf <skill> ──────────────────────────► agent        here

your terminal ─ view ─┐
Desktop pane  ─ view ─┼─► Machine server ─► holder ─► PTY ─► agent or shell   held
phone         ─ view ─┘         │
                                └─ store, hooks, steers, Work reads and writes

lf task run / lf ship / landing ─► Graph process ─► step lf ─► agent          detached
        │                                │
        └─ one startup line, then gone   └─ logs/graph-<id>.log
```

A launch runs in exactly one of three places, chosen once in
`transport::delegate`:

| | Here | Held | Detached |
| --- | --- | --- | --- |
| What | `lf <skill>`, `lf graph …`, any bare `-b` or piped run, every headless Graph step | Task, Wave and repository conversations; Desktop shells | Graph launches, landing watchers, `lf ship`, a `-b` or piped Work command |
| Parent of the agent | The launching `lf`, in its caller's terminal or pipe | The holder, on a PTY | A step `lf` under the Graph process |
| Ends with | Its caller | Its agent quitting, or its Task closing | Its Graph |
| Seen through | The terminal it runs in | Any number of views, all live | Its log, `lf task status`, the Session's history |
| Needs the server | No | Yes; it fails rather than fall back | To start, no |

Work decides between Here and the other two: a command that names a Task or
Wave, or reopens a Session bound to one, is Work, and a plain `lf` typed in a
Task's checkout is not. A terminal decides between Held and Detached: Work
someone views is held, Work nobody views is detached. Nothing wraps a headless
run in a terminal, and nothing hosts a conversation in a process of its own.

Hooks follow Work, not the place. A bare agent gets two, which print the
context block at start and after compaction from files in the checkout. An
agent launched for a Task or Wave (a held conversation, or a Graph step, which
`lf task run` starts with `--task`) also reports turn and tool boundaries to
the server and takes steers from it, so its launch starts the server first.

| Process | One per | Holds | Restarts |
| --- | --- | --- | --- |
| Machine server (`lf server run`) | Machine | Client connections, hook boundaries, the registry of held terminals with one terminal emulator each, views, exit settlement | On every install; the OS supervisor keeps it up |
| Holder (`lf holder run`) | Build with live terminals | PTYs, their children, a ring of recent output per PTY. No store, no Sessions | Never; it exits when its last terminal ends |
| Graph process (`lf … graph run`) | Running Graph | The graph, its cursor and its step children | Never; a stopped Graph is history and `lf task run` starts a fresh one |

What survives what follows from that table. Closing a view ends nothing.
Restarting or upgrading the server ends nothing: it greets the holders it
finds, rebuilds each screen from the holder's ring and carries on, and views
reconnect at the PTY byte they had reached. An install replaces the server and
leaves holders on the build that started them, so a conversation keeps its
build until it is quit and resumed. Killing the `lf task run` that started a
Graph leaves the Graph running. Logout and reboot end every process; Sessions
and Graph history remain, and `lf resume` or `lf task run` starts new ones.

The server is in no agent's byte path except as the relay between a held PTY
and its views. A hook that cannot reach it logs a gap and the turn continues;
a holder that loses it keeps its agents and says nothing about them. The
store is opened by whichever process is writing: commands, Graph processes
and the server. The holder never opens it.

Conversations started by a build from before held terminals run under an
`lf session serve` process of their own until their agent quits. Nothing
starts one now.

The [transport README](../../rust/loopflow/src/transport/README.md) owns the
holder and view protocols; the
[server README](../../rust/loopflow/src/server/README.md) owns client
operations and hook boundaries.

## The whole system

```text
                                      shared truth
                              planning   GitHub     providers
                                 ^          ^           ^
                                 |          |           |
user / agent --> lf CLI --------+----------+-----------+
                    |
          +---------+----------+
          |                    |
          v                    v
  authored definitions    tracked Work
  Skills / Graphs /       Wave -> Task
  goals / memory                  |
          |                       +---------> Task delivery
          |                       |                ^
          |                       v                |
          +------------> Task advancement --------+
          |                       |
          +-----------------------+ Skill boundary
                                  v
                       shared execution components
              discovery / prompt / route / harness
                              |
                              v
                        Process / LfSession
                              |
                              v
                  status / planning / usage / app

another machine is another Machine; cross it explicitly with `lf --machine`
```

Each Machine's server also admits clients: Desktop over a private socket, and
a paired phone over an authenticated connection ([client API server](server.md)).
It does not centralize execution across Machines. Machine routing selects
where commands run, the server admits clients, and installation owns server
registration and updates. A Machine owns its processes, credentials,
planning store, execution history, and OS locks. Repository files carry authored
definitions and memory. The planning provider (a Git ref or Linear) and GitHub
keep shared planning and delivery truth. Readers join those sources; they do not replace them with a universal
ledger.

## Core models

```text
Repository
  |-- Task                    repository work, optionally outside a Wave
  `-- Wave                    enduring objective, memory, cadence and default graph
        `-- Project           plan, Tasks, KRs and targets; optional provider binding
              `-- Task        identity, worktree, PR and captured graph position

Process                          one actual lf process; immutable causal parent
LfSession                  one conversation; nullable attached LfProcess
  `-- history                 provider starts, outcomes, retries and usage
Graph                          one lf process; cursor in its memory, graph in GraphProcess
  `-- step processes              plain commands; the Graph process records each one's node
```

| Model | Represents | Primary truth |
| --- | --- | --- |
| Skill | Reusable instructions and declared context | Repository, builtin or installed Markdown |
| Graph | Reusable graph of agent, mechanical and routing steps | Repository or builtin YAML |
| Process | One actual lf process, its caller and command completion | `processes` |
| LfSession | An interactive or headless conversation across attachments and native reconnection | `agent_sessions`, subordinate history and provider-native conversation |
| Running Graph | One Graph process and the steps it starts, including taskless execution | The Graph process and its child step processes in `processes` |
| Wave | Enduring objective, memory, cadence, budget and metric instruments | Wave files, local Wave identity and, with Linear, Initiative membership |
| Project | A Wave's plan, Tasks, KRs and targets | `projects` row, shared through the planning provider |
| Task | A concrete change, investigation or document | `tasks`, the planning provider, Git and GitHub |
| Steer | An authored correction to Work | Ordered Work input |
| Machine | A machine's store, credentials and exact process authority | Machine identity and observed route |
| Task ownership | Who works on a Task, on which Machine, or unowned | Append-only `task_ownership` log; each person's Machine-local Wave → Machine default places their unowned Work |

An LfSession keeps its ID, name, feedback and native conversation when its
command process changes. Its `interactive` field is independent of purpose,
Graph membership and completion. Default conversation views show interactive
Sessions; explicit filters expose headless and completed history. Large captured
payloads remain files, but readers select and page rows before opening them.

Connect uses the live AgentProcess when possible. Passive display acquires no claim.
Transferring the attachment revokes the old client's ability to start
or steer turns and mutate Session state, including queued writes. It does not
replace the AgentProcess or interrupt an existing turn. Client replacement
leaves the AgentProcess alive and never authorizes killing a shared one
for one thread. AgentProcess PID, client PID and attached LfProcess are distinct.

Process ancestry records the actual lf caller. A direct child names its parent's
Process; an agent-issued child also records `via_agent` and LfSession provenance.
The provider's AgentProcess resolves to its attached LfProcess at child admission.
A delayed command from a replaced provider retains historical provenance; old
Process parents are never rewritten. Causal ancestry grants no control authority.

Every Graph naming a Task, or run in its checkout, is part of its work.
Only `lf task run` and `lf ship` advance the Task's captured graph position;
independent Graphs leave that position unchanged. Each launch starts a fresh
Graph process.
Taskless and Task-owned Graphs use the same Graph runner. The Graph process holds the compiled graph
and cursor in memory; template composition compiles into the graph, and loop
passes are positions in it. Each step is a real child Process whose argv carries
the Graph name, launch sequence and position. A step's result is how its process
exited; a deciding or routing step also answers through the Session turn its
Process captured. Mechanical work creates no agent conversation. A Session reaches
its Graph through the step Process that captured its input. A killed Graph process leaves
its Processes as history; nothing resumes it, and its caller launches fresh work.

Task requires repository identity and may belong to a Wave through its planning
row. Constructors fill omitted ancestors and reject mismatches.
Bind fills an unassigned conversation's Task once. CLI states the permanent
target and writes; Desktop confirms it. Bind cannot clear or move an assignment
or change Graph membership. Done and landed Tasks remain valid targets. Earlier
usage retains its recorded owner; binding affects subsequent work, and uncertain
mid-turn allocation remains unknown.

Reserving actual Task work (`lf task run` or `lf task checkout` allocating a
checkout) claims an unowned Task in its append-only ownership log. Recording an
inspection command's Process does not claim a Task. Missing history alone cannot
prove untouched backlog; unrelated competing plans remain unresolved.

The [reference](reference-models.md#core-models-and-apis) owns the field and
write contracts and the current-state conversion boundary. Process and LfSession
are the execution owners; Run has no separate lifecycle.

## Follow the common paths

### Direct work

```bash
lf debug -c
lf gate --diff files
```

These commands need the execution area only: discover, prompt, route, launch,
record, return.

### Direct Task work

```bash
lf checkout INF-123
lf --task INF-123 research "write scratch/runtime.md"
lf --task INF-123 research "write scratch/prompts.md"
lf commit -m "Reconcile Task research"
lf pr publish
lf pr submit
lf task status INF-123 --json
```

`checkout` creates or reuses tracked Task Work and its checkout placement
without starting execution. Publication creates its optional, sole PR. Each `--task` command
starts an independent LfSession in that worktree; several may overlap and write distinct
scratch paths. Any caller may then use the ordinary Work and delivery commands.
Those commands act on delivery facts, not on Graph-driving authority. `submit` and `land` therefore work the same whether the Task was pursued by a
Task Graph, piecemeal helper LfSessions, or another system.

### Task Graphs

```bash
lf --wave product wave-operate        # one finite pass over your started Tasks
lf task run INF-123
lf --wave <wave> wave-operate "ship invoices first"
lf wave status product
```

`task run` places the Task and starts a fresh Graph process from its saved
position, returning once the process registers. It stops at the next interactive
step for review in the Task conversation. `-b` waits and runs those steps headless;
`lf ship` approves continuation through delivery. A stopped process stays history.
A direct `lf --task ISSUE graph GRAPH` uses the same runner without moving the
Task's graph position. Replanning keeps existing Task identity and execution.
See [Task workflows](../orchestration.md#where-a-person-takes-part).

### Another machine

```bash
lf --machine build-home session list --json
lf --machine build-home --wave product wave-operate
```

The origin transports one command. The target resolves its own Machine state and
runs the same `lf`. Reads are local unless this hop is explicit.

## How to read the code

Start with the area that owns the behavior, then follow the object passed to
the next area. Do not begin with the storage implementation unless storage is
the behavior.

| If you are changing… | Read |
| --- | --- |
| provider launch, retries, usage, or telemetry | [Execution](execution.md) |
| Graph semantics, Work state, Steer, questions, review GraphSteps and Task advancement | [Planning](planning.md) |
| worktrees, commits, PR ranges, checks, or landing | [Delivery](delivery.md) |
| remote execution, Task ownership, process control, promotion | [Machines and processes](machines.md) |
| where an agent runs, held terminals, views, headless launches | [Transport](../../rust/loopflow/src/transport/README.md) |
| client operations, hook boundaries, subscriptions | [Server](../../rust/loopflow/src/server/README.md) |
| schema, files, projections, DTOs, or consistency | [Data and persistence](data.md) |
| module ownership, APIs, binaries, routes, or code size | [Codebase map](codebase.md) |

For exhaustive lookup, open the [checked architecture reference](reference.md).
Its bounded checker compares CLI families, process boundaries, SQLite tables,
HTTP routes, providers, subprocess edges, projections, and historical vocabulary.
A successful check covers that inventory, not configured runtime behavior.

## Research

[Session experience comparison](https://github.com/loopflowstudio/loopflow/blob/7232e533e14be764951b8eb35bfef8262086e5a5/docs/dev/research/session-experience.html) reviews T3 Code,
Paradev and Paseo commit history and tests, with Loopflow findings and Task handoffs.
It records source observations from November 2026, not a live product benchmark.
