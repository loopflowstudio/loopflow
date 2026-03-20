---
linear_id: b00f983a-47b2-4ea8-b357-e45e0d183aa3
---
# Wave-Aware CLI Runtime Journal

## Problem

`lfd` cannot see normal `lf` execution unless it owns the run. Today it works around that by executing flows itself and recreating loopflow semantics inside the daemon. That leaves two bad seams:

1. **Execution semantics drift.** `lf` and `lfd` both need to know how flows, routing, loops, and waiting behave.
2. **Manual CLI work disappears.** Running `lf implement` in a wave worktree leaves no durable runtime trace unless `lfd` was already in the middle.

The fix is not “more daemon control.” The fix is one shared runtime substrate that `lf` can write and `lfd` can consume.

## Intent

Keep wave identity and execution semantics in `lf`.

- A **wave** is a durable line of work expressed through a worktree
- A **branch / PR** is the current publication state of that wave
- A **run** is one execution within that wave
- `lfd` observes, schedules, indexes, and fans out runtime state
- `lf` remains the thing that actually executes steps and flows

This milestone is about **durable observation of wave-attributed CLI runs**, not arbitrary repo activity and not yet the full WaveExecutor rewrite.

## Core decision

Use a **durable local runtime journal**, not live HTTP callbacks to a running daemon.

`lf` writes structured lifecycle events to a predefined path. `lfd` tails or imports them when present. If `lfd` is down, the run still leaves a usable trace. If `lfd` starts later, it can ingest after the fact.

This keeps delivery fire-and-forget without making daemon availability part of the contract.

## Scope

### In scope

- Wave-attributed CLI runs write durable runtime journals
- `lfd` can ingest those journals live or later
- Manual CLI runs become visible without going through the daemon executor
- The journal schema is stable enough for later `lfd -> lf` real-process execution

### Out of scope

- Refactoring `WaveExecutor` to spawn real `lf` processes
- Daemon-hosted PTYs / shells
- Remote transport or TLS
- Tracking arbitrary non-wave `lf` runs

## Wave attribution

The journal contract applies only when `lf` can attribute the run to a wave.

### Primary attribution path

Infer the wave from the current worktree using the existing sibling naming contract:

```text
~/src/loopflow            # main repo
~/src/loopflow.lfd        # wave worktree -> wave "lfd"
```

If `lf` is running from `loopflow.lfd`, the wave is `lfd`.

### Non-wave runs

If wave attribution is ambiguous, `lf` behaves normally and does **not** write runtime journal entries. This keeps the model wave-centric instead of turning all CLI usage into daemon data.

In v1, wave runtime integration is worktree-derived only. Running `lf` from the main repo or any other non-wave context is plain standalone CLI behavior with no journal emission and no `lfd` integration for that run.

## Journal layout

Use one directory per run inside the wave worktree:

```text
<worktree>/.lf/runtime/
  runs/
    <run_id>/
      meta.json
      events.jsonl
```

Why this shape:

- no global append-only log shared by many writers
- each `lf` process owns its own run directory
- later `lfd` replay is simple: scan runs, read metadata, tail `events.jsonl`
- concurrent runs do not contend on one file

## Identity

`lf` creates the run id locally when the run starts.

The id should use the same durable ID scheme the daemon already uses (`LfdId`-style, monotonic/type-prefixed if that remains the house style). The CLI should not need to synchronously ask `lfd` for permission to exist.

Later, when `lfd` starts automated runs itself, it may inject a run id for correlation — but that is a follow-on, not a requirement for journal v1.

## Metadata

`meta.json` is the stable summary for discovery and indexing. It should include:

- `run_id`
- `wave_id` or `wave_name`
- `repo`
- `worktree`
- `command`
- `flow` or top-level step
- `started_at`
- optional `target_branch`
- optional parent / repair lineage fields when relevant

`lfd` should be able to list known runs and decide whether to ingest them without opening the event stream first.

## Events

Use newline-delimited JSON in `events.jsonl`.

Each event has:

- `type`
- `timestamp` (RFC3339)
- `run_id`

V1 keeps the event set small:

| Type | When | Fields |
|------|------|--------|
| `run.started` | run begins | `command`, `flow` or `step`, `worktree`, `wave` |
| `step.started` | a flow step begins | `step`, `index` |
| `step.completed` | a flow step finishes | `step`, `index`, `exit_code` |
| `run.waiting` | execution needs human input | `step`, optional session/terminal references |
| `run.completed` | run exits successfully | `exit_code` |
| `run.failed` | run exits with error | `exit_code`, `error` |

For a single-step CLI invocation, `run.started` + terminal event are enough.

## Delivery semantics

Fire-and-forget still holds, but the target is the journal instead of the daemon.

- `lf` appends events locally
- no retry loop
- no daemon handshake
- no blocking on `lfd`
- if journal writes fail, `lf` logs debug output and continues execution

This is observability, not correctness.

## Concurrency model

Avoid the hardest logging problem by refusing to have one shared writer target.

- one run -> one writer
- one writer -> one `events.jsonl`
- many runs can exist at once in the same wave or repo
- `lfd` is the reader/indexer, not an intermediary writer

This makes concurrent writers mostly a non-problem in v1.

## `lfd` behavior

`lfd` becomes a consumer of the journal.

### When already running

- discover journal roots for known wave worktrees
- tail new `events.jsonl` entries
- translate runtime events into `EventHub` updates for WebSocket / Concerto

### When started later

- scan known wave worktrees for run directories
- ingest `meta.json`
- replay `events.jsonl`
- mark imported runs as historical if they already ended

The daemon should own replay cursors and indexing state in its own store. The journal remains append-only runtime evidence, not the query layer.

## Relationship to existing daemon events

The current `lfd` event vocabulary is wave- and agent-centric (`wave_started`, `agent_started`, `wave_waiting`, ...). The journal is run- and step-centric.

That mismatch should be handled at the daemon boundary:

- journal events are the source format
- `lfd` maps them onto existing client-facing events where possible
- if the mapping becomes awkward, add first-class run/step events to `lfd`

Do **not** make `lf` emit daemon-shaped events just to satisfy current UI plumbing.

## Shared implementation

The important split is not “in `lf`” versus “in `lfd`.” The durable pieces should live in shared runtime code:

- event schema
- run ids
- journal paths
- metadata read/write
- event append/read helpers

Then:

- `lf` uses the shared code to write
- `lfd` uses the shared code to read, tail, and import

One contract. Two roles.

## Ownership

### `lf` owns

- wave attribution
- run creation
- flow / step execution semantics
- journal writes

### Shared runtime module owns

- journal layout
- metadata schema
- event schema
- append/read helpers

### `lfd` owns

- scanning and ingestion
- replay cursors
- store indexing
- WebSocket / Concerto fanout
- scheduling and supervision for future daemon-launched runs

## Why not live HTTP for v1

HTTP only simplifies things if daemon availability is part of the contract.

If the desired behavior is:

> run `lf` now, start `lfd` later, still observe the run

then HTTP is the wrong primitive. It makes `lfd` the required runtime receiver instead of an optional consumer.

The journal model matches the wave README more closely:

- shared runtime substrate first
- daemon as host and observer around it
- CLI remains first-class

## Open questions

1. Should the journal root always live inside the worktree, or should there be an overridable shared root for unusual environments?
2. Does `lfd` need first-class `run.*` / `step.*` events immediately, or is mapping onto existing wave/agent events good enough for the first cut?

## Done when

- Running `lf` inside a wave worktree creates a durable runtime journal
- The journal exists whether or not `lfd` is running
- Starting `lfd` later can import completed or in-progress runs from that journal
- If `lfd` is already running, clients can see step progress from the same journal
- Runs without wave attribution do not enter the runtime model
- The later “real CLI executor” milestone can consume this contract instead of inventing a second one
