---
layout: default
title: Architecture
---

# Architecture

Loopflow turns one reusable instruction into one observed model-provider run.
Everything else composes that path, gives it durable purpose, delivers its
changes, places it on a machine, or projects what happened.

This guide is for developers changing Loopflow. It starts with the smallest
complete path, then opens into the six areas that own the system. Command
syntax lives in the [`lf` reference](lf.md); the exhaustive checked inventory
lives in [Architecture Reference](architecture-reference.md).

## Run one Skill

```bash
lf implement
```

That command discovers `implement`, assembles its context, chooses a provider,
publishes a Run manifest, launches the provider, records direct evidence, and
settles once. It needs no Wave, Project, Task, daemon, or readable planning
database.

```text
request
  |
  v
lf CLI --> Skill discovery --> prompt --> provider route --> harness
                                                          |
                                                          v
                                               Home-local Run record
                                               manifest + JSONL + terminal
```

Before the provider starts, the current Home contains:

```text
$LF_HOME/runs/<prefix>/<run-id>/
  manifest.json
```

While it runs, Loopflow may append lifecycle, conversation, tool, raw provider,
and usage evidence to `events.jsonl`. When the harness returns, it creates
`terminal.json` once. Planning SQLite may be unreadable; the launch still
works. Missing telemetry may make the record incomplete; it never changes the
provider result.

The implementation follows the same order as the diagram:

| Stage | Concrete owner | Produces |
| --- | --- | --- |
| Parse and dispatch | [`lf/mod.rs`](../rust/loopflow/src/lf/mod.rs) | One command and launch context |
| Find the Skill | [`lf/discovery.rs`](../rust/loopflow/src/lf/discovery.rs) | One selected Skill source |
| Assemble context | [`engine/prompt.rs`](../rust/loopflow/src/engine/prompt.rs) | System and task prompts |
| Select credentials and route | [`provider_account.rs`](../rust/loopflow/src/provider_account.rs) | Harness, account, model, credential |
| Launch and normalize | [`harness/`](../rust/loopflow/src/harness/) | Provider output and usage events |
| Publish and settle evidence | [`run_record.rs`](../rust/loopflow/src/run_record.rs) | One immutable manifest and terminal receipt |

[Follow the complete execution path →](architecture/execution.md)

## Add one capability at a time

The Skill runner is useful by itself. The rest of Loopflow grows outward by
adding one kind of capability at each layer. Tracked Work and autonomous
controllers are separate layers: Work remains fully operable when no
controller is installed or running, while controllers consume the same Work,
execution, and delivery operations available to any caller.

```text
one Skill run
    |
    +-- Flow: compose Skills, mechanical operations, and human boundaries
    |
    +-- Work: preserve purpose and input across independent processes
    |
    +-- Task delivery: bind concrete Work to a worktree and serial PRs
    |
    +-- Controllers: pursue Work end to end using the layers above
    |
    +-- Home: place execution, credentials, services, files, and locks
    |
    `-- Views: project planning, Run, provider, Git, and OS evidence
```

| Area | What it adds | Start here |
| --- | --- | --- |
| Execution | Skill discovery, prompt assembly, provider routing, harnesses, Run records | [Execution](architecture/execution.md) |
| Work | Wave/Project/Task identity, status, Steer, Ask, and planning facts | [Planning](architecture/planning.md) |
| Controllers | Flow playheads and end-to-end Wave/Project/Task automation | [Planning](architecture/planning.md#end-to-end-controllers) |
| Delivery | Managed worktrees, commits, serial PRs, CI repair, merge | [Delivery](architecture/delivery.md) |
| Homes | Placement, `lfd`, Wave listeners, SSH routing, machine install | [Homes and processes](architecture/homes.md) |
| Data | Truth owners, SQLite, files, external systems, projections, consistency | [Data and persistence](architecture/data.md) |
| Codebase | Source territories, public surfaces, processes, extension points | [Codebase map](architecture/codebase.md) |

The source tree makes the planning boundary literal:

```text
work/                          controller/
wave/{mod,config,context,      wave/{runner,resident,server,
      memory}                       placement,...}
project                       project/{mod,state}
task                          task/{mod,state}
                              runner + store

execution kernel: engine/ + harness/ + Run records
composition surfaces: lf/ + bin/
```

`work` never imports `controller`. The execution kernel works without either
layer and never loads Work. CLI and controller callers resolve Work identity,
Wave memory, and controller state, then pass ordinary launch inputs into the
kernel.

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
human / agent --> lf CLI --------+----------+-----------+
                    |
          +---------+----------+
          |                    |
          v                    v
  authored definitions    tracked Work
  Skills / Flows /       Wave -> Project -> Task
  goals / memory                  |
          |                       +---------> Task delivery
          |                       |                ^
          |                       v                |
          +------------> end-to-end controllers --+
          |                       |
          +-----------------------+ Skill boundary
                                  v
                       shared execution components
              discovery / prompt / route / harness
                              |
                              v
                    Home-local Run record
                              |
                              v
                  status / roadmap / usage / app

another machine is another Home; cross it explicitly with `lf ssh`
```

There is no central Loopflow server. A Home owns its processes, credentials,
planning store, Run records, and OS locks. Repository files carry authored
definitions and memory. Linear and GitHub keep shared planning and delivery
truth. Readers join those sources; they do not replace them with a universal
ledger.

## Core models

```text
Wave
  `-- Project
        `-- Task
              `-- serial PRs

Flow = ordered Skill | Op | Xor | human boundaries
Run  = evidence for one mediated harness launch

WorkRef = Wave | Project | Task
WorkStatus = Ready | Done | Abandoned
```

| Model | Represents | Primary truth |
| --- | --- | --- |
| Skill | Reusable instructions plus declared context needs | Repository override, builtin, or installed Markdown |
| Flow | Ordered Skill and mechanical nodes, Xor routing, human boundaries | Repository or builtin YAML plus a caller-owned playhead |
| Run | Evidence from one mediated provider launch | One immutable Home-local record |
| Wave | Durable operating context with goal, memory, cadence, chat, and project selection | Repository Wave files, local identity, Linear Initiative membership |
| Project | One measured bet inside exactly one Wave | Linear Project plus bounded local Work state |
| Task | One concrete change, investigation, or document | Linear Issue, local delivery state, Git, GitHub |
| Work | Shared durable planning state for one Wave, Project, or Task | Rows keyed directly by stable Work identity |
| Steer | Ordered authored correction to Work | Append-only Work input |
| Ask | Durable blocking request with a typed, first-writer-wins result | Ask exchange and active answering-attempt fence |
| Home | Stable machine authority whose route may change | Home identity and observed SSH route |
| Placement | Assignment of one Work to one Home | `(WorkRef, HomeId)` |

Run identity records causality and provenance. It never grants Work mutation,
credential, Git, or process-signal authority.

A provider-backed Flow boundary launches or continues a harness and therefore
produces Run evidence. Mechanical, routing, and human boundaries need not
create a Run.

## Follow the common paths

### Direct work

```bash
lf debug -c
lf gate --diff-files
```

These commands need the execution area only: discover, prompt, route, launch,
record, return.

### Tracked work without a controller

```bash
lf task prepare INF-123
lf --task INF-123 research "write scratch/runtime.md"
lf --task INF-123 research "write scratch/prompts.md"
lf commit -m "Reconcile Task research"
lf pr publish
lf pr submit
lf task status INF-123 --json
```

`prepare` creates or reuses tracked Task Work, its one worktree, and the active
serial PR identity. It installs no end-to-end controller. Each `--task` command
is an independent Run in that worktree; several may overlap and write distinct
scratch paths. Any caller may then use the ordinary Work and delivery commands.
Those commands act on delivery facts, not on proof that a controller ran its
expected Flow. `submit`, `arm`, and `land` therefore work the same whether the
Task was pursued piecemeal, by the built-in controller, or by another system.

### End-to-end controllers

```bash
lf start product
lf task run INF-123
lf chat --steer "ship invoices first"
lf status product
```

Controllers build on tracked Work rather than changing its meaning. `lf task
run` ensures the same Task/worktree substrate, installs Task controller state,
and starts its end-to-end flow. The Home keeper similarly starts the placed
Wave listener and its controller. Wave, Project, Task, and Ask retain distinct
loops; each reuses ordinary execution and delivery operations.

### Another machine

```bash
lf ssh build-home runs --json
lf ssh build-home start product
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
| Work state, Steer, Ask, or Work-bound Runs | [Planning](architecture/planning.md) |
| Flow semantics or Wave/Project/Task controllers | [Planning](architecture/planning.md#end-to-end-controllers) |
| worktrees, commits, PR ranges, checks, or landing | [Delivery](architecture/delivery.md) |
| daemons, remote execution, placement, process control, promotion | [Homes and processes](architecture/homes.md) |
| schema, files, projections, DTOs, or consistency | [Data and persistence](architecture/data.md) |
| module ownership, APIs, binaries, routes, or code size | [Codebase map](architecture/codebase.md) |

For exhaustive lookup, open the [checked architecture reference](architecture-reference.md).
Its maps are machine-verified against CLI families, process boundaries, live
SQLite tables, HTTP routes, providers, subprocess edges, projections,
compatibility seams, and retired vocabulary.
