---
layout: default
title: Architecture
---

# Architecture

```text
Human / Loopflow app
    ├── lf commands ───────────────────────┐
    └── Wave Chat ── per-Wave listener     │
                                           ▼
                                  local SQLite store
                                      │          │
                             Project Session  Task Session
                                                   │
                                          worktree + PR to main
```

`lf` is the machine-wide command and JSON interface. `lf wave <name>` is the
resident process for one Wave: it owns that Wave's chat listener, journal,
cadence, memory, and project selection. There is no global service.

## Product model

| Concept | Durable truth | Runtime responsibility |
|---|---|---|
| Wave | `wave/<name>/GOAL.md`, `MEMORY.md`, registry row | Choose Projects, converse, remember, and stay resident |
| Project | Linear Project + local Project Session | Pursue measurable KRs across Tasks |
| Task | Linear issue + local Task Session | Deliver one change through a worktree and PR |
| Directive | local store | Preserve child direction and incorporation proof |
| Trace | local store + `~/.lf/traces` | Record agent launch and conversation evidence |

Every Project belongs to one Wave. Every Task belongs to one Project. Only a
Task Session owns a worktree, branch, and pull request.

## CLI and engine

`lf` resolves skills and flows, assembles context, starts provider CLIs, and
exposes local domain commands. The engine owns reusable prompt execution and
Git primitives; Wave, Project, and Task controllers own lifecycle decisions.

Important paths:

- `rust/loopflow/src/lf/`
- `rust/loopflow/src/engine/`
- `rust/loopflow/src/wave/`
- `rust/loopflow/src/project_session/`
- `rust/loopflow/src/task/`

## Local store

SQLite coordinates Wave identity, PM snapshots, Project and Task Sessions,
commands, directives, event ledgers, provider credentials, and traces. Callers
open it directly. The default path is `~/.lf/loopflow.db`; set `LF_DB_PATH` to
use another path.

Important path:

- `rust/loopflow/src/store/`

## Wave process

```bash
lf wave infrastructure
```

One Wave process serves replay, live turns, and health for that Wave. The Mac
app connects directly to the selected Wave while reading current registry,
Project, and Task state through its bundled `lf`.

Project and Task Sessions are explicit child processes. They share the local
store and durable control channel; they do not call a global HTTP API.

## Delivery truth

Task runners and status commands reconcile pull-request state through `gh`.
Merge correctness does not depend on webhook delivery. A merged Task remains
visible until Linear writeback succeeds or exposes a pending reconciliation.

## Wire contracts

The Mac app invokes `lf --json`. Shared fixtures keep Rust and Swift
representations aligned. Wire fields have no implicit defaults: absence is
either a parse error or an explicit optional value.

Important paths:

- `swift/Loopflow/Models/`
- `tests/fixtures/dto/`
