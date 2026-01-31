---
status: todo
phase: 2
---

# gRPC Terminal Streaming

Bidirectional stream for remote terminal I/O. Mobile gets full interactive control.

## Current

Terminal runs locally via Ghostty. No remote streaming.

## Build

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

lfd responsibilities:
- Spawn agent in pty
- Multiplex terminal I/O over gRPC stream
- Handle reconnection (buffer ~1000 lines)

## Done when

Mobile client can connect to remote lfd and interact with terminal session.
