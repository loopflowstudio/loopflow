---
layout: default
title: Architecture
---

# Architecture

Loopflow turns repo-authored intent into persistent agent work. Humans talk to
a Wave. The Wave directs Projects, Projects supervise Tasks, and each Task owns
the worktree and delivery lifecycle for one concrete change.

## System Map

```text
Authoring layer
  README.md, docs/
  .lf/skills/*.md
  .lf/flows/*.yaml
  .lf/directions/*.md
  wave/<name>/GOAL.md
  wave/<name>/MEMORY.md
  wave/<name>/*.md
        |
        v
Execution engine
  rust/loopflow/src/engine/
  prompt assembly, flow execution, agent launch, git/worktree helpers
        |
        +------------------------+
        |                        |
        v                        v
Local CLI                 Daemon
  lf                        lfd
  rust/.../lf               rust/.../lfd
  rust/.../ops              HTTP read surface, registry,
                            webhook Task reconciliation,
                            token refresh
        |                        |
        v                        v
Git/PR/PM ops             Clients
                            lf CLI
                            Loopflow Swift app
                            webhooks
```

## Core Concepts

| Concept | Stored in | Runtime owner |
|---|---|---|
| Skill | `.lf/skills/` and built-ins | `engine` |
| Flow | `.lf/flows/` and built-ins | `engine` |
| Direction | `.lf/directions/` | `engine` prompt assembly |
| Wave | repo files + lfdb | `lf serve` listener and resident on clean `main` |
| Project | Linear Project + lfdb runtime | Project loop on clean `main` |
| Task | Linear issue + lfdb runtime | Task loop in one sibling worktree |
| Directive | lfdb | Project/Task controller and acknowledgement |
| Run/event | lfdb | historical execution and durable observations |
| Attention | lfdb | lfd + Loopflow |

## CLI and Engine

`lf` is the local command runner. It resolves a skill or flow, assembles context,
launches the configured coding agent, and runs ops such as commit, rebase, PR,
PM sync, and release.

Important paths:

- `rust/loopflow/src/bin/lf.rs`
- `rust/loopflow/src/lf/`
- `rust/loopflow/src/engine/`
- `rust/loopflow/src/ops/`

The engine owns the common language: prompts, flows, forks, worktrees, built-in
skills, skills, structured replies, and launch behavior. Ops wrap concrete
side-effectful workflows around git, PRs, PM providers, and release artifacts.

## Daemon

`lfd` is the gatekeeper: it serves read routes and event push, verifies GitHub
webhooks and reconciles merged Task Sessions, refreshes provider tokens, and
tidies the registry at boot. It dispatches no work; Wave residents and durable
Project/Task processes own agent execution.

Important paths:

- `rust/loopflow/src/bin/lfd.rs`
- `rust/loopflow/src/lfd/http/`
- `rust/loopflow/src/lfdb/`
- `rust/loopflow/src/lfd/triggers/` (token refresh — the one surviving loop)
- `rust/loopflow/src/lfd/types/`

`lfd` uses sqlite and a local capability token. The old container service path
is gone; self-hosted operations are SSH-first.

## Clients

`lf` and Loopflow read lfd state; webhooks push events in. The Python package
is a library of wire models only.

Important paths:

- `python/loopflow/models.py`

Loopflow is the Swift app. It reads the native Wave → Project → Task snapshot,
renders the work map beside the one Wave conversation, and provides native
surfaces for attention, terminal workspaces, provider auth, and live output.

Important paths:

- `swift/Loopflow/Models/`
- `swift/Loopflow/State/`
- `swift/Loopflow/Services/`
- `swift/Loopflow/Views/`

## Wave Loop

```text
1. Read GOAL.md, MEMORY.md, the PM snapshot's Projects and tasks, and relevant docs.
2. Clarify the current objective and portfolio.
3. Pursue one move: direct a Project or Task, unblock, review, or wait.
4. Judge the evidence and return scheduling to the controller.
5. Record events, update PM state, and surface attention.
6. Repeat when the controller, a `GOAL.md` cron, or an incoming message asks for another iteration.
```

A Wave coordinates. Project loops pursue KR proof. Task loops do scoped
implementation in their own worktrees and report through typed observations,
directive receipts, PR state, and PM state.

## Data Boundaries

Wire DTOs are mirrored by hand across Rust, Python, Swift, and JSON fixtures.
They do not get hidden defaults. A missing field is either a parse error or an
explicit optional value.

Important paths:

- `rust/loopflow/src/lfd/http/dto.rs`
- `python/loopflow/models.py`
- `swift/Loopflow/Models/`
- `tests/fixtures/dto/`

This boundary deserves special care. If a field changes, update every mirror and
the fixture tests in the same unit of work.

## External Systems

Loopflow integrates with:

- Git and worktrees for branch isolation.
- GitHub for PRs, webhook ingress translated to `lf` execs, and release workflows.
- Linear for the Wave → Project → Task planning hierarchy.
- tmux and local processes for Wave, Project, and Task processes.
- Swift/macOS services for Loopflow and native host behavior.

## Where Complexity Collects

- Context assembly: every agent session depends on it, and the sources span
  docs, prompts, skills, wave memory, scratch notes, and command arguments.
- Child control: SQLite, Loopflow, tmux, and external agents must agree on the
  current directive, its provider effect, incorporation, state, and next move.
- DTO parity: Rust, Python, Swift, and fixtures can drift unless changes are
  made as one contract update.
- Product continuity: backend, CLI, UI, docs, tests, and release notes often
  move at different times.

These are the highest-leverage places to simplify first.
