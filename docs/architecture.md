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
adding one kind of capability at each layer.

```text
one Skill run
    |
    +-- Flow: compose Skills, mechanical operations, and human boundaries
    |
    +-- Work: preserve purpose and input across provider processes
    |
    +-- Task delivery: bind concrete Work to a worktree and serial PRs
    |
    +-- Home: place execution, credentials, services, files, and locks
    |
    `-- Views: project planning, Run, provider, Git, and OS evidence
```

| Area | What it adds | Start here |
| --- | --- | --- |
| Execution | Skill discovery, prompt assembly, provider routing, harnesses, Run records | [Execution](architecture/execution.md) |
| Planning | Flow composition, Wave/Project/Task Work, Steer, Ask, resident loops | [Planning](architecture/planning.md) |
| Delivery | Managed worktrees, commits, serial PRs, CI repair, merge | [Delivery](architecture/delivery.md) |
| Homes | Placement, `lfd`, Wave listeners, SSH routing, machine install | [Homes and processes](architecture/homes.md) |
| Data | Truth owners, SQLite, files, external systems, projections, consistency | [Data and persistence](architecture/data.md) |
| Codebase | Source territories, public surfaces, processes, extension points | [Codebase map](architecture/codebase.md) |

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
  authored definitions   durable planning
  Skills / Flows /       Wave -> Project -> Task
  goals / memory                  |
          |                       v
          +-----------> planning controllers <------ Task delivery
                              |
                              | Skill boundary
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
| Flow | Ordered Skill and mechanical nodes, Xor routing, human boundaries | Repository or builtin YAML plus the Work playhead |
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

### Durable delegated work

```bash
lf task run INF-123
lf task steer INF-123 "keep the parser public"
lf pr publish
lf task status INF-123 --json
```

This crosses planning, execution, and delivery. Stable Task Work survives any
one provider process; each provider launch leaves a separate Run record.

### Resident planning

```bash
lf start product
lf chat --steer "ship invoices first"
lf status product
```

The Home keeper starts the placed Wave listener. Its resident loop refreshes
current planning evidence and chooses the next Project or Task boundary. Wave,
Project, Task, and Ask retain distinct controllers; when one reaches a Skill
boundary it reuses the ordinary discovery, prompt, provider, harness, and
Run-evidence components before recording its domain transition.

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
| Flow semantics, Work state, Steer, Ask, Project/Task loops | [Planning](architecture/planning.md) |
| worktrees, commits, PR ranges, checks, or landing | [Delivery](architecture/delivery.md) |
| daemons, remote execution, placement, process control, promotion | [Homes and processes](architecture/homes.md) |
| schema, files, projections, DTOs, or consistency | [Data and persistence](architecture/data.md) |
| module ownership, APIs, binaries, routes, or code size | [Codebase map](architecture/codebase.md) |

For exhaustive lookup, open the [checked architecture reference](architecture-reference.md).
Its maps are machine-verified against CLI families, process boundaries, live
SQLite tables, HTTP routes, providers, subprocess edges, projections,
compatibility seams, and retired vocabulary.
