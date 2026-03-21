# LFD

## Vision

The runtime host for loopflow.

`lfd` decides when runs start, supervises their processes, persists their state, and streams execution state to clients. The thing that actually executes flows is still the normal `lf` CLI. Hosting attachable shells / PTYs is a later capability, not the first requirement.

The first clean contract is that `lf` emits structured lifecycle state into a globally agreed-upon runtime store when that store is present. `lfd` is one host around that store. Concerto is one client of it. Automated runs can later start because `lfd` forks normal `lf <flow-or-step>` commands in the correct worktree and executor environment. Interactive shells and PTYs can come after that.

This keeps loopflow CLI-native. If a user runs `lf build` in their favorite TUI, the runtime can still observe it. Concerto does not need to beat someone else's TUI before it becomes useful; it needs to understand and compose the work.

## Why this is a separate wave

This is a runtime architecture shift, not just terminal embedding polish.

`agent-embedding` should consume this model in Concerto. `lfd` should define it.

Without this split, terminal transport, app workspace UX, daemon execution semantics, and CLI/daemon protocol all get tangled into one wave.

## Strategy

### One execution model

There is one real execution path. `FlowEngine` drives flow sequencing, xor routing, loops, and fork/and for both the CLI (`CliExecutor`) and daemon (`DaemonFlowExecutor`). They differ only in step execution: in-process for CLI, process-supervised for daemon.

`lfd` remains responsible for scheduling, supervision, persistence, and fanout. `lf` remains responsible for flow semantics and execution. The daemon spawns `lf <step> -b` for headless steps and hosts tmux sessions for interactive steps, injecting `LFD_WAVE_ID`, `LFD_RUN_ID`, `LF_RUN_ID`, and `LFD_SESSION_ID` for run correlation.

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

### Three access planes

`lfd` participates differently in each plane. The terminal plane connection contract is shipped; the structured plane is next.

**Terminal plane.** Full interactive terminal access. Ghostty connects to tmux directly — locally via `tmux attach-session -t <name>`, remotely via SSH. `lfd` owns session lifecycle (create, track, destroy) and returns transport-agnostic connection metadata (`session_name`, `host`, `cwd`, `status`). Concerto decides whether to attach locally or over SSH. `lfd` never touches terminal bytes.

**Structured plane.** Non-terminal interaction with agent sessions. `lfd` runs the agent harness in server mode and exposes a higher-level API — tool calls, questions, approvals, structured output. This is the interface for clients that can't be terminal participants (iPhone, web). Not terminal bytes in a web view.

**Event plane.** Metadata stream over WebSocket. Already exists. The `connected` event carries a full state snapshot; every mutation emits an event. Both terminal and non-terminal clients consume this.

| Client | Terminal | Structured | Events |
|--------|----------|------------|--------|
| macOS Concerto (Ghostty) | SSH + tmux attach | — | WebSocket |
| iPhone Concerto | — | Harness API | WebSocket |
| `lfq` CLI | — | — | HTTP + WebSocket |

### Local terminals before daemon-hosted shells

Local-first use does not wait for daemon PTYs. Execution state flows into the shared store via journal events, and Concerto can open ordinary local Ghostty sessions while presenting a coherent workspace. The minimal interactive-step path (tmux sessions with execution cursor persistence) is shipped; full PTY ownership is next.

Journal ingestion is currently poll-based (file scan). Push-based ingestion (inotify/kqueue or IPC) is a natural optimization once the event schema and daemon-hosted shell transport stabilize.

### Daemon-hosted shells

Daemon-owned shells / PTYs are still the right longer-term model, especially for remote work, reconnect, and multi-client attachment.

But they are not step zero. They should arrive after the shared-store contract and real CLI execution path are stable.

Clients should be able to:
- ask for a fresh shell in a repo or worktree
- reattach to an existing shell
- run normal `lf` commands there

This should feel SSH-like in product terms even if the first transport is not literal SSH.

### tmux lessons, applied

The tmux architecture study (shipped, see git history) established which ideas to borrow and which to avoid. The actionable design choices that flow from it:

- **Monotonic, type-prefixed, never-reused IDs** for sessions and runs. tmux does this with `$session`, `@window`, `%pane`. Loopflow's `LfdId` scheme already fits.
- **Server owns all persistent state.** Clients are disposable renderers. `lfd` owns run state, session state, scrollback buffers. Concerto reconstructs on reconnect.
- **Structured event protocol, not terminal scraping.** tmux control mode separates command-response from async notifications with `%begin`/`%end` framing. `lf` → `lfd` events should follow the same principle: typed messages, correlation IDs, fire-and-forget delivery that never blocks execution.
- **Flow control for high-output sessions.** tmux's `pause-after` prevents slow clients from killing sessions. Agent output can be enormous. Event delivery must be resilient to slow consumers.
- **Multi-client size negotiation is a server policy** (`smallest`/`largest`/`latest`/`manual`). Mobile + desktop coexistence needs this from the start.
- **Auth is filesystem permissions locally, tokens remotely.** Start simple (Unix socket permissions), add cryptographic auth when remote access arrives.
- **WezTerm's Domain abstraction** (local/SSH/socket/TLS all implement the same interface) is the model for transport-agnostic session access.
- **Mosh's state sync** (sync current screen, not replay bytes) is the model for late-joining observers of agent sessions.

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

## Three roles

lfd does three things. All three are listen-and-react, not request-response.

### 1. Watch

Filesystem, git state, shared runtime store → emit events.

`lf` writes structured lifecycle events into the store. lfd watches the store and fans out events over WebSocket. Concerto subscribes and mirrors. No polling, no refresh endpoints.

### 2. React

Triggers fire, cron ticks, CI fails → lfd spawns `lf` runs. Same mechanism as a human typing `lf build` in a shell — lfd just types it automatically.

### 3. Sync

External world (GitHub webhooks, PM providers, OAuth flows) → update local state and emit events. Auth is the one flow where lfd genuinely brokers something `lf` can't (OAuth callbacks need a server).

### What this means for the API

Most of the current HTTP surface disappears. If the user runs `lf` commands in a shell:

- No `POST /waves/{id}/run` — they ran `lf build`
- No `POST /waves/{id}/stop` — they hit Ctrl-C
- No `POST /waves/{id}/land` — they ran `lf op land`
- No `POST /waves/{id}/next` — they ran `lf op next`
- No `PATCH /waves/{id}` — they edited the yaml
- No daemon-proxied terminal byte routes — Concerto attaches to tmux locally or over SSH from `lfd` connection metadata

What stays:
- **WebSocket event stream** — the primary interface. `connected` event carries full state snapshot (waves, attention, terminal sessions, worktrees, runs). Every mutation emits an event. Concerto never needs to GET.
- **Auth flows** — `POST /auth/{provider}` stays because OAuth needs a server
- **Terminal session lifecycle + attach metadata** — `lfd` still creates, tracks, and tears down tmux-backed sessions, then returns `session_name` / `host` / `cwd` / `status` so clients can attach directly
- **Content pulls** — diff content, log replay, usage analytics. State is pushed, content is pulled on demand.

### Boundaries

**lfd owns**: watching, reacting, syncing. Triggers, scheduling, process supervision, persistence, event fanout, external integrations.

**`lf` owns**: flow expansion, step execution, structured lifecycle events.

**Concerto owns**: tmux session management (builds local `tmux attach` or remote `ssh ... tmux attach` commands from connection info), pane layout, rendering the event stream into a workspace UI.

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
