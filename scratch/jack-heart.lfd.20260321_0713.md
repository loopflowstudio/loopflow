# LFD: Real CLI Executor + Daemon-Hosted Shells

Combined design for items 01 (Real CLI Executor) and 02 (Daemon-Hosted Shells).

## Problem

`lfd` needs to be a process supervisor around the real `lf` CLI, not a parallel flow interpreter. And clients need to attach to daemon-owned shells through `lfd` instead of touching tmux directly. These two items are sequential — the executor convergence landed first, and daemon-hosted shells build on top of it.

## What's done

The Real CLI Executor is substantially complete. The daemon-hosted shell attach protocol has not started.

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

### Daemon-Hosted Shells — not started

| Piece | Status | Notes |
|-------|--------|-------|
| WebSocket attach protocol (I/O bytes, input, resize, detach) | Not built | WS carries metadata events only |
| tmux `send-keys` routing through `lfd` | Not built | Concerto runs `tmux attach-session` directly |
| tmux `capture-pane` / output streaming | Not built | No byte-level I/O |
| `tmux resize-window` via `lfd` | Not built | No size negotiation |
| Multi-client size negotiation (latest/smallest/read-only) | Not built | — |
| TLS + token auth for remote attach | Not built | Local filesystem permissions only |
| SSH-style remote shell access | Not built | — |

### Gaps in the "done" executor

| Gap | Impact |
|-----|--------|
| No regression test suite for executor parity | Design calls for tests on serialized vs parallel waves, queued activations, CI-fix runs, cancellation, failure propagation, run-scoped overrides. None exist yet. |
| Attach endpoint returns launch spec, not I/O channel | `POST /terminal-sessions/{id}/attach` returns `TerminalLaunchSpecDto` (argv to run `tmux attach-session` locally). Client still touches tmux directly. |
| `WaveExecutor` still has supervision complexity that could shrink | Process lifecycle works, but convergence isn't "trending to zero duplicate logic" yet — the executor still builds shell commands, manages exit files, polls `has-session`. |

## Approach

One milestone that delivers the attach protocol end-to-end. After this, Concerto attaches to daemon-owned tmux sessions through `lfd` without touching tmux directly.

### 1. WebSocket attach sub-protocol

Extend the existing WS connection with session-scoped channels:

```
Client → lfd:  { "type": "attach", "session_id": "ts_abc123", "mode": "rw", "size": { "cols": 120, "rows": 40 } }
lfd → Client:  { "type": "attached", "session_id": "ts_abc123", "meta": { ... } }
lfd → Client:  { "type": "output", "session_id": "ts_abc123", "data": "<base64>" }
Client → lfd:  { "type": "input", "session_id": "ts_abc123", "data": "<base64>" }
Client → lfd:  { "type": "resize", "session_id": "ts_abc123", "cols": 80, "rows": 24 }
Client → lfd:  { "type": "detach", "session_id": "ts_abc123" }
```

Messages are typed JSON. Terminal bytes are base64-encoded inside JSON frames. This is simple and sufficient — binary framing is an optimization for later if throughput matters.

### 2. tmux I/O bridge in `lfd`

On attach:
- Start a `tmux capture-pane -p -e -t <session>` to send current screen state
- Begin polling `tmux capture-pane` at ~100ms for output (or use control mode `-C` if latency matters)
- Route input via `tmux send-keys -t <session> -l <keys>`
- Route resize via `tmux resize-window -t <session> -x <cols> -y <rows>`

Start with polling. Move to control mode (`tmux -C`) if latency is unacceptable.

### 3. Multi-client tracking

`lfd` maintains a set of attached clients per session:

```rust
struct AttachedClient {
    client_id: String,
    mode: AttachMode,       // ReadWrite | ReadOnly
    size: TerminalSize,
    attached_at: OffsetDateTime,
}
```

Size policy: `latest` — most recently attached read-write client controls the window size. Read-only clients observe without affecting size.

### 4. Remove launch-spec shim

Once the attach protocol works, delete `TerminalLaunchSpecDto` and the `POST .../attach` endpoint that returns argv. Concerto switches from "run this tmux command locally" to "open a WebSocket channel to this session."

### 5. Executor regression tests

Add the test suite the design called for:
- Serialized vs parallel wave runs
- Queued activations (run starts while another is active)
- CI-fix / reactive runs triggered by signals
- Cancellation mid-run
- Failure propagation (exit code → run status → wave status)
- Run-scoped flow/area/direction overrides

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Binary WebSocket frames for terminal I/O | Lower overhead, no base64 tax | Premature optimization. JSON framing is debuggable and the WS infra already handles JSON. Switch later if profiling shows it matters. |
| tmux control mode (`-C`) from the start | Lower latency, push-based output | Higher complexity. Parsing `%output`, `%begin`/`%end` framing, flow control. Polling `capture-pane` is simpler and sufficient for v1. |
| Direct PTY ownership (skip tmux) | No tmux dependency | Reimplements session persistence, scrollback, process supervision. tmux already does this well. |
| Separate WebSocket endpoint for terminal I/O | Clean separation from event stream | Adds connection management complexity. Multiplexing over the existing WS is simpler — sessions are already identified by ID. |

## Key decisions

**tmux stays as PTY owner.** `lfd` is a frontend to tmux, not a replacement. This is the decision from the architecture study and it holds.

**Polling before control mode.** `capture-pane` polling at 100ms is ~10fps, which is fine for agent sessions where most output is text. Control mode adds parsing complexity for marginal latency improvement.

**JSON over the existing WebSocket.** No new connections, no binary protocol. The attach sub-protocol is just new message types on the same channel.

**`latest` size policy only.** No smallest/largest/manual for v1. Most recent read-write client wins. Simple, covers the desktop+mobile case.

## Scope

- In scope: WebSocket attach/detach/input/output/resize, tmux I/O bridge, multi-client tracking, size negotiation (latest policy), remove launch-spec shim, executor regression tests
- Out of scope: TLS/remote auth, SSH-style remote access, control mode, binary framing, scrollback persistence beyond tmux's buffer, Concerto UI changes (separate wave)

## Done when

- `lfd` can stream terminal I/O for a tmux session over WebSocket
- Clients send input and resize through `lfd`, never touching tmux directly
- Multiple clients can attach to the same session; latest read-write client controls size
- Detach/reattach works (session continues running, client reconnects and gets current screen)
- Launch-spec shim (`TerminalLaunchSpecDto`) is deleted
- Executor regression tests pass for the cases listed above
- `attach_terminal_session_handler` returns a WebSocket channel, not argv
