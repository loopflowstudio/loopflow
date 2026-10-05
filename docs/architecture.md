---
layout: default
title: Architecture
---

# Architecture

Loopflow records commands, preserves agent conversations, and drives Flows.
Exec owns a process and AgentSession a conversation; a Flow is one driver Exec
and the step Execs it starts.

This guide specifies the accepted model. The reference owns
[cutover status](architecture-reference.md#cutover-status), the remaining work
map and the checked current-source inventory.

This guide is for developers changing Loopflow. It starts with the smallest
complete path, then opens into the six areas that own the system. Command
syntax lives in the [`lf` reference](lf.md); the exhaustive checked inventory
lives in [Architecture Reference](architecture-reference.md).

## Run one Skill

```bash
lf implement
```

That command discovers `implement`, assembles its context, chooses a provider,
records its Exec, reserves an AgentSession, captures the input, then launches the
provider. It needs no Wave, Project, Task, or daemon. The conversation persists
after that command exits; a later command can connect to the same AgentSession.

```text
request
  |
  v
lf CLI --> Skill discovery --> prompt --> provider route --> harness
                                                          |
                                                          v
                                               AgentSession history
                                               + immutable input/output artifacts
```

Before the provider starts, the current Home has the conversation reservation
and its immutable input. SQLite owns identity, attribution, driver authority and
history references; payload files retain large captured inputs and output.
Admission failure stops before provider launch. General command logging has a
different boundary: unavailable/bootstrap stores leave explicitly unrecorded
Execs rather than bypassing installation authority to create a receipt.

AgentSession history records provider outcomes, retries and usage. Exec records
the command's outcome. A provider can finish successfully before a later command
operation fails; neither result overwrites the other. Missing telemetry stays
missing. The [execution contract](architecture-reference.md#core-models-and-apis)
defines these ownership boundaries.

The implementation follows the same order as the diagram:

| Stage | Concrete owner | Produces |
| --- | --- | --- |
| Parse and dispatch | [`lf/mod.rs`](../rust/loopflow/src/lf/mod.rs) | One command and launch context |
| Find the Skill | [`lf/discovery.rs`](../rust/loopflow/src/lf/discovery.rs) | One selected Skill source |
| Assemble context | [`engine/prompt.rs`](../rust/loopflow/src/engine/prompt.rs) | System and task prompts |
| Select credentials and route | [`provider_account.rs`](../rust/loopflow/src/provider_account.rs) | Harness, account, model, credential |
| Launch and normalize | [`harness/`](../rust/loopflow/src/harness/) | Provider output and usage events |
| Record command and conversation evidence | [`journal/`](../rust/loopflow/src/journal/) and the store | Exec completion, AgentSession history and immutable payloads |

[Follow the complete execution path →](architecture/execution.md)

## Add one capability at a time

The Skill runner is useful by itself. The rest of Loopflow grows outward by
adding one kind of capability at each layer. Tracked Work owns autonomous
progression; attributed conversations consume the same execution and delivery
operations available to helpers without becoming standing Work identities.

```text
one Skill run
    |
    +-- Flow: compose Skills, mechanical operations, and review boundaries
    |
    +-- Work: preserve purpose and input across independent processes
    |
    +-- Task delivery: bind concrete Work to one active remote branch, worktree, and PR
    |
    +-- Controllers: pursue Work end to end using the layers above
    |
    +-- Home: place execution, credentials, services, files, and locks
    |
    `-- Views: project planning, command, conversation, provider, Git, and OS evidence
```

| Area | What it adds | Start here |
| --- | --- | --- |
| Execution | Skill discovery, prompt assembly, provider routing, harnesses, command and conversation history | [Execution](architecture/execution.md) |
| Planning | Flow composition, Wave/Task Work, Steer, questions, review FlowSteps | [Planning](architecture/planning.md) |
| Delivery | Managed worktrees, commits, one active Task branch/PR, CI repair, merge | [Delivery](architecture/delivery.md) |
| Homes | Placement, SSH routing, machine install | [Homes and processes](architecture/homes.md) |
| Data | Truth owners, SQLite, files, external systems, projections, consistency | [Data and persistence](architecture/data.md) |
| Codebase | Source territories, public surfaces, processes, extension points | [Codebase map](architecture/codebase.md) |

The source tree makes the planning boundary literal:

```text
work/
wave/{mod,config,context,
      memory,metrics,relocate}
project
task

ops/chapter.rs                deterministic chapter rotation
ops/task.rs                   Task launch: a fresh Flow in the Task checkout
lf/commands/flow.rs           Flow driver: one process per Flow

execution kernel: engine/ + harness/ + command and conversation history
composition surfaces: lf/ + bin/
```

`work` never imports the Flow driver. The execution kernel works without either
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

## The whole system

```text
                                      shared truth
                              Linear     GitHub     providers
                                 ^          ^           ^
                                 |          |           |
user / agent --> lf CLI --------+----------+-----------+
                    |
          +---------+----------+
          |                    |
          v                    v
  authored definitions    tracked Work
  Skills / Flows /       Wave -> Linear Project -> Task
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
                        Exec / AgentSession
                              |
                              v
                  status / roadmap / usage / app

another machine is another Home; cross it explicitly with `lf ssh`
```

There is no central Loopflow server. A Home owns its processes, credentials,
planning store, execution history, and OS locks. Repository files carry authored
definitions and memory. Linear and GitHub keep shared planning and delivery
truth. Readers join those sources; they do not replace them with a universal
ledger.

## Core models

```text
Repository
  `-- Wave                    enduring objective, memory and cadence
        `-- Linear Project    status + shared chapter name + Flow + KRs
              `-- Task        identity, worktree, PR and every Flow run for it

Exec                          one actual lf process; immutable causal parent
AgentSession                  one conversation; nullable current driver Exec
  `-- history                 provider starts, outcomes, retries and usage
Flow                          one driver Exec; cursor in its memory, graph in FlowExec
  `-- step Execs              plain commands; the driver records each one's node
```

| Model | Represents | Primary truth |
| --- | --- | --- |
| Skill | Reusable instructions and declared context | Repository, builtin or installed Markdown |
| Flow | Reusable graph of agent, mechanical and routing steps | Repository or builtin YAML |
| Exec | One actual lf process, its caller and command completion | `execs` |
| AgentSession | An interactive or headless conversation across drivers and native reconnection | `agent_sessions`, subordinate history and provider-native conversation |
| Running Flow | One driver process and the steps it starts, including taskless execution | The driver Exec and its child step Execs in `execs` |
| Wave | Enduring objective, memory, cadence, budget and metric instruments | Wave files, local Wave identity and Linear Initiative membership |
| Chapter | Shared name of each Wave's In Progress Project | Linear Project statuses; no Chapter row or packet |
| Project | A Wave's plan, KRs, targets and default Flow | Linear Project and its synchronized `projects` row |
| Task | A concrete change, investigation or document | `tasks`, Linear Issue, Git and GitHub |
| Steer | An authored correction to Work | Ordered Work input |
| Home | A machine's store, credentials and exact process authority | Home identity and observed route |
| Placement | Where a Work executes | `(WorkRef, HomeId)` |

An AgentSession keeps its ID, name, feedback and native conversation when its
command process changes. Its `interactive` field is independent of purpose,
Flow membership and completion. Default conversation views show interactive
Sessions; explicit filters expose headless and completed history. Large captured
payloads remain files, but readers select and page rows before opening them.

Connect uses the live engine when possible. Passive display acquires no claim.
Transferring the conversation driver revokes the old client's ability to start
or steer turns and mutate Session state, including queued writes. It does not
replace the provider generation or interrupt an existing turn. Client replacement
leaves the engine alive and never authorizes killing a shared engine
for one thread. Engine PID, client PID and conversation driver are distinct.

Exec ancestry records the actual lf caller. A direct child names its parent's
Exec; an agent-issued child also records `via_agent` and AgentSession provenance.
The provider's generation resolves to the current driver at child admission.
A delayed command from a replaced provider retains historical provenance; old
Exec parents are never rewritten. Causal ancestry grants no control authority.

Every Flow naming a Task, or run in its checkout, is equally its work;
none is privileged. `task run` always runs a fresh one.
Taskless and Task-owned Flows use the same driver. It holds the compiled graph
and cursor in memory; template composition compiles into the graph, and loop
passes are positions in it. Each step is a real child Exec whose argv carries
the Flow name, launch sequence and position. A step's result is how its process
exited; a deciding or routing step also answers through the Session turn its
Exec captured. Mechanical work creates no agent conversation. A Session reaches
its Flow through the step Exec that captured its input. A killed driver leaves
its Execs as history; nothing resumes it, and its caller launches fresh work.

Task implies Wave. Constructors fill omitted ancestors and reject mismatches.
Bind fills an unassigned conversation's Task once. CLI states the permanent
target and writes; Desktop confirms it. Bind cannot clear or move an assignment
or change Flow membership. Done and landed Tasks remain valid targets. Earlier
usage retains its recorded owner; binding affects subsequent work, and uncertain
mid-turn allocation remains unknown.

`tasks.started_at` is set once when actual Task work is reserved or first bound.
Recording an inspection command's Exec does not start a Task. Chapter retirement
also checks authored work, PRs and Flows; missing history alone cannot
prove untouched backlog. Rotation converges from fresh Linear facts using the
explicit target name and stable Project identities. A partially rotated repository
must be retryable; unrelated competing plans remain unresolved.

The [reference](architecture-reference.md#core-models-and-apis) owns the field and
write contracts and the current-state conversion boundary. Exec and AgentSession
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
lf submit
lf task status INF-123 --json
```

`prepare` creates or reuses tracked Task Work, its one worktree, and the active
serial PR identity. It starts no Task execution. Each `--task` command
starts an independent AgentSession in that worktree; several may overlap and write distinct
scratch paths. Any caller may then use the ordinary Work and delivery commands.
Those commands act on delivery facts, not on Flow-driving authority. `submit` and `land` therefore work the same whether the Task was pursued by a
Task Flow, piecemeal helper AgentSessions, or another system.

### Task Flows

```bash
lf --wave product wave/operate        # one finite planning pass
lf task run INF-123
lf --wave <wave> wave/operate "ship invoices first"
lf wave status product
```

`task run` places the Task, then runs a fresh Flow in its checkout exactly as
`lf --task ISSUE run FLOW` does, returning when the Flow ends; it never
continues an earlier one. Repository rotation converges every Wave on the
requested Project name, preserving active Task identity and execution. Direct
questions and helpers use attributed AgentSessions without gaining Flow
authority. Flows hold autonomous steps only; review happens in the Task
conversation.

### Another machine

```bash
lf ssh build-home session list --json
lf ssh build-home --wave product wave/operate
```

The origin transports one command. The target resolves its own Home state and
runs the same `lf`. Reads are local unless this hop is explicit.

## How to read the code

Start with the area that owns the behavior, then follow the object passed to
the next area. Do not begin with the storage implementation unless storage is
the behavior.

| If you are changing… | Read |
| --- | --- |
| provider launch, retries, usage, or telemetry | [Execution](architecture/execution.md) |
| Flow semantics, Work state, Steer, questions, review FlowSteps, chapter rotation and Task advancement | [Planning](architecture/planning.md) |
| worktrees, commits, PR ranges, checks, or landing | [Delivery](architecture/delivery.md) |
| remote execution, placement, process control, promotion | [Homes and processes](architecture/homes.md) |
| schema, files, projections, DTOs, or consistency | [Data and persistence](architecture/data.md) |
| module ownership, APIs, binaries, routes, or code size | [Codebase map](architecture/codebase.md) |

For exhaustive lookup, open the [checked architecture reference](architecture-reference.md).
Its bounded checker compares CLI families, process boundaries, SQLite tables,
HTTP routes, providers, subprocess edges, projections, and historical vocabulary.
A successful check covers that inventory, not configured runtime behavior.
