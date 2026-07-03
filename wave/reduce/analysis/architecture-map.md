---
head: 615729570782d730d2ea3b196e34779db9f63555
status: bootstrap
---

# Architecture Map

## System shape

Loopflow has four product surfaces over one model:

- `lf` - local CLI for running steps, flows, and ops from a checkout.
- `lfd` - daemon for HTTP API, wave scheduling, triggers, sessions, and remote
  execution.
- `lfq` / Python API - lightweight query and control clients for lfd.
- Concerto - Swift app for watching waves, sessions, attention, and live
  work.

The authored repo state is the product language:

- `.lf/steps/` - prompt steps.
- `.lf/flows/` - flow definitions.
- `.lf/directions/` - judgment lenses applied to steps/flows.
- `wave/<name>/GOAL.md` and `wave/<name>/MEMORY.md` - wave intent and durable
  memory.
- `wave/<name>/*.md` or `items/*.md` - roadmap/work items.

The runtime state lives outside that authored layer:

- lfd store - waves, sessions, runs, events, triggers, auth, provider state.
- tmux/processes/Docker - execution backends.
- PM provider - Asana mirror and lifecycle state.
- release artifacts - decision ledgers and archived notes.

## Main code boundaries

```text
README/docs/wave/.lf
        |
        v
rust/loopflow/src/engine       # prompt, flow, step, launch, worktree
        |
        +--> rust/loopflow/src/lf    # `lf` commands
        +--> rust/loopflow/src/ops   # git, PR, PM, release ops
        +--> rust/loopflow/src/lfd   # daemon API, store, scheduler, sessions
        |
        v
python/loopflow                # lfq CLI + Python client models
        |
        v
swift/LoopflowCore, Concerto   # native UI models, stores, views
```

The highest-risk boundary is DTO parity. The same wire concepts appear in
Rust, Python, Swift, and JSON fixtures under `tests/fixtures/dto/`. The style
rule is explicit: wire DTOs have required fields or explicit optional fields,
never hidden defaults.

## Control loops

```text
Wave authoring
  GOAL.md + MEMORY.md + roadmap
        |
        v
lfd scheduler / triggers
  repo, wave, ci_failure, cron, loop ticker
        |
        v
worker/session execution
  flow -> step -> agent process
        |
        v
events, runs, attention, PR/PM state
        |
        v
human and wave reassessment
```

Loopflow's central idea is not "run a prompt." It is persistent work: an
authored goal, recurring assessment, delegated execution, and visible state.

## State ownership

| State | Owner | Notes |
|---|---|---|
| Product docs | repo | README, docs, testing, visual design |
| Prompt language | repo | `.lf/steps`, `.lf/flows`, `.lf/directions` |
| Wave intent | repo | `wave/<name>/GOAL.md`, `MEMORY.md` |
| Wave runtime | lfd store | status, sessions, events, triggers |
| Agent execution | engine/executor | local process, tmux, Docker |
| PM truth | provider | Asana can arbitrate picks |
| UI state | Swift stores | derived from lfd and local preferences |

## Current architectural tension

- The user model is "waves of persistent work"; some code and docs still expose
  older one-shot step/flow language first.
- `lf`, `lfd`, `lfq`, and Concerto share concepts but not one generated DTO
  model. Fixture tests help, but drift remains a standing risk.
- Loopflow is starting to use itself as a governance system. That is a product
  feature and a test harness; the architecture should make that path first
  class rather than incidental.
- Prompt/context creation is the multiplier. Every worker, wave, and review
  inherits its quality.

## Missing detail for next refresh

- Draw the exact lfd store schema and migrations.
- Trace one full `lf <flow>` execution from CLI parse to agent launch.
- Trace one full Concerto session update from HTTP/WebSocket event to rendered
  view.
- Record which docs are source of truth for each product concept.
