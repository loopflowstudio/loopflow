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

<<<<<<< HEAD
### tmux lessons, applied

The tmux architecture study (shipped, guidance propagated into remaining items and `agent-embedding/06`) established which ideas to borrow and which to avoid. The actionable design choices that flow from it:

- **Monotonic, type-prefixed, never-reused IDs** for sessions and runs. tmux does this with `$session`, `@window`, `%pane`. Loopflow's `LfdId` scheme already fits.
- **Server owns all persistent state.** Clients are disposable renderers. `lfd` owns run state, session state, scrollback buffers. Concerto reconstructs on reconnect.
- **Structured event protocol, not terminal scraping.** tmux control mode separates command-response from async notifications with `%begin`/`%end` framing. `lf` → `lfd` events should follow the same principle: typed messages, correlation IDs, fire-and-forget delivery that never blocks execution.
- **Flow control for high-output sessions.** tmux's `pause-after` prevents slow clients from killing sessions. Agent output can be enormous. Event delivery must be resilient to slow consumers.
- **Multi-client size negotiation is a server policy** (`smallest`/`largest`/`latest`/`manual`). Mobile + desktop coexistence needs this from the start.
- **Auth is filesystem permissions locally, tokens remotely.** Start simple (Unix socket permissions), add cryptographic auth when remote access arrives.
- **WezTerm's Domain abstraction** (local/SSH/socket/TLS all implement the same interface) is the model for transport-agnostic session access.
- **Mosh's state sync** (sync current screen, not replay bytes) is the model for late-joining observers of agent sessions.

=======
>>>>>>> eb790e5f (concerto: stabilize bundled daemon terminal handoff)
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

<<<<<<< HEAD
1. `02-daemon-aware-cli-contract.md` — define the shared runtime-store contract and how `lf` discovers and writes to it
2. `03-real-cli-executor.md` — make automated runs spawn normal `lf <flow-or-step>` commands against that same store
3. `04-daemon-hosted-shells.md` — add daemon-owned shells / PTYs when local-first observation is solid and remote transport is the next pressure point
=======
1. `01-tmux-architecture-study.md` — study tmux's server/client split and apply the right lessons without copying its whole UI model
2. `02-daemon-aware-cli-contract.md` — make `lf` detect `lfd` and emit structured lifecycle events
3. `03-real-cli-executor.md` — make automated runs spawn normal `lf <flow-or-step>` commands
4. `04-daemon-hosted-shells.md` — make daemon-owned shells / PTYs and SSH-style attach the interactive model
>>>>>>> eb790e5f (concerto: stabilize bundled daemon terminal handoff)

## Three roles

lfd does three things. All three are listen-and-react, not request-response.

<<<<<<< HEAD
### 1. Watch
=======
- triggers and scheduling
- run creation policy
- process / PTY supervision
- persistence and event fanout
- worktree / session attachment semantics
- reconciliation of waves, runs, sessions, and outcomes
>>>>>>> eb790e5f (concerto: stabilize bundled daemon terminal handoff)

Filesystem, git state, shared runtime store → emit events.

<<<<<<< HEAD
`lf` writes structured lifecycle events into the store. lfd watches the store and fans out events over WebSocket. Concerto subscribes and mirrors. No polling, no refresh endpoints.
=======
- flow expansion
- step execution
- prompt/runtime semantics
- emitting structured lifecycle events when inside `lfd`
>>>>>>> eb790e5f (concerto: stabilize bundled daemon terminal handoff)

### 2. React

Triggers fire, cron ticks, CI fails → lfd spawns `lf` runs. Same mechanism as a human typing `lf build` in a shell — lfd just types it automatically.

### 3. Sync

External world (GitHub webhooks, PM providers, OAuth flows) → update local state and emit events. Auth is the one flow where lfd genuinely brokers something `lf` can't (OAuth callbacks need a server).

### What this means for the API

Most of the current HTTP surface disappears. If the user runs `lf` commands in a shell:

- No `POST /waves/{id}/run` — they ran `lf build`
- No `POST /waves/{id}/stop` — they hit Ctrl-C
- No `POST /waves/{id}/land` — they ran `lf ops land`
- No `POST /waves/{id}/next` — they ran `lf ops next`
- No `PATCH /waves/{id}` — they edited the yaml
- No `POST /terminal-sessions/*` — Concerto manages tmux locally

What stays:
- **WebSocket event stream** — the primary interface. `connected` event carries full state snapshot (waves, attention, terminal sessions, worktrees, runs). Every mutation emits an event. Concerto never needs to GET.
- **Auth flows** — `POST /auth/{provider}` stays because OAuth needs a server
- **Content pulls** — diff content, log replay, usage analytics. State is pushed, content is pulled on demand.

### Boundaries

**lfd owns**: watching, reacting, syncing. Triggers, scheduling, process supervision, persistence, event fanout, external integrations.

**`lf` owns**: flow expansion, step execution, structured lifecycle events.

**Concerto owns**: tmux session management (local shells), pane layout, rendering the event stream into a workspace UI.

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
