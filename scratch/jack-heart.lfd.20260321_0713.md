# LFD: Session Lifecycle + Client Access

Combined design for items 01 (Real CLI Executor), 02 (Daemon-Hosted Shells), and the emerging harness server mode.

## Problem

`lfd` needs to own session lifecycle without mediating terminal bytes. The original design tried to bridge tmux I/O through `lfd` over WebSocket — capture-pane polling, control mode, byte-level forwarding. That's the wrong layer. Terminal clients (Ghostty) should connect to tmux directly. Non-terminal clients (iPhone) need a higher-level structured API, not a terminal emulator.

## What's done

### Real CLI Executor — shipped

| Piece | Status | Where |
|-------|--------|-------|
| `WaveExecutor` spawns `lf` CLI | Done | `lfd/executor/wave/mod.rs` — `build_lf_step_command()`, tmux + direct paths |
| Environment injection (`LFD_WAVE_ID`, `LF_RUN_ID`, `LFD_SESSION_ID`) | Done | `flow_step_env()` at line 715 |
| Journal system (`LfEvent`, `emit()`, `RunContext`) | Done | `journal/mod.rs` |
| `LfObserver` polling loop (1s interval, JSONL ingest) | Done | `lfd/journal.rs` |
| Escalation as distinct event type (`*.escalated`) | Done | `LfEventType::Escalated`, `StepEscalated`, `RunEscalated` in event types |
| tmux session creation + exit tracking | Done | `launch_tmux_terminal_session()`, `wait_for_tmux_session_exit()` |
| `TerminalSession` metadata lifecycle | Done | `terminal_session.rs` — Pending → Running → Succeeded/Failed/Canceled |
| Old bespoke executor deleted | Done | `RuntimeRun`, `RuntimeEvent`, etc. removed |
| `FlowEngine` shared between CLI and daemon | Done | `CliExecutor` (in-process) and `DaemonFlowExecutor` (process-supervised) |

### Gaps in the shipped executor

| Gap | Impact |
|-----|--------|
| No regression test suite for executor parity | Design calls for tests on serialized vs parallel waves, queued activations, CI-fix runs, cancellation, failure propagation, run-scoped overrides. None exist yet. |
| Attach endpoint returns launch spec, not connection info | `POST /terminal-sessions/{id}/attach` returns `TerminalLaunchSpecDto` (argv to run `tmux attach-session` locally). Client knows about tmux. |

## Architecture

Three access planes. `lfd` participates differently in each.

### Terminal plane

Full interactive terminal access. Ghostty connects to tmux directly.

- **Local:** Ghostty runs `tmux attach-session -t <name>`. No intermediary.
- **Remote:** Ghostty SSHs to the host, then `tmux attach-session`. SSH is standard machine access — the user manages keys and connection profiles.

`lfd` is not in the data path. It owns session lifecycle (create, track, destroy) and tells clients what to connect to. It never touches terminal bytes.

Auth: SSH keys for remote. Filesystem permissions for local. `lfd` doesn't broker the SSH connection.

### Structured plane

Non-terminal interaction with agent sessions. `lfd` runs the agent harness in server mode and exposes a higher-level API.

This is the interface for clients that can't be terminal participants — iPhone, web, or any UI built on the harness protocol rather than raw terminal access. The API surface is the agent's actual interaction points: tool calls, questions, approvals, structured output. Not terminal bytes rendered in a web view.

Auth: OAuth / tokens. Same auth model as the existing WebSocket connection and `lfd` API endpoints.

### Event plane

Metadata stream over WebSocket. Already exists.

Status updates, journal events, output lines, terminal session lifecycle events. Both terminal and non-terminal clients consume this. The `connected` event carries a full state snapshot; every mutation emits an event.

Auth: OAuth / tokens. Already built.

### How the planes relate

| Client | Terminal | Structured | Events |
|--------|----------|------------|--------|
| macOS Concerto (Ghostty) | SSH + tmux attach | — | WebSocket |
| iPhone Concerto | — | Harness API | WebSocket |
| `lfq` CLI | — | — | HTTP + WebSocket |

Terminal clients go around `lfd` for I/O. Non-terminal clients go through `lfd` at a higher level. Both get the event stream.

## Approach

Two milestones. The first is a contract cleanup. The second is the new capability.

### Milestone 1: Connection info contract

Replace `TerminalLaunchSpecDto` with transport-agnostic connection metadata.

The attach endpoint returns:

```rust
struct TerminalConnectionInfo {
    session_name: String,      // tmux session name
    host: String,              // hostname (localhost for local)
    cwd: PathBuf,              // working directory
    status: SessionStatus,     // current session state
}
```

Concerto uses this to decide how to connect: local `tmux attach`, or SSH + `tmux attach` for remote hosts. The decision is in the client, not the daemon.

Delete `TerminalLaunchSpecDto` and the argv-returning code path.

### Milestone 2: Harness server mode

`lfd` runs agent sessions with a structured API for non-terminal clients.

The agent harness (Claude Code, Codex, etc.) runs in a mode where `lfd` mediates interactions rather than a terminal:

- Agent requests tool approval → `lfd` forwards to client, collects response
- Agent asks a question → `lfd` forwards to client, collects answer
- Agent produces output → `lfd` streams structured output to client
- Client sends input → `lfd` routes to agent

This is the custom harness in server mode. The protocol is the agent's interaction protocol, not a terminal protocol.

Scope, API shape, and harness integration details TBD — this needs its own design pass once milestone 1 lands.

## Auth model

Two auth mechanisms for two access patterns:

| Access | Auth | Managed by |
|--------|------|------------|
| Terminal (SSH + tmux) | SSH keys | User / system sshd |
| API (WebSocket, harness, HTTP) | OAuth tokens | `lfd` (existing flows) |

SSH auth is the user's responsibility. If you can SSH to the machine, you can attach to sessions. `lfd` doesn't embed an SSH server or broker SSH connections.

OAuth stays for API access. The existing provider auth flows (GitHub, Claude, Codex) are orthogonal — they authenticate with services, not with the machine.

## Alternatives considered

| Approach | Why not |
|----------|---------|
| WebSocket byte bridge (capture-pane polling) | Lossy — output between polls is lost. Requires diffing screen snapshots or sending full screen every 100ms. Reinvents terminal transport poorly. |
| tmux control mode through `lfd` | Only iTerm2 ships a control mode client. Protocol is stable but essentially unproven outside one app. And unnecessary — Ghostty already renders tmux natively. |
| Embedded SSH server in `lfd` (russh) | Adds complexity for no gain. System sshd already works. Remote access = "can you SSH to this machine." |
| Terminal I/O for mobile clients | Wrong abstraction. Mobile needs structured agent interaction, not a terminal emulator. A 4-inch screen showing raw terminal output is bad UX regardless of transport quality. |

## Key decisions

**`lfd` never touches terminal bytes.** Terminal I/O flows between Ghostty and tmux directly. `lfd` owns lifecycle, not transport.

**SSH is a client concern.** Remote terminal access is Concerto connecting via SSH. `lfd` doesn't know or care whether the client is local or remote.

**Two auth models, cleanly split.** SSH for terminal access. OAuth for API access. No attempt to unify them.

**Structured API for non-terminal clients.** Mobile and web get a purpose-built harness API, not degraded terminal access.

**tmux stays as PTY owner.** `lfd` is a supervisor, not a terminal multiplexer.

## Scope

### Milestone 1 (contract cleanup)

- In scope: Replace `TerminalLaunchSpecDto` with `TerminalConnectionInfo`, delete argv-returning code, update Concerto to use connection info
- Out of scope: Remote SSH, harness server mode, new auth flows

### Milestone 2 (harness server mode)

- In scope: Structured agent API, harness integration, non-terminal client support
- Out of scope: Detailed design TBD after milestone 1

### Separate work

- Executor regression tests (serialized/parallel waves, queued activations, cancellation, failure propagation, run-scoped overrides) — not part of either milestone, own backlog item

## Done when

### Milestone 1

- `TerminalLaunchSpecDto` is deleted
- Attach endpoint returns `TerminalConnectionInfo`
- Concerto connects to tmux sessions using connection info (local attach works)
- No terminal bytes flow through `lfd`

### Milestone 2

- `lfd` can run an agent session in server mode
- Non-terminal clients can interact with agent sessions through the structured API
- iPhone Concerto can observe and interact with running sessions without terminal access
