# LFD

## Vision

`lfd` is not just an HTTP daemon that happens to launch some work. It is the runtime host for loopflow.

The thing that actually executes flows is still the normal `lf` CLI. `lfd` decides when runs start, supervises their processes, persists their state, and streams execution state to clients. Hosting attachable shells / PTYs is a later capability, not the first requirement.

The first clean contract is that `lf` emits structured lifecycle state into a globally agreed-upon runtime store when that store is present. `lfd` is one host around that store. Concerto is one client of it. Automated runs can later start because `lfd` forks normal `lf <flow-or-step>` commands in the correct worktree and executor environment. Interactive shells and PTYs can come after that.

This keeps loopflow CLI-native. If a user runs `lf build` in their favorite TUI, the runtime can still observe it. Concerto does not need to beat someone else's TUI before it becomes useful; it needs to understand and compose the work.

## Why this is a separate wave

This is a runtime architecture shift, not just terminal embedding polish.

`agent-embedding` should consume this model in Concerto. `lfd` should define it.

Without this split, terminal transport, app workspace UX, daemon execution semantics, and CLI/daemon protocol all get tangled into one wave.

## Status

This wave is newly split out. The current `agent-embedding` diff improves the terminal workspace seam and bundled-daemon behavior, but it does **not** finish interactive sessions end to end inside the Swift app, and it does not yet replace the bespoke daemon executor with a shared runtime-store model plus real `lf` process supervision. Treat the docs here as the next build target.

## Strategy

### One execution model

There should be one real execution path:
- `lf design`
- `lf build`
- `lf ship-roadmap`
- `lf ci-fix`

No second bespoke in-daemon flow executor as the long-term architecture.

`lfd` remains responsible for scheduling, supervision, persistence, and fanout. `lf` remains responsible for flow semantics and execution.

### Shared runtime store first

Before daemon-owned PTYs, establish one shared place where execution state lands.

- `lf` writes structured lifecycle events when the store is available
- `lfd` reads and extends that store for supervision and fanout
- Concerto reads the same store for queue, workspace, and history

This is the smallest step that makes loopflow execution feel like a language instead of a daemon-specific protocol.

### Parallel-first runtime

A wave is configuration and grouping. A run is execution.

The runtime model should assume:
- many runs per wave
- many worktrees per wave
- many reactive activations per wave

Serialization remains useful, but only as explicit policy.

### Local terminals before daemon-hosted shells

Local-first use should not wait for daemon PTYs.

If execution state is already flowing into the shared store, Concerto can open ordinary local Ghostty sessions and still present a coherent workspace. That gets embedded local work sooner while keeping the runtime contract simple.

### Daemon-hosted shells

Daemon-owned shells / PTYs are still the right longer-term model, especially for remote work, reconnect, and multi-client attachment.

But they are not step zero. They should arrive after the shared-store contract and real CLI execution path are stable.

Clients should be able to:
- ask for a fresh shell in a repo or worktree
- reattach to an existing shell
- run normal `lf` commands there

This should feel SSH-like in product terms even if the first transport is not literal SSH.

### Structured observation, not terminal scraping

The runtime should not infer execution by scraping terminal text.

Instead, `lf` should report structured events such as:
- command start
- resolved flow / step
- wave / run / session association
- interactive wait points
- completion / failure

That makes manual, automated, and eventually remote execution reconcilable inside one store.

### Concerto is a client, not a second runtime

Concerto should observe, launch, attach, render, and foreground the right run.

It should not need its own launch shim or a private interpretation of how loopflow commands execute.

## Milestone docs

1. `01-tmux-architecture-study.md` — study tmux's server/client split and identify what still matters once local v0 is shared-store observation plus ordinary local terminals
2. `02-daemon-aware-cli-contract.md` — define the shared runtime-store contract and how `lf` discovers and writes to it
3. `03-real-cli-executor.md` — make automated runs spawn normal `lf <flow-or-step>` commands against that same store
4. `04-daemon-hosted-shells.md` — add daemon-owned shells / PTYs when local-first observation is solid and remote transport is the next pressure point

## Boundaries

### `lfd` owns

- triggers and scheduling
- run creation policy
- process supervision
- persistence and event fanout
- optional PTY supervision later
- worktree / session attachment semantics
- reconciliation of waves, runs, sessions, and outcomes

### `lf` owns

- flow expansion
- step execution
- prompt/runtime semantics
- emitting structured lifecycle events when the shared runtime store is available

### Concerto owns

- foregrounding one run for a selected wave
- presenting terminal, work, queue, and portfolio surfaces
- applying calm serialized UX where the product wants it

## Milestones

1. **Shared-store observation**
   - `lf` can discover a runtime store and write structured lifecycle events to it
   - manual CLI runs become visible without going through a bespoke daemon executor

2. **Automated runs via real `lf` processes**
   - `lfd` starts normal `lf <flow-or-step>` commands
   - automated and manual runs converge on one event model

3. **Local client convergence**
   - Concerto consumes the same store and can open ordinary local terminals
   - no new launch shim becomes the product contract

4. **Daemon-hosted PTY / shell model**
   - attach/read/write/resize/close
   - fresh or existing worktree shells
   - reconnect support

5. **Remote access**
   - decide whether remote should begin as SSH into a host/container before inventing a custom PTY transport
   - keep the shared store and CLI contract stable across that move

## Relationship to existing waves

- `wave/agent-embedding/` consumes this runtime model for terminal embedding, workspaces, lifecycle UI, and composition. Near term that means local Ghostty plus shared runtime state; later it can adopt daemon-owned PTYs without changing its higher-level model.
- `wave/chord-model/` depends on this runtime being legible enough that runs, repairs, and signals are tracked consistently.
- `wave/pm/` and other execution-adjacent waves should not need to care whether a run was automated or interactive if the event model is correct.

## Goals

- `lf` can write structured lifecycle events into a shared runtime store
- `lfd` starts normal `lf` commands for automated runs
- attachable daemon-owned shells / PTYs can be added without changing execution semantics
- worktree / run / session attribution is reliable for both automated and interactive execution
- Concerto consumes the same runtime model instead of a custom launch shim

## Risks

- Store contract drift if the event schema is underspecified
- Over-correcting into daemon PTYs too early when local Ghostty plus shared state is enough
- Migration pain while bespoke executor paths and real-CLI execution coexist
- Worktree and run attribution bugs could produce subtle state corruption if correlation is weak
- Interactive and automated execution may diverge again unless tests pin parity aggressively

## Metrics

- manual `lf` runs observed through the shared store: 100% for local opt-in use
- automated runs launched through real `lf` process execution: 100%
- interactive runs tracked without launch-spec shims: 100%
- run/session attribution mismatches: 0
- duplicate execution semantics between `lfd` and `lf`: trending to 0
