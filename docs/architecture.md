---
layout: default
title: Architecture
---

# Architecture

```text
Human ── Wave Chat ──▶ Wave
                       │ selects and directs
                       ▼
                 Project Session
                       │ supervises
                       ▼
                   Task Session ──▶ stable worktree + serial PRs to main
                       │
                       └──────────▶ diff/file/terminal inspector

Wave ─ ─ ─ ─ root inspection and override ─ ─ ─ ─ ─ ┘
```

`lf` is the machine-wide command and JSON interface. `lf wave <name>` is the
resident process for one Wave: it owns that Wave's chat listener, journal,
cadence, memory, and project selection. There is no global service.

## Product model

| Concept | Durable truth | Runtime responsibility |
|---|---|---|
| Wave | `wave/<name>/GOAL.md`, `MEMORY.md`, registry row | Choose Projects, converse, remember, and stay resident |
| Project | Linear Project + local Project Session | Pursue measurable KRs across Tasks |
| Task | Linear issue + local Task Session | Advance concrete work through zero or more serial PRs |
| Directive | local store | Preserve child direction and incorporation proof |
| Trace | local store + `~/.lf/traces` | Record agent launch and conversation evidence |

Every Project belongs to one Wave. Every Task belongs to one Project and one
durable Project Session. Only a Task Session owns a worktree. Its ordered PRs
own the serial branches and GitHub history that advance it.

There is one supervision path: Wave → Project Session → Task Session. The Wave
retains root authority to inspect or override any descendant, but that command
source does not bypass or replace the Task's Project Session. Loopflow creates
no default Project. Free-text `lf task start` requires `--project`, creates the
Linear issue, ensures the Project Session, then creates the Task Session and
worktree. `lf task run` does the same for an existing Linear issue. A newly
reserved Project Session does not block Task launch on another provider turn;
the Task's first consequential event wakes it through the observation outbox.

Task worktrees are flat siblings even when work is dependent. `lf task run
CHILD --stack-on PARENT` forks CHILD from PARENT's active published PR and
records the parent PR id plus exact fork commit. CHILD's PR targets PARENT's
branch until merge, then deterministically replays only CHILD-authored commits
onto `main`. Same-Task PRs remain serial; concurrent stack nodes are Tasks.
The Task argument is an ergonomic lookup: placement persists the active PR id,
so a later serial PR on PARENT does not move CHILD's dependency.

Free-text Project and Task starts verify the owning Wave before mutating
Linear. They refresh the PM snapshot before creating local runtime state; a
post-commit refresh failure reports the committed id and leaves no Session or
worktree created by that attempt to reconcile.

Wave and Project turns run from the clean canonical main checkout as a control
plane. They read, decide, and create or steer children there; repository edits
belong to Task worktrees. Commands fail before provider launch when that main
checkout is dirty or the caller is in another checkout.

## CLI and engine

`lf` resolves skills and flows, assembles context, starts provider CLIs, and
exposes local domain commands. The engine owns reusable prompt execution and
Git primitives; Wave, Project, and Task controllers own lifecycle decisions.

Each controller runs one bounded `clarify → pursue → mutate` flow. The three
skills remain separate because they have separate jobs; no skill owns a loop
bit. After the pass, the domain controller inspects durable truth. A Task
continues, rotates after a merged or abandoned PR, waits for review, or ends on
explicit Task completion. A Project repeats,
waits on Tasks, blocks on no progress, or completes when every current KR
holds. A Wave returns to its resident idle state and wakes on human input,
cadence, or child observations.

Important paths:

- `rust/loopflow/src/lf/`
- `rust/loopflow/src/engine/`
- `rust/loopflow/src/wave/`
- `rust/loopflow/src/project_session/`
- `rust/loopflow/src/task/`

## Local store

SQLite coordinates Wave identity, PM snapshots, Project and Task Sessions,
ordered Task PRs, commands, directives, event ledgers, provider credentials,
and traces. Callers open it directly. Installed release builds share
`~/.lf/loopflow.db`; set `LF_DB_PATH` to use another path.

Builds made from a source checkout use
`~/.lf-dev/worktrees/<source-identity>/loopflow.db`, so branches cannot migrate
one another's development store. They refuse `~/.lf/loopflow.db` even when it
arrives through `LF_DB_PATH`; `LF_ALLOW_PRODUCTION_DB_FROM_DEV=1` is the
break-glass override. Release packaging stamps its provenance explicitly.

Session runners carry their pinned binary and store through the internal
`LF_CONTROL_BIN`, `LF_CONTROL_HOME`, and `LF_CONTROL_DB_PATH` variables. Provider
agents do not inherit `LF_BIN`, `LF_HOME`, `LF_DB_PATH`, or the break-glass flag.
Before an existing on-disk database advances, SQLite writes an atomic backup
named for the previously applied migration.

A Task PR persists evidence, not a mutable state label. No publication evidence
means Working. A publication request without a GitHub receipt means Publishing;
the receipt makes it Open; merge and abandonment have their own terminal
evidence. The GitHub receipt is nested under publication, so GitHub cannot exist
without the durable request that explains `after_merge`.

Important path:

- `rust/loopflow/src/store/`

## Wave process

```bash
lf wave infrastructure
```

One Wave process serves replay, live turns, and health for that Wave. The Mac
app connects directly to the selected Wave while reading current registry,
Project, and Task state through its bundled `lf`.

Selecting a Task opens its worktree inspector. `lf task changes/diff/file`
owns Git and path semantics; Swift renders those typed snapshots and keeps
Task-scoped Ghostty/tmux terminals as presentation state. The terminal
multiplexer never owns Task lifecycle or worktree identity.

Project and Task Sessions are explicit child processes. They share the local
store and durable control channel; they do not call a global HTTP API.

## PR truth

Task runners and status commands reconcile pull-request state through `gh`.
Merge correctness does not depend on webhook delivery. A merge settles one PR;
its recorded `after_merge` disposition decides whether the runner rotates the
same worktree or completes the Task. A completed Task remains visible until
Linear writeback succeeds or exposes a pending reconciliation. A completing
merge settles the PR and completes the Task in one SQLite transaction.

## Wire contracts

The Mac app invokes `lf --json`. Shared fixtures keep Rust and Swift
representations aligned. Wire fields have no implicit defaults: absence is
either a parse error or an explicit optional value.

Task snapshots expose ordered `prs` plus `active_pr`. Each PR includes its
derived phase, active-worktree emptiness when knowable, publication disposition,
nested GitHub receipt, merge commit, and abandonment time. Rust and Swift can
therefore answer whether the current PR exists on GitHub and whether merging it
completes the Task.

Important paths:

- `swift/Loopflow/Models/`
- `tests/fixtures/dto/`
