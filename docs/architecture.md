---
layout: default
title: Architecture
---

# Architecture

Loopflow turns repo-authored intent into persistent agent work. Steps and flows
run one job. Waves keep working: they remember, dispatch workers, respond to
what's spoken into their thread, surface attention, and leave an auditable
trail.

## System Map

```text
Authoring layer
  README.md, docs/
  .lf/steps/*.md
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
  rust/.../ops              HTTP read surface, sessions,
                            webhooks-as-speech, worktree
                            janitor, token refresh
        |                        |
        v                        v
Git/PR/PM ops             Clients
                            lf CLI
                            Concerto Swift app
                            webhooks
```

## Core Concepts

| Concept | Stored in | Runtime owner |
|---|---|---|
| Step | `.lf/steps/` and built-ins | `engine` |
| Flow | `.lf/flows/` and built-ins | `engine` |
| Direction | `.lf/directions/` | `engine` prompt assembly |
| Wave goal | `wave/<name>/GOAL.md` | `lf wave` server + resident mind |
| Wave memory | `wave/<name>/MEMORY.md` | wave agent |
| Roadmap item | Asana | `lf op pm` and wave flows |
| Session | lfdb | `lf q` dispatch (tmux-wrapped `lf`) |
| Run/event | lfdb | lfd HTTP/event stream |
| Attention | lfdb | lfd + Concerto |

## CLI and Engine

`lf` is the local command runner. It resolves a step or flow, assembles context,
launches the configured coding agent, and runs ops such as commit, rebase, PR,
PM sync, and release.

Important paths:

- `rust/loopflow/src/bin/lf.rs`
- `rust/loopflow/src/lf/`
- `rust/loopflow/src/engine/`
- `rust/loopflow/src/ops/`

The engine owns the common language: prompts, flows, forks, worktrees, built-in
steps, skills, structured replies, and launch behavior. Ops wrap concrete
side-effectful workflows around git, PRs, PM providers, and release artifacts.

## Daemon

`lfd` is the gatekeeper: it serves read routes and the event push, verifies
GitHub webhooks and speaks them inward as `lf` execs, refreshes provider
tokens, and tidies the registry at boot. It dispatches no work — `lf q`
launches workers, and each wave's resident mind owns its own loop and cron
schedules.

Important paths:

- `rust/loopflow/src/bin/lfd.rs`
- `rust/loopflow/src/lfd/http/`
- `rust/loopflow/src/lfdb/`
- `rust/loopflow/src/lfd/triggers/` (token refresh — the one surviving loop)
- `rust/loopflow/src/lfd/executor/` (dispatch helpers shared with `lf q`, worktree janitor)
- `rust/loopflow/src/lfd/types/`

Native mode uses sqlite and a local session token; container mode uses
postgres for shared or remote hosts.

## Clients

`lf` and Concerto read lfd state; webhooks push events in. The Python package
is a library of wire models only.

Important paths:

- `python/loopflow/models.py`

Concerto is the Swift app. It reads lfd state, renders waves and sessions, and
provides native surfaces for attention, terminal workspaces, provider auth, and
live output.

Important paths:

- `swift/LoopflowCore/Models/`
- `swift/LoopflowCore/State/`
- `swift/LoopflowCore/Services/`
- `swift/Concerto/Views/`

## Wave Loop

```text
1. Read GOAL.md, MEMORY.md, roadmap, and relevant docs.
2. Assess current wave state and anything spoken into the thread.
3. Pick one move: study, ingest, dispatch, unblock, review, or wait.
4. Run a step or flow, often by dispatching a worker.
5. Record events, update PM/repo state, and surface attention.
6. Loop when mode, a `GOAL.md` cron, or an incoming message asks for another pass.
```

A wave agent coordinates. Workers do scoped implementation and report back
through PRs, PM state, events, and memory updates.

## Data Boundaries

Wire DTOs are mirrored by hand across Rust, Python, Swift, and JSON fixtures.
They do not get hidden defaults. A missing field is either a parse error or an
explicit optional value.

Important paths:

- `rust/loopflow/src/lfd/http/dto.rs`
- `python/loopflow/models.py`
- `swift/LoopflowCore/Models/`
- `tests/fixtures/dto/`

This boundary deserves special care. If a field changes, update every mirror and
the fixture tests in the same unit of work.

## External Systems

Loopflow integrates with:

- Git and worktrees for branch isolation.
- GitHub for PRs, webhooks spoken inward as `lf` execs, and release workflows.
- Asana, Linear, and Notion for PM-backed wave roadmaps.
- tmux and local processes for interactive sessions.
- Docker and postgres for hosting the daemon on remote hosts.
- Swift/macOS services for Concerto and native host behavior.

## Where Complexity Collects

- Context assembly: every agent session depends on it, and the sources span
  docs, prompts, skills, wave memory, scratch notes, and command arguments.
- Session lifecycle: lfd, Concerto, tmux, and external agents must agree on
  what is running, blocked, attachable, complete, or failed.
- DTO parity: Rust, Python, Swift, and fixtures can drift unless changes are
  made as one contract update.
- Product continuity: backend, CLI, UI, docs, tests, and release notes often
  move at different times.

These are the highest-leverage places to simplify first.
