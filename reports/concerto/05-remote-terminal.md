# Remote Terminal Architecture

Mobile gets full interactive control. Agent runs on server, terminal streams to device.

```
┌─────────────┐              ┌─────────────────────────┐
│   Mobile    │    stream    │   Mac/Server + lfd      │
│  Concerto   │ ◄──────────► │                         │
│             │   (grpc)     │   ┌─────────────────┐   │
│  ┌───────┐  │              │   │  pty/tmux       │   │
│  │ term  │  │   input/     │   │  ┌───────────┐  │   │
│  │ view  │  │   output     │   │  │ claude    │  │   │
│  └───────┘  │              │   │  │ code      │  │   │
└─────────────┘              │   │  └───────────┘  │   │
                             │   └─────────────────┘   │
                             └─────────────────────────┘
```

## lfd Responsibilities

- Spawn agent in pty
- Multiplex terminal I/O over gRPC stream
- Handle reconnection (session persists if mobile disconnects)
- Buffer output for reconnecting clients (~1000 lines)

## Client Responsibilities

- Render terminal output (Ghostty view or similar)
- Send keystrokes
- Handle connection state

## Session Model

- Multiple clients can connect to same session, see same output
- Input from any connection goes to same pty
- First to hit "Continue" wins, others get "already finished"
- No ownership, no locks, no heartbeats
- Session doesn't complete until you say so (interactive steps wait)

## Protocol Choice

gRPC bidirectional stream. Leaves room for structured events later (understanding agent conversation), but can start with raw bytes. Aligns with Rust lfd's gRPC-first design.

Future possibility: semantic events for understanding which characters composed an agent message, when agent is waiting vs working, etc. Not needed for v1.
