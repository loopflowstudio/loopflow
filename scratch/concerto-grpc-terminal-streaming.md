---
status: todo
phase: 2
---

# gRPC Terminal Streaming

Bidirectional stream for remote terminal I/O. Mobile gets full interactive control.

## Current

Terminal runs locally via Ghostty. No remote streaming.

## Build

Remote terminal streams go through loopflow.studio relay:

```
┌─────────────┐              ┌─────────────────┐              ┌─────────────────────────┐
│   Mobile    │    TLS       │  loopflow.studio│   tunnel     │   Mac/Server + lfd      │
│  Concerto   │ ◄──────────► │     (relay)     │ ◄──────────► │                         │
│             │              │                 │              │   ┌─────────────────┐   │
│  ┌───────┐  │   gRPC       │  terminates TLS │   gRPC       │   │  pty/tmux       │   │
│  │ term  │  │   stream     │  validates JWT  │   stream     │   │  ┌───────────┐  │   │
│  │ view  │  │              │  routes to lfd  │              │   │  │ claude    │  │   │
│  └───────┘  │              │                 │              │   │  │ code      │  │   │
└─────────────┘              └─────────────────┘              │   │  └───────────┘  │   │
                                                              │   └─────────────────┘   │
                                                              └─────────────────────────┘
```

lfd maintains an outbound tunnel to loopflow.studio (solves NAT). Mobile connects to loopflow.studio which proxies through the tunnel.

lfd responsibilities:
- Maintain outbound tunnel to loopflow.studio
- Spawn agent in pty
- Multiplex terminal I/O over gRPC stream
- Handle reconnection (buffer ~1000 lines)

loopflow.studio responsibilities:
- TLS termination with real certs
- JWT validation for connection tokens
- Route streams to correct lfd via tunnel

## Done when

Mobile client can connect to remote lfd through loopflow.studio relay and interact with terminal session.
