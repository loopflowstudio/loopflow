# LFD

## Vision

`lfd` is not just an HTTP daemon that happens to launch some work. It is the runtime host for loopflow.

It decides when runs start, supervises their processes, persists their state, hosts attachable shells / PTYs, and streams execution state to clients. The thing that actually executes flows is still the normal `lf` CLI.

Automated runs start because `lfd` forks normal `lf <flow-or-step>` commands in the correct worktree and executor environment. Interactive runs start because Concerto or SSH-style clients attach to an `lfd`-owned shell and run those same commands by hand.

`lf` detects that it is running under `lfd` and emits the structured lifecycle events `lfd` needs to track waves, runs, sessions, and outcomes.

## Why this is a separate wave

This is a runtime architecture shift, not just terminal embedding polish.

`agent-embedding` should consume this model in Concerto. `lfd` should define it.

Without this split, terminal transport, app workspace UX, daemon execution semantics, and CLI/daemon protocol all get tangled into one wave.

## Status

This wave is newly split out. The current `agent-embedding` diff improves the terminal workspace seam and bundled-daemon behavior, but it does **not** finish interactive sessions end-to-end inside the Swift app, and it does not yet replace the bespoke daemon executor with real `lf` process supervision. Treat the docs here as the next build target.

## Strategy

### One execution model

There should be one real execution path:
- `lf design`
- `lf build`
- `lf build-or-silent`
- `lf ci-fix`

No second bespoke in-daemon flow executor as the long-term architecture.

`lfd` remains responsible for scheduling and supervision. `lf` remains responsible for flow semantics and execution.

### Parallel-first runtime

A wave is configuration and grouping. A run is execution.

The runtime model should assume:
- many runs per wave
- many worktrees per wave
- many reactive activations per wave

Serialization remains useful, but only as explicit policy.

### Daemon-hosted shells

`lfd` should own attachable shells / PTYs.

Clients should be able to:
- ask for a fresh shell in a repo or worktree
- reattach to an existing shell
- run normal `lf` commands there

This should feel SSH-like in product terms even if the first transport is not literal SSH.

### Structured observation, not terminal scraping

`lfd` should not infer execution by scraping terminal text.

Instead, `lf` should detect an `lfd`-managed environment and report structured events such as:
- command start
- resolved flow / step
- wave / run / session association
- interactive wait points
- completion / failure

That makes interactive and automated execution reconcilable inside one store.

### Concerto is a client, not a second runtime

Concerto should attach, render, and foreground the right run.

It should not need its own launch shim or a private interpretation of how loopflow commands execute.

## Milestone docs

1. `01-tmux-architecture-study.md` — study tmux's server/client split and apply the right lessons without copying its whole UI model
2. `02-daemon-aware-cli-contract.md` — make `lf` detect `lfd` and emit structured lifecycle events
3. `03-real-cli-executor.md` — make automated runs spawn normal `lf <flow-or-step>` commands
4. `04-daemon-hosted-shells.md` — make daemon-owned shells / PTYs and SSH-style attach the interactive model

## Boundaries

### `lfd` owns

- triggers and scheduling
- run creation policy
- process / PTY supervision
- persistence and event fanout
- worktree / session attachment semantics
- reconciliation of waves, runs, sessions, and outcomes

### `lf` owns

- flow expansion
- step execution
- prompt/runtime semantics
- emitting structured lifecycle events when inside `lfd`

### Concerto owns

- foregrounding one run for a selected wave
- presenting terminal, work, queue, and portfolio surfaces
- applying calm serialized UX where the product wants it

## Milestones

1. **Daemon-aware `lf` contract**
   - define how `lf` detects `lfd`
   - define event/auth/session correlation contract

2. **Automated runs via real `lf` processes**
   - `lfd` starts normal `lf <flow-or-step>` commands
   - remove duplication between daemon executor and CLI semantics

3. **Daemon-hosted PTY / shell model**
   - attach/read/write/resize/close
   - fresh or existing worktree shells
   - reconnect support

4. **SSH-style access**
   - product-quality terminal access to daemon-owned shells
   - local first, remote-capable later

5. **Client convergence**
   - Concerto and terminal clients both consume the same session model

## Relationship to existing waves

- `wave/agent-embedding/` consumes this runtime model for terminal embedding, workspaces, lifecycle UI, and composition.
- `wave/chord-model/` depends on this runtime being legible enough that runs, repairs, and signals are tracked consistently.
- `wave/pm/` and other execution-adjacent waves should not need to care whether a run was automated or interactive if the event model is correct.

## Goals

- `lfd` starts normal `lf` commands for automated runs
- `lf` can detect `lfd` and emit structured lifecycle events
- attachable daemon-owned shells / PTYs exist as a first-class interface
- worktree / run / session attribution is reliable for both automated and interactive execution
- Concerto consumes the same runtime model instead of a custom launch shim

## Risks

- CLI/daemon protocol drift if the event contract is underspecified
- Over-correcting into literal SSH too early when an SSH-like PTY attach protocol would be enough
- Migration pain while bespoke executor paths and real-CLI execution coexist
- Worktree and run attribution bugs could produce subtle state corruption if correlation is weak
- Interactive and automated execution may diverge again unless tests pin parity aggressively

## Metrics

- automated runs launched through real `lf` process execution: 100%
- interactive runs tracked without launch-spec shims: 100%
- run/session attribution mismatches: 0
- duplicate execution semantics between `lfd` and `lf`: trending to 0
